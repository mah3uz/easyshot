use es_model::ShareKind;
use es_record::clip::export::{
    CancellableFfmpeg, ExportError, ExportJob, ExportMode, FfmpegRunner,
};
use es_record::clip::options::ExportOptions;
use es_record::clip::share::{
    ShareError as ClipError, TooLong, export_poster, export_share, share_plan,
};
use es_record::clip::trim::Trim;
use es_record::project::{CommandRunner, SystemRunner, poster_time};
use es_record::recovery::{audio_track_count, probe_video};
use es_render::encode::encode_jpeg;
use es_render::image_ops::resize_premultiplied;
use es_share::Media;
use es_store::Paths;
use es_store::temp::{self, TempKind};
use image::{ImageFormat, ImageReader, RgbaImage};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use tiny_skia::Pixmap;

const GIB: u64 = 1024 * 1024 * 1024;
const JPEG_QUALITY: u8 = 85;
const SHRINK_STEP: f64 = 0.85;
const SHRINK_MARGIN: f64 = 0.9;
const SMALLEST_SIDE: u32 = 32;
const RECORDING_VIDEOS: [&str; 2] = ["screen.mkv", "screen.mp4"];

/// The most a target takes of one file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Caps {
    pub image_max_bytes: u64,
    pub video_max_bytes: u64,
}

impl Caps {
    /// An own bucket accepts what one PUT carries.
    pub const BUCKET: Caps = Caps {
        image_max_bytes: 5 * GIB,
        video_max_bytes: 5 * GIB,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrepareError {
    /// Even at 720p the clip does not fit; `max_duration_ms_at_720p` is how long a clip fits.
    TooLong {
        duration_ms: u64,
        max_duration_ms_at_720p: u64,
    },
    TooLarge,
    File,
    Encode,
    Cancelled,
}

/// What to send, and the temporary files made for it, which go when this is dropped.
#[derive(Debug)]
pub struct Prepared {
    pub media: Media,
    dir: Option<PathBuf>,
}

impl Drop for Prepared {
    fn drop(&mut self) {
        remove(self.dir.as_deref());
    }
}

fn remove(dir: Option<&Path>) {
    if let Some(dir) = dir
        && let Err(e) = std::fs::remove_dir_all(dir)
    {
        tracing::warn!(error = %e, "share temp files not removed");
    }
}

pub struct Job<'a> {
    pub paths: &'a Paths,
    pub caps: Caps,
    /// How the trim viewer would export this recording.
    pub video: ExportOptions,
    /// The trim viewer's own export of this video, sent as trimmed; `None` sends it whole with `video`.
    pub export: Option<&'a ExportJob>,
    pub cancel: &'a AtomicBool,
}

pub struct Tools<'a> {
    pub probe: &'a dyn CommandRunner,
    pub ffmpeg: &'a dyn FfmpegRunner,
}

/// Blocks; run it off the GPUI thread. `progress` gets 0..=1.
pub fn prepare(
    source: &Path,
    job: &Job,
    progress: &mut dyn FnMut(f64),
) -> Result<Prepared, PrepareError> {
    let ffmpeg = CancellableFfmpeg(job.cancel);
    let tools = Tools {
        probe: &SystemRunner,
        ffmpeg: &ffmpeg,
    };
    prepare_with(source, job, &tools, progress)
}

pub fn prepare_with(
    source: &Path,
    job: &Job,
    tools: &Tools,
    progress: &mut dyn FnMut(f64),
) -> Result<Prepared, PrepareError> {
    if let Some(video) = video_file(source) {
        video_share(&video, job, tools, progress)
    } else {
        image_share(source, job)
    }
}

fn video_file(source: &Path) -> Option<PathBuf> {
    if source.is_dir() {
        return RECORDING_VIDEOS
            .iter()
            .map(|name| source.join(name))
            .find(|file| file.is_file());
    }
    es_store::media::is_video(source).then(|| source.to_owned())
}

fn file_err(e: impl std::fmt::Display) -> PrepareError {
    tracing::warn!(error = %e, "share source");
    PrepareError::File
}

fn cancelled(cancel: &AtomicBool) -> Result<(), PrepareError> {
    if cancel.load(Ordering::Relaxed) {
        Err(PrepareError::Cancelled)
    } else {
        Ok(())
    }
}

