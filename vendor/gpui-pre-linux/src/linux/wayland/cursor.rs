use std::collections::HashMap;
use std::fs::File;
use std::io::Write as _;
use std::os::fd::{AsFd as _, FromRawFd as _};

use crate::linux::{DEFAULT_CURSOR_ICON_NAME, log_cursor_icon_warning};
use crate::linux::{Globals, WaylandClientStatePtr};
use anyhow::{Context as _, anyhow};
use gpui::{CursorImage, CustomCursorId};
use gpui_util::ResultExt;

use wayland_client::protocol::wl_buffer::WlBuffer;
use wayland_client::protocol::wl_compositor::WlCompositor;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::protocol::{wl_pointer::WlPointer, wl_shm, wl_shm::WlShm};
use wayland_client::{Connection, QueueHandle};
use wayland_cursor::{CursorImageBuffer, CursorTheme};
use wayland_protocols::wp::viewporter::client::wp_viewport::WpViewport;
use wayland_protocols::wp::viewporter::client::wp_viewporter::WpViewporter;

pub(crate) struct Cursor {
    loaded_theme: Option<LoadedTheme>,
    size: u32,
    scaled_size: u32,
    surface: WlSurface,
    shm: WlShm,
    connection: Connection,
    custom_images: HashMap<CustomCursorId, CursorImage>,
    custom_buffers: HashMap<(CustomCursorId, usize), WlBuffer>,
    custom_surfaces: HashMap<CustomCursorId, CustomSurface>,
    compositor: WlCompositor,
    viewporter: Option<WpViewporter>,
    qh: QueueHandle<WaylandClientStatePtr>,
}

// One surface per cursor, so set_cursor names a surface that already shows its image.
struct CustomSurface {
    surface: WlSurface,
    viewport: Option<WpViewport>,
    shown: Option<usize>,
}

pub(crate) struct LoadedTheme {
    theme: CursorTheme,
    name: Option<String>,
    scaled_size: u32,
}

impl Drop for Cursor {
    fn drop(&mut self) {
        self.loaded_theme.take();
        self.surface.destroy();
        for buffer in self.custom_buffers.values() {
            buffer.destroy();
        }
        for custom in self.custom_surfaces.values() {
            if let Some(viewport) = &custom.viewport {
                viewport.destroy();
            }
            custom.surface.destroy();
        }
    }
}

impl Cursor {
    pub fn new(connection: &Connection, globals: &Globals, size: u32) -> Self {
        let mut this = Self {
            loaded_theme: None,
            size,
            scaled_size: size,
            surface: globals.compositor.create_surface(&globals.qh, ()),
            shm: globals.shm.clone(),
            connection: connection.clone(),
            custom_images: HashMap::new(),
            custom_buffers: HashMap::new(),
            custom_surfaces: HashMap::new(),
            compositor: globals.compositor.clone(),
            viewporter: globals.viewporter.clone(),
            qh: globals.qh.clone(),
        };
        this.set_theme_internal(None);
        this
    }

    fn set_theme_internal(&mut self, theme_name: Option<String>) {
        if let Some(loaded_theme) = self.loaded_theme.as_ref()
            && loaded_theme.name == theme_name
            && loaded_theme.scaled_size == self.scaled_size
        {
            return;
        }
        let result = if let Some(theme_name) = theme_name.as_ref() {
            CursorTheme::load_from_name(
                &self.connection,
                self.shm.clone(),
                theme_name,
                self.scaled_size,
            )
        } else {
            CursorTheme::load(&self.connection, self.shm.clone(), self.scaled_size)
        };
        if let Some(theme) = result
            .context("Wayland: Failed to load cursor theme")
            .log_err()
        {
            self.loaded_theme = Some(LoadedTheme {
                theme,
                name: theme_name,
                scaled_size: self.scaled_size,
            });
        }
    }

    pub fn set_theme(&mut self, theme_name: String) {
        self.set_theme_internal(Some(theme_name));
    }

    fn set_scaled_size(&mut self, scaled_size: u32) {
        self.scaled_size = scaled_size;
        let theme_name = self
            .loaded_theme
            .as_ref()
            .and_then(|loaded_theme| loaded_theme.name.clone());
        self.set_theme_internal(theme_name);
    }

    pub fn set_size(&mut self, size: u32) {
        self.size = size;
        self.set_scaled_size(size);
    }

