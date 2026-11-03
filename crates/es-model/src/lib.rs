pub mod background;
pub mod capture;
pub mod config;
pub mod document;
pub mod error;
pub mod format;
pub mod platform;
pub mod recording;
pub mod shape;
pub mod state;
pub mod style;
pub mod swatch;
mod time_serde;

pub use background::{
    Alignment, AspectRatio, BackgroundColor, BackgroundGradient, BackgroundPreset,
    BackgroundSettings, BackgroundStyle, BlurEdgeMode, BlurMode, BorderSettings, CameraSettings,
    CornerMultipliers, PresetLibrary, ProgressiveBlurSettings, ShadowStyle, UnitPoint,
    WatermarkSettings,
};
pub use capture::{
    CaptureKind, CaptureVariant, DeckManifest, HistoryEntry, HistoryIndex, Step,
    after_capture_plan, resolve_new_capture_look,
};
pub use config::Config;
pub use document::AnnotationDocument;
pub use error::{Error, Result};
pub use format::{Envelope, FormatKind, Header, Migrated, migrate};
pub use recording::{RecordingCapture, RecordingInput, RecordingProject};
pub use shape::{
    AnnoShape, AnnoShapeId, ArrowBinding, ArrowProps, ArrowTerminal, Arrowhead, DashStyle,
    DrawProps, FillStyle, FontFamily, GeoKind, GeoProps, HighlightProps, NormalizedAnchor,
    NumberedProps, RedactionKind, RedactionProps, ShapeKind, TextAlign, TextProps, Tool,
};
pub use state::{LastRegion, LastTarget, Recovery, State};
pub use style::StylePreset;
pub use swatch::Swatch;

#[cfg(test)]
mod fixture_tests {
    use super::*;

    #[test]
    fn history_v1_fixture_loads_and_migrates_as_current() {
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/formats/history/v1.json"
        ));
        let env: Envelope<HistoryIndex> = serde_json::from_str(src).unwrap();
        assert_eq!(env.format, FormatKind::History.name());
        assert_eq!(env.body.entries.len(), 2);
        let e = &env.body.entries[0];
        assert_eq!(e.kind, CaptureKind::Region);
        assert_eq!(e.pixel_size, (1280, 720));
        assert!(e.has_annotations);
        assert_eq!(env.body.entries[1].export_path, None);
        let v: serde_json::Value = serde_json::from_str(src).unwrap();
        assert!(matches!(
            migrate(FormatKind::History, env.version, v).unwrap(),
            Migrated::Current(_)
        ));
    }

    #[test]
    fn deck_item_v1_fixture_loads() {
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/formats/deck-item/v1.json"
        ));
        let env: Envelope<DeckManifest> = serde_json::from_str(src).unwrap();
        assert_eq!(env.format, FormatKind::DeckItem.name());
        assert_eq!(env.version, 1);
        assert_eq!(env.body.kind, CaptureKind::Window);
        assert_eq!(env.body.history_id, None);
        assert_eq!(
            env.body.created_at.unix_timestamp(),
            1_767_225_600 + 3600 * 12
        );
    }
}