fn video_share(
    video: &Path,
    job: &Job,
    tools: &Tools,
    progress: &mut dyn FnMut(f64),
) -> Result<Prepared, PrepareError> {
    let probe = probe_video(tools.probe, video).ok_or_else(|| file_err("ffprobe"))?;
    let tracks = audio_track_count(tools.probe, video);
    let chosen = job.export.cloned().unwrap_or_else(|| ExportJob {
        input: video.to_owned(),
        output: PathBuf::new(),
        trim: Trim::new(probe.duration_secs),
        mode: ExportMode::Precise,
        remove_audio: false,
        audio_tracks: tracks,
        options: job.video,
    });
    let duration_ms = (chosen.trim.kept() * 1000.) as u64;
    let plan = share_plan(
        duration_ms,
        probe.width,
        probe.height,
        tracks > 0 && !chosen.remove_audio,
        job.caps.video_max_bytes,
    )
    .map_err(
        |TooLong {
             max_duration_ms_at_720p,
         }| PrepareError::TooLong {
            duration_ms,
            max_duration_ms_at_720p,
        },
    )?;
    cancelled(job.cancel)?;
    let dir = temp::make_dir(job.paths, TempKind::Share).map_err(file_err)?;
    let prepared = Prepared {
        media: Media {
            kind: ShareKind::Video,
            mime: "video/mp4".into(),
            path: dir.join("share.mp4"),
            width: plan.width,
            height: plan.height,
            duration_ms: Some(duration_ms),
            poster: Some(dir.join("poster.jpg")),
        },
        dir: Some(dir),
    };
    let export = ExportJob {
        output: prepared.media.path.clone(),
        audio_tracks: tracks,
        ..chosen
    };
    let from_clip = |e: ClipError| {
        if job.cancel.load(Ordering::Relaxed) {
            return PrepareError::Cancelled;
        }
        match e {
            ClipError::OverCap { .. } => PrepareError::TooLarge,
            ClipError::Export(ExportError::Io(e)) => file_err(e),
            ClipError::Export(e) => {
                tracing::warn!(error = %e, "share export");
                PrepareError::Encode
            }
            ClipError::PosterTooBig => PrepareError::Encode,
        }
    };
    let max = job.caps.video_max_bytes;
    export_share(tools.ffmpeg, &export, &plan, max, &mut |f| {
        progress(f * 0.95)
    })
    .map_err(from_clip)?;
    if let Some(poster) = &prepared.media.poster {
        export_poster(
            tools.ffmpeg,
            &prepared.media.path,
            poster_time(export.trim.kept()),
            poster,
        )
        .map_err(from_clip)?;
    }
    progress(1.);
    cancelled(job.cancel)?;
    Ok(prepared)
}

fn empty_media() -> Media {
    Media {
        kind: ShareKind::Image,
        mime: String::new(),
        path: PathBuf::new(),
        width: 0,
        height: 0,
        duration_ms: None,
        poster: None,
    }
}

fn image_share(source: &Path, job: &Job) -> Result<Prepared, PrepareError> {
    let reader = ImageReader::open(source)
        .and_then(ImageReader::with_guessed_format)
        .map_err(file_err)?;
    let mime = match reader.format() {
        Some(ImageFormat::Png) => "image/png",
        Some(ImageFormat::Jpeg) => "image/jpeg",
        _ => return Err(file_err("not a PNG or JPEG")),
    };
    let (width, height) = reader.into_dimensions().map_err(file_err)?;
    let bytes = std::fs::metadata(source).map_err(file_err)?.len();
    if bytes <= job.caps.image_max_bytes {
        return Ok(Prepared {
            media: Media {
                kind: ShareKind::Image,
                mime: mime.into(),
                path: source.to_owned(),
                width,
                height,
                duration_ms: None,
                poster: None,
            },
            dir: None,
        });
    }
    let base = premultiplied(&image::open(source).map_err(file_err)?.into_rgba8())?;
    let dir = temp::make_dir(job.paths, TempKind::Share).map_err(file_err)?;
    let mut prepared = Prepared {
        media: empty_media(),
        dir: Some(dir.clone()),
    };
    let jpeg = shrink_to(&base, job.caps.image_max_bytes, job.cancel)?;
    let path = dir.join("share.jpg");
    std::fs::write(&path, &jpeg.bytes).map_err(file_err)?;
    prepared.media = Media {
        kind: ShareKind::Image,
        mime: "image/jpeg".into(),
        path,
        width: jpeg.width,
        height: jpeg.height,
        duration_ms: None,
        poster: None,
    };
    Ok(prepared)
}

struct Jpeg {
    bytes: Vec<u8>,
    width: u32,
    height: u32,
}

/// One JPEG under `max_bytes`, from `base` scaled down only as far as that takes.
fn shrink_to(base: &Pixmap, max_bytes: u64, cancel: &AtomicBool) -> Result<Jpeg, PrepareError> {
    let mut scale = 1.0_f64;
    loop {
        cancelled(cancel)?;
        let (width, height) = (
            (f64::from(base.width()) * scale).round() as u32,
            (f64::from(base.height()) * scale).round() as u32,
        );
        if width.min(height) < SMALLEST_SIDE {
            return Err(PrepareError::TooLarge);
        }
        let bytes = if scale < 1. {
            let mut small = Pixmap::new(width, height).ok_or(PrepareError::Encode)?;
            resize_premultiplied(base, &mut small, None).ok_or(PrepareError::Encode)?;
            encode_jpeg(&small, JPEG_QUALITY)
        } else {
            encode_jpeg(base, JPEG_QUALITY)
        }
        .map_err(|e| {
            tracing::warn!(error = %e, "share image export");
            PrepareError::Encode
        })?;
        let len = bytes.len() as u64;
        if len <= max_bytes {
            return Ok(Jpeg {
                bytes,
                width,
                height,
            });
        }
        let by_size = (max_bytes as f64 / len as f64).sqrt() * SHRINK_MARGIN;
        scale *= by_size.min(SHRINK_STEP);
    }
}

fn premultiplied(image: &RgbaImage) -> Result<Pixmap, PrepareError> {
    let mut pixmap = Pixmap::new(image.width(), image.height()).ok_or(PrepareError::File)?;
    for (out, px) in pixmap
        .data_mut()
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(image.as_raw().as_chunks::<4>().0)
    {
        let a = u32::from(px[3]);
        for c in 0..3 {
            out[c] = ((u32::from(px[c]) * a + 127) / 255) as u8;
        }
        out[3] = px[3];
    }
    Ok(pixmap)
}
