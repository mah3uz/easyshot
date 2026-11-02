use es_record::CommandRunner;
use es_store::Paths;
use std::path::{Path, PathBuf};

/// Each tile is cropped to this shape, so the strip reads the same whatever the video's shape.
pub const TILE_ASPECT: f64 = 16. / 10.;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StripSpec {
    pub tiles: u32,
    pub tile_height: u32,
}

impl StripSpec {
    /// Enough tiles to cover `width` x `height` pixels without stretching them.
    pub fn fitting(width: f64, height: f64) -> Self {
        let tiles = (width / (height * TILE_ASPECT)).round().max(1.) as u32;
        StripSpec {
            tiles,
            tile_height: height.round().max(2.) as u32,
        }
    }

    fn tile_width(&self) -> u32 {
        let w = (f64::from(self.tile_height) * TILE_ASPECT).round() as u32;
        w + w % 2
    }
}

/// One frame every `duration / tiles` seconds, laid side by side in a single picture.
pub fn strip_args(video: &Path, duration: f64, spec: StripSpec, out: &Path) -> Vec<String> {
    let (w, h) = (spec.tile_width(), spec.tile_height);
    let rate = f64::from(spec.tiles) / duration.max(0.01);
    let filter = format!(
        "fps={rate:.6},scale={w}:{h}:force_original_aspect_ratio=increase,crop={w}:{h},tile={}x1",
        spec.tiles
    );
    [
        "-y",
        "-v",
        "error",
        "-threads",
        "2",
        "-i",
        &video.to_string_lossy(),
        "-vf",
        &filter,
        "-frames:v",
        "1",
        "-update",
        "1",
        "-c:v",
        "png",
        "-f",
        "image2",
        &out.to_string_lossy(),
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect()
}

/// The cached strip when there is one; otherwise ffmpeg makes it, finished before it is visible.
pub fn strip(
    runner: &dyn CommandRunner,
    paths: &Paths,
    video: &Path,
    duration: f64,
    spec: StripSpec,
) -> Result<PathBuf, String> {
    let cached = es_store::thumbs::strip_path_for(paths, video, spec.tiles, spec.tile_height)
        .ok_or_else(|| format!("cannot read {}", video.display()))?;
    if cached.is_file() {
        return Ok(cached);
    }
    let dir = cached.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let part = cached.with_extension("png.part");
    let out = runner
        .run("ffmpeg", &strip_args(video, duration, spec, &part))
        .map_err(|e| e.to_string())?;
    if !out.status.success() || !part.is_file() {
        let _ = std::fs::remove_file(&part);
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_owned());
    }
    std::fs::rename(&part, &cached).map_err(|e| e.to_string())?;
    Ok(cached)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiles_fill_the_width_at_their_own_shape() {
        let spec = StripSpec::fitting(1000., 50.);
        assert_eq!(spec.tiles, 13);
        assert_eq!(spec.tile_width(), 80);
        assert_eq!(StripSpec::fitting(10., 50.).tiles, 1);
    }

    #[test]
    fn the_filter_takes_one_frame_per_tile_across_the_whole_video() {
        let spec = StripSpec {
            tiles: 10,
            tile_height: 60,
        };
        let args = strip_args(Path::new("/v.mkv"), 5., spec, Path::new("/s.png"));
        let vf = args
            .iter()
            .position(|a| a == "-vf")
            .and_then(|i| args.get(i + 1))
            .unwrap();
        assert!(vf.starts_with("fps=2.000000,"), "{vf}");
        assert!(vf.ends_with("crop=96:60,tile=10x1"), "{vf}");
    }
}