    pub fn set_icon(
        &mut self,
        wl_pointer: &WlPointer,
        serial_id: u32,
        cursor_icon_names: &[&str],
        scale: i32,
    ) {
        self.set_scaled_size(self.size * scale as u32);

        let Some(loaded_theme) = &mut self.loaded_theme else {
            log::warn!("Wayland: Unable to load cursor themes");
            return;
        };
        let theme = &mut loaded_theme.theme;

        let buffer: &CursorImageBuffer;
        'outer: {
            for cursor_icon_name in cursor_icon_names {
                if let Some(cursor) = theme.get_cursor(cursor_icon_name) {
                    buffer = &cursor[0];
                    break 'outer;
                }
            }

            if let Some(cursor) = theme.get_cursor(DEFAULT_CURSOR_ICON_NAME) {
                buffer = &cursor[0];
                log_cursor_icon_warning(anyhow!(
                    "wayland: Unable to get cursor icon {:?}. \
                    Using default cursor icon: '{}'",
                    cursor_icon_names,
                    DEFAULT_CURSOR_ICON_NAME
                ));
            } else {
                log_cursor_icon_warning(anyhow!(
                    "wayland: Unable to fallback on default cursor icon '{}' for theme '{}'",
                    DEFAULT_CURSOR_ICON_NAME,
                    loaded_theme.name.as_deref().unwrap_or("default")
                ));
                return;
            }
        }

        let (width, height) = buffer.dimensions();
        let (hot_x, hot_y) = buffer.hotspot();

        self.surface.set_buffer_scale(scale);

        wl_pointer.set_cursor(
            serial_id,
            Some(&self.surface),
            hot_x as i32 / scale,
            hot_y as i32 / scale,
        );

