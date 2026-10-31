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
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::protocol::{wl_pointer::WlPointer, wl_shm, wl_shm::WlShm};
use wayland_client::{Connection, QueueHandle};
use wayland_cursor::{CursorImageBuffer, CursorTheme};
use wayland_protocols::wp::viewporter::client::wp_viewport::WpViewport;

pub(crate) struct Cursor {
    loaded_theme: Option<LoadedTheme>,
    size: u32,
    scaled_size: u32,
    surface: WlSurface,
    shm: WlShm,
    connection: Connection,
    custom_images: HashMap<CustomCursorId, CursorImage>,
    custom_buffers: HashMap<(CustomCursorId, usize), WlBuffer>,
    custom_surface: WlSurface,
    custom_viewport: Option<WpViewport>,
    qh: QueueHandle<WaylandClientStatePtr>,
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
        if let Some(viewport) = &self.custom_viewport {
            viewport.destroy();
        }
        self.custom_surface.destroy();
    }
}

impl Cursor {
    pub fn new(connection: &Connection, globals: &Globals, size: u32) -> Self {
        let custom_surface = globals.compositor.create_surface(&globals.qh, ());
        let custom_viewport = globals
            .viewporter
            .as_ref()
            .map(|viewporter| viewporter.get_viewport(&custom_surface, &globals.qh, ()));
        let mut this = Self {
            loaded_theme: None,
            size,
            scaled_size: size,
            surface: globals.compositor.create_surface(&globals.qh, ()),
            shm: globals.shm.clone(),
            connection: connection.clone(),
            custom_images: HashMap::new(),
            custom_buffers: HashMap::new(),
            custom_surface,
            custom_viewport,
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
        let fractional = self.custom_viewport.is_some();
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

        if let Some(viewport) = &self.custom_viewport {
            self.custom_surface.set_buffer_scale(1);
            viewport.set_destination(image.size.width as i32, image.size.height as i32);
        } else {
            self.custom_surface.set_buffer_scale(*image_scale as i32);
        }

        wl_pointer.set_cursor(
            serial_id,
            Some(&self.custom_surface),
            image.hotspot.x as i32,
            image.hotspot.y as i32,
        );

        self.custom_surface.attach(Some(buffer), 0, 0);
        self.custom_surface
            .damage(0, 0, image.size.width as i32, image.size.height as i32);
        self.custom_surface.commit();
        true
    }
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

/// Creates a `width` x `height` ARGB8888 buffer holding `bgra`.
pub(crate) fn create_shm_buffer(
    shm: &WlShm,
    qh: &QueueHandle<WaylandClientStatePtr>,
    width: i32,
    height: i32,
    bgra: &[u8],
) -> anyhow::Result<WlBuffer> {
    let fd = unsafe { libc::memfd_create(c"gpui-shm".as_ptr(), libc::MFD_CLOEXEC) };
    if fd < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    let mut file = unsafe { File::from_raw_fd(fd) };
    file.write_all(bgra)?;
    let pool = shm.create_pool(file.as_fd(), bgra.len() as i32, qh, ());
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
