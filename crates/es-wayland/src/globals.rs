#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WlGlobals {
    pub layer_shell: Option<u32>,
    pub ext_image_copy_capture: Option<u32>,
    pub ext_output_capture_source: Option<u32>,
    pub ext_toplevel_capture_source: Option<u32>,
    pub wlr_screencopy: Option<u32>,
    pub ext_foreign_toplevel_list: Option<u32>,
    pub wlr_foreign_toplevel: Option<u32>,
    pub ext_data_control: Option<u32>,
    pub wlr_data_control: Option<u32>,
    pub virtual_pointer: Option<u32>,
    pub cursor_shape: Option<u32>,
    pub fractional_scale: Option<u32>,
    pub xdg_output: Option<u32>,
    pub shm: Option<u32>,
    pub lock_notify: Option<u32>,
    pub all: Vec<(String, u32)>,
}

impl WlGlobals {
    pub fn from_globals<'a>(globals: impl IntoIterator<Item = (&'a str, u32)>) -> Self {
        let mut g = WlGlobals::default();
        for (interface, version) in globals {
            let slot = match interface {
                "zwlr_layer_shell_v1" => Some(&mut g.layer_shell),
                "ext_image_copy_capture_manager_v1" => Some(&mut g.ext_image_copy_capture),
                "ext_output_image_capture_source_manager_v1" => {
                    Some(&mut g.ext_output_capture_source)
                }
                "ext_foreign_toplevel_image_capture_source_manager_v1" => {
                    Some(&mut g.ext_toplevel_capture_source)
                }
                "zwlr_screencopy_manager_v1" => Some(&mut g.wlr_screencopy),
                "ext_foreign_toplevel_list_v1" => Some(&mut g.ext_foreign_toplevel_list),
                "zwlr_foreign_toplevel_manager_v1" => Some(&mut g.wlr_foreign_toplevel),
                "ext_data_control_manager_v1" => Some(&mut g.ext_data_control),
                "zwlr_data_control_manager_v1" => Some(&mut g.wlr_data_control),
                "zwlr_virtual_pointer_manager_v1" => Some(&mut g.virtual_pointer),
                "wp_cursor_shape_manager_v1" => Some(&mut g.cursor_shape),
                "wp_fractional_scale_manager_v1" => Some(&mut g.fractional_scale),
                "zxdg_output_manager_v1" => Some(&mut g.xdg_output),
                "wl_shm" => Some(&mut g.shm),
                "hyprland_lock_notifier_v1" => Some(&mut g.lock_notify),
                _ => None,
            };
            if let Some(slot) = slot {
                *slot = Some(slot.map_or(version, |v| v.max(version)));
            }
            if interface != "wl_output" {
                g.all.push((interface.to_owned(), version));
            }
        }
        g.all.sort();
        g.all.dedup();
        g
    }

    pub fn ext_output_capture(&self) -> bool {
        self.shm.is_some()
            && self.ext_image_copy_capture.is_some()
            && self.ext_output_capture_source.is_some()
    }

    pub fn toplevel_capture(&self) -> bool {
        self.shm.is_some()
            && self.ext_image_copy_capture.is_some()
            && self.ext_toplevel_capture_source.is_some()
            && self.ext_foreign_toplevel_list.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wayland_client::Proxy;

    #[test]
    fn interface_names_match_the_generated_protocols() {
        use wayland_protocols::ext::{
            data_control::v1::client::ext_data_control_manager_v1::ExtDataControlManagerV1,
            foreign_toplevel_list::v1::client::ext_foreign_toplevel_list_v1::ExtForeignToplevelListV1,
            image_capture_source::v1::client::{
                ext_foreign_toplevel_image_capture_source_manager_v1::ExtForeignToplevelImageCaptureSourceManagerV1,
                ext_output_image_capture_source_manager_v1::ExtOutputImageCaptureSourceManagerV1,
            },
            image_copy_capture::v1::client::ext_image_copy_capture_manager_v1::ExtImageCopyCaptureManagerV1,
        };
        use wayland_protocols::wp::{
            cursor_shape::v1::client::wp_cursor_shape_manager_v1::WpCursorShapeManagerV1,
            fractional_scale::v1::client::wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1,
        };
        use wayland_protocols::xdg::xdg_output::zv1::client::zxdg_output_manager_v1::ZxdgOutputManagerV1;
        use wayland_protocols_wlr::{
            data_control::v1::client::zwlr_data_control_manager_v1::ZwlrDataControlManagerV1,
            foreign_toplevel::v1::client::zwlr_foreign_toplevel_manager_v1::ZwlrForeignToplevelManagerV1,
            layer_shell::v1::client::zwlr_layer_shell_v1::ZwlrLayerShellV1,
            screencopy::v1::client::zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1,
            virtual_pointer::v1::client::zwlr_virtual_pointer_manager_v1::ZwlrVirtualPointerManagerV1,
        };
        let names = [
            ZwlrLayerShellV1::interface().name,
            ExtImageCopyCaptureManagerV1::interface().name,
            ExtOutputImageCaptureSourceManagerV1::interface().name,
            ExtForeignToplevelImageCaptureSourceManagerV1::interface().name,
            ZwlrScreencopyManagerV1::interface().name,
            ExtForeignToplevelListV1::interface().name,
            ZwlrForeignToplevelManagerV1::interface().name,
            ExtDataControlManagerV1::interface().name,
            ZwlrDataControlManagerV1::interface().name,
            ZwlrVirtualPointerManagerV1::interface().name,
            WpCursorShapeManagerV1::interface().name,
            WpFractionalScaleManagerV1::interface().name,
            ZxdgOutputManagerV1::interface().name,
            wayland_client::protocol::wl_shm::WlShm::interface().name,
            crate::lock::protocol::hyprland_lock_notifier_v1::HyprlandLockNotifierV1::interface()
                .name,
        ];
        let g = WlGlobals::from_globals(names.iter().map(|n| (*n, 1)));
        let slots = [
            g.layer_shell,
            g.ext_image_copy_capture,
            g.ext_output_capture_source,
            g.ext_toplevel_capture_source,
            g.wlr_screencopy,
            g.ext_foreign_toplevel_list,
            g.wlr_foreign_toplevel,
            g.ext_data_control,
            g.wlr_data_control,
            g.virtual_pointer,
            g.cursor_shape,
            g.fractional_scale,
            g.xdg_output,
            g.shm,
            g.lock_notify,
        ];
        for (name, slot) in names.iter().zip(slots) {
            assert_eq!(slot, Some(1), "{name} not recognised");
        }
    }

    #[test]
    fn screencopy_alone_is_neither_ext_output_capture_nor_windows() {
        let g = WlGlobals::from_globals([("wl_shm", 1), ("zwlr_screencopy_manager_v1", 3)]);
        assert!(!g.ext_output_capture());
        assert!(!g.toplevel_capture());
    }

    #[test]
    fn ext_capture_needs_the_source_manager_and_shm() {
        let without_source =
            WlGlobals::from_globals([("wl_shm", 1), ("ext_image_copy_capture_manager_v1", 1)]);
        assert!(!without_source.ext_output_capture());
        let without_shm = WlGlobals::from_globals([
            ("ext_image_copy_capture_manager_v1", 1),
            ("ext_output_image_capture_source_manager_v1", 1),
        ]);
        assert!(!without_shm.ext_output_capture());
    }

    #[test]
    fn highest_version_wins_and_outputs_are_not_listed() {
        let g = WlGlobals::from_globals([
            ("wl_output", 4),
            ("wl_output", 4),
            ("wp_cursor_shape_manager_v1", 1),
            ("wp_cursor_shape_manager_v1", 2),
            ("vendor_extra_manager_v1", 1),
        ]);
        assert_eq!(g.cursor_shape, Some(2));
        assert_eq!(
            g.all,
            vec![
                ("vendor_extra_manager_v1".to_owned(), 1),
                ("wp_cursor_shape_manager_v1".to_owned(), 1),
                ("wp_cursor_shape_manager_v1".to_owned(), 2),
            ]
        );
    }
}