        self.surface.attach(Some(buffer), 0, 0);
        self.surface.damage(0, 0, width as i32, height as i32);
        self.surface.commit();
    }

    pub fn register_custom(&mut self, id: CustomCursorId, image: CursorImage) {
        self.custom_images.insert(id, image);
    }

    /// Shows a registered cursor image on `wl_pointer`. Returns false if `id`
    /// has no image usable at `scale`.
    pub fn set_custom(
        &mut self,
        wl_pointer: &WlPointer,
        serial_id: u32,
        id: CustomCursorId,
        scale: f32,
    ) -> bool {
        let Some(image) = self.custom_images.get(&id) else {
            log::warn!("wayland: cursor image {id:?} is not registered");
            return false;
        };
        // Without a viewport the buffer scale must be an integer.
        let fractional = self.viewporter.is_some();
        let Some(index) = custom_image_for_scale(image, scale, fractional) else {
            log::warn!("wayland: cursor image {id:?} has no image usable at scale {scale}");
            return false;
        };
        let (image_scale, pixels) = &image.images[index];
        let width = (image.size.width as f32 * image_scale).round() as i32;
        let height = (image.size.height as f32 * image_scale).round() as i32;
        if width <= 0 || height <= 0 || pixels.len() != width as usize * height as usize * 4 {
            log::warn!(
                "wayland: cursor image {id:?} at scale {image_scale} is not {width}x{height} BGRA"
            );
            return false;
        }
        let buffer = match self.custom_buffers.entry((id, index)) {
            std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
            std::collections::hash_map::Entry::Vacant(entry) => {
                let Some(buffer) = create_shm_buffer(&self.shm, &self.qh, width, height, pixels)
                    .context("Wayland: Failed to create cursor buffer")
                    .log_err()
                else {
                    return false;
                };
                entry.insert(buffer)
            }
        };

        let custom = self.custom_surfaces.entry(id).or_insert_with(|| {
            let surface = self.compositor.create_surface(&self.qh, ());
            let viewport = self
                .viewporter
                .as_ref()
                .map(|viewporter| viewporter.get_viewport(&surface, &self.qh, ()));
            CustomSurface {
                surface,
                viewport,
                shown: None,
            }
        });
        for step in custom_steps(custom.shown, index) {
            match step {
                CustomStep::Commit(index) => {
                    if let Some(viewport) = &custom.viewport {
                        custom.surface.set_buffer_scale(1);
                        viewport.set_destination(image.size.width as i32, image.size.height as i32);
                    } else {
                        custom.surface.set_buffer_scale(*image_scale as i32);
                    }
                    custom.surface.attach(Some(buffer), 0, 0);
                    custom
                        .surface
                        .damage(0, 0, image.size.width as i32, image.size.height as i32);
                    custom.surface.commit();
                    custom.shown = Some(index);
                }
                CustomStep::SetCursor => wl_pointer.set_cursor(
                    serial_id,
                    Some(&custom.surface),
                    image.hotspot.x as i32,
                    image.hotspot.y as i32,
                ),
            }
        }
        true
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CustomStep {
    Commit(usize),
    SetCursor,
}

/// Compositors show a pointer surface at set_cursor, so its image is committed first, and only on change.
fn custom_steps(shown: Option<usize>, index: usize) -> impl Iterator<Item = CustomStep> {
    (shown != Some(index))
        .then_some(CustomStep::Commit(index))
        .into_iter()
        .chain([CustomStep::SetCursor])
}

/// The smallest image scale at or above `scale`, else the largest one.
fn custom_image_for_scale(image: &CursorImage, scale: f32, fractional: bool) -> Option<usize> {
    let usable = image
        .images
        .iter()
        .enumerate()
        .filter(|(_, (image_scale, _))| {
            *image_scale > 0.0 && (fractional || image_scale.fract() == 0.0)
        })
        .map(|(index, (image_scale, _))| (index, *image_scale));
    let mut above: Option<(usize, f32)> = None;
    let mut largest: Option<(usize, f32)> = None;
    for (index, image_scale) in usable {
        if image_scale >= scale && above.is_none_or(|(_, best)| image_scale < best) {
            above = Some((index, image_scale));
        }
        if largest.is_none_or(|(_, best)| image_scale > best) {
            largest = Some((index, image_scale));
        }
    }
    above.or(largest).map(|(index, _)| index)
}

/// A pool shorter than its buffer is a wl_shm protocol error, which ends the whole connection.
fn shm_pool_size(width: i32, height: i32, len: usize) -> Option<i32> {
    let (w, h) = (usize::try_from(width).ok()?, usize::try_from(height).ok()?);
    let size = w.checked_mul(h)?.checked_mul(4)?;
    if w == 0 || h == 0 || size != len {
        return None;
    }
    i32::try_from(size).ok()
}

/// Creates a `width` x `height` ARGB8888 buffer holding `bgra`.
pub(crate) fn create_shm_buffer(
    shm: &WlShm,
    qh: &QueueHandle<WaylandClientStatePtr>,
    width: i32,
    height: i32,
    bgra: &[u8],
) -> anyhow::Result<WlBuffer> {
    let size = shm_pool_size(width, height, bgra.len()).ok_or_else(|| {
        anyhow!(
            "{} bytes is not a {width}x{height} ARGB8888 image",
            bgra.len()
        )
    })?;
    // SAFETY: the name is a NUL-terminated static string, the flag is valid, the result is checked.
    let fd = unsafe { libc::memfd_create(c"gpui-shm".as_ptr(), libc::MFD_CLOEXEC) };
    if fd < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    // SAFETY: fd is a fresh valid descriptor owned by nothing else, so the File closes it once.
    let mut file = unsafe { File::from_raw_fd(fd) };
    file.write_all(bgra)?;
    let pool = shm.create_pool(file.as_fd(), size, qh, ());
    let buffer = pool.create_buffer(
        0,
        width,
        height,
        width * 4,
        wl_shm::Format::Argb8888,
        qh,
        (),
    );
    pool.destroy();
    Ok(buffer)
}

#[cfg(test)]
mod tests {
    use super::{CustomStep, custom_steps, shm_pool_size};

    fn steps(shown: Option<usize>, index: usize) -> Vec<CustomStep> {
        custom_steps(shown, index).collect()
    }

    #[test]
    fn a_cursor_image_is_committed_before_its_surface_becomes_the_pointer() {
        assert_eq!(
            steps(None, 1),
            [CustomStep::Commit(1), CustomStep::SetCursor]
        );
    }

    #[test]
    fn showing_a_cursor_again_does_not_upload_its_image_again() {
        assert_eq!(steps(Some(1), 1), [CustomStep::SetCursor]);
    }

    #[test]
    fn a_scale_change_commits_the_other_image_before_set_cursor() {
        assert_eq!(
            steps(Some(0), 2),
            [CustomStep::Commit(2), CustomStep::SetCursor]
        );
    }

    #[test]
    fn shm_pool_size_needs_exactly_four_bytes_per_pixel() {
        assert_eq!(shm_pool_size(3, 2, 24), Some(24));
        assert_eq!(shm_pool_size(3, 2, 23), None);
        assert_eq!(shm_pool_size(3, 2, 25), None);
        assert_eq!(shm_pool_size(0, 2, 0), None);
        assert_eq!(shm_pool_size(-3, -2, 24), None);
        assert_eq!(shm_pool_size(i32::MAX, i32::MAX, 0), None);
        assert_eq!(
            shm_pool_size(32768, 16384, 1 << 31),
            None,
            "larger than an i32 pool"
        );
    }
}
