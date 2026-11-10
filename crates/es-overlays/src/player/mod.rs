#[cfg(feature = "player")]
mod controls;
#[cfg(feature = "player")]
pub mod engine;
#[cfg(feature = "player")]
mod scrubber;
pub mod strip;
#[cfg(feature = "player")]
mod view;
#[cfg(feature = "player")]
mod volume;

pub use es_record::clip::{export, options, trim};

#[cfg(feature = "player")]
pub use engine::AudioOut;
pub use export::{ExportError, ExportJob, ExportMode, FfmpegRunner, SystemFfmpeg};
pub use options::{BRIGHTNESS_LIMIT, ExportOptions, Format, Look, Quality, Size};
#[cfg(feature = "player")]
pub use scrubber::HANDLE_WIDTH;
pub use trim::{MIN_TRIM_SECS, Trim};
#[cfg(feature = "player")]
pub use view::{ExportState, PlayerConfig, PlayerEvent, PlayerWindow, TITLE_BAR_HEIGHT};
