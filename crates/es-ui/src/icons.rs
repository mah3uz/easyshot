use gpui::SharedString;
use gpui_component::IconNamed;

macro_rules! icons {
    (
        lucide { $($lucide:ident = $lucide_name:literal,)* }
        custom { $($custom:ident = $custom_name:literal,)* }
    ) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum IconName {
            $($lucide,)*
            $($custom,)*
        }

        impl IconName {
            pub const ALL: &[IconName] = &[$(Self::$lucide,)* $(Self::$custom,)*];

            pub const fn name(self) -> &'static str {
                match self {
                    $(Self::$lucide => $lucide_name,)*
                    $(Self::$custom => $custom_name,)*
                }
            }

            pub const fn is_lucide(self) -> bool {
                matches!(self, $(Self::$lucide)|*)
            }

            pub const fn asset(self) -> &'static str {
                match self {
                    $(Self::$lucide => concat!("icons/lucide/", $lucide_name, ".svg"),)*
                    $(Self::$custom => concat!("icons/", $custom_name, ".svg"),)*
                }
            }
        }
    };
}

icons! {
    lucide {
        ALargeSmall = "a-large-small",
        AppWindow = "app-window",
        ArrowUpDown = "arrow-up-down",
        ArrowUpRight = "arrow-up-right",
        Camera = "camera",
        Check = "check",
        ChevronDown = "chevron-down",
        ChevronRight = "chevron-right",
        ChevronsUpDown = "chevrons-up-down",
        Circle = "circle",
        CircleAlert = "circle-alert",
        CircleArrowDown = "circle-arrow-down",
        CircleCheck = "circle-check",
        CircleDot = "circle-dot",
        CircleEllipsis = "circle-ellipsis",
        CirclePause = "circle-pause",
        CirclePlay = "circle-play",
        CirclePlus = "circle-plus",
        CircleX = "circle-x",
        Cloud = "cloud",
        CloudCheck = "cloud-check",
        CloudOff = "cloud-off",
        CloudUpload = "cloud-upload",
        Copy = "copy",
        Crop = "crop",
        Download = "download",
        Droplet = "droplet",
        EyeOff = "eye-off",
        FileText = "file-text",
        Film = "film",
        FlipHorizontal2 = "flip-horizontal-2",
        FlipVertical2 = "flip-vertical-2",
        Folder = "folder",
        FolderOpen = "folder-open",
        Grid2x2 = "grid-2x2",
        Grid3x3 = "grid-3x3",
        Hand = "hand",
        HardDrive = "hard-drive",
        House = "house",
        Image = "image",
        Images = "images",
        Info = "info",
        KeyRound = "key-round",
        Keyboard = "keyboard",
        Layers = "layers",
        Link = "link",
        List = "list",
        Lock = "lock",
        Mic = "mic",
        MicOff = "mic-off",
        Minus = "minus",
        Monitor = "monitor",
        MousePointer2 = "mouse-pointer-2",
        Palette = "palette",
        PanelLeft = "panel-left",
        PanelTop = "panel-top",
        Pencil = "pencil",
        PencilLine = "pencil-line",
        Pin = "pin",
        PinOff = "pin-off",
        Pipette = "pipette",
        Plus = "plus",
        Pointer = "pointer",
        Power = "power",
        Ratio = "ratio",
        RectangleHorizontal = "rectangle-horizontal",
        Redo2 = "redo-2",
        RefreshCw = "refresh-cw",
        RotateCcw = "rotate-ccw",
        RotateCcwClock = "rotate-ccw-clock",
        RotateCw = "rotate-cw",
        Scan = "scan",
        ScanText = "scan-text",
        Settings = "settings",
        Slash = "slash",
        Square = "square",
        SquareArrowOutUpRight = "square-arrow-out-up-right",
        SquareDashed = "square-dashed",
        SquarePen = "square-pen",
        SquarePlay = "square-play",
        SquareSlash = "square-slash",
        Target = "target",
        TextAlignCenter = "text-align-center",
        TextAlignEnd = "text-align-end",
        TextAlignJustify = "text-align-justify",
        TextAlignStart = "text-align-start",
        Timer = "timer",
        Trash = "trash",
        TriangleAlert = "triangle-alert",
        Type = "type",
        Undo2 = "undo-2",
        UnfoldVertical = "unfold-vertical",
        Video = "video",
        VideoOff = "video-off",
        Volume2 = "volume-2",
        VolumeOff = "volume-off",
        X = "x",
    }
    custom {
        Counter = "counter",
        Highlight = "highlight",
        Highlighter = "highlighter",
        HighlighterLine = "highlighter-line",
        Pixelate = "pixelate",
        Easyshot = "easyshot",
    }
}

impl IconNamed for IconName {
    fn path(self) -> SharedString {
        SharedString::new_static(self.asset())
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeSet, path::Path};

    use gpui::AssetSource;

    use super::IconName;
    use crate::assets::Assets;

    fn assets_dir() -> &'static Path {
        Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets"))
    }

    #[test]
    fn every_icon_has_its_svg_on_disk_and_embedded() {
        for icon in IconName::ALL {
            assert!(
                assets_dir().join(icon.asset()).is_file(),
                "{} is missing from assets/",
                icon.asset()
            );
            let embedded = Assets.load(icon.asset()).ok().flatten();
            assert!(
                embedded.is_some_and(|svg| svg.windows(4).any(|w| w == b"<svg")),
                "{} is not embedded as an SVG",
                icon.asset()
            );
        }
    }

    // Only the listed Lucide icons are bundled, so a name outside the list would draw nothing.
    #[test]
    fn the_lucide_icons_are_exactly_the_bundled_list() {
        let listed = std::fs::read_to_string(assets_dir().join("icons/lucide.txt"))
            .expect("read lucide.txt");
        let listed: BTreeSet<&str> = listed
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect();
        let named: BTreeSet<&str> = IconName::ALL
            .iter()
            .filter(|i| i.is_lucide())
            .map(|i| i.name())
            .collect();
        assert_eq!(named, listed);
        assert_eq!(IconName::ALL.len(), listed.len() + 6, "six custom icons");
    }
}
