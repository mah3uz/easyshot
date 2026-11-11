use super::options::{ExportOptions, Format, Quality};

pub(super) fn num(v: f64) -> String {
    format!("{v:.3}")
}

pub(super) fn picture_filters(o: &ExportOptions) -> Vec<String> {
    let mut filters = Vec::new();
    if let Some(h) = o.size.height() {
        filters.push(format!("scale=-2:'min({h},ih)'"));
    }
    let (contrast, saturation) = o.look.tone();
    let mut eq = Vec::new();
    if o.brightness != 0. {
        eq.push(format!("brightness={}", num(o.brightness)));
    }
    if contrast != 1. {
        eq.push(format!("contrast={}", num(contrast)));
    }
    if saturation != 1. {
        eq.push(format!("saturation={}", num(saturation)));
    }
    if !eq.is_empty() {
        filters.push(format!("eq={}", eq.join(":")));
    }
    filters
}

pub(super) fn gif_filter(o: &ExportOptions) -> String {
    let (fps, colours) = match o.quality {
        Quality::High => (15, 256),
        Quality::Balanced => (12, 192),
        Quality::Small => (10, 128),
    };
    let mut chain = picture_filters(o);
    chain.push(format!("fps={fps}"));
    chain.push("split[a][b]".into());
    format!(
        "{};[a]palettegen=max_colors={colours}:stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=5:diff_mode=rectangle",
        chain.join(",")
    )
}

pub(super) fn codec_args(o: &ExportOptions) -> Vec<String> {
    let (crf, audio) = match (o.format, o.quality) {
        (Format::Mp4, Quality::High) => ("18", "192k"),
        (Format::Mp4, Quality::Balanced) => ("23", "160k"),
        (Format::Mp4, Quality::Small) => ("28", "128k"),
        (_, Quality::High) => ("28", "160k"),
        (_, Quality::Balanced) => ("33", "112k"),
        (_, Quality::Small) => ("40", "80k"),
    };
    let args: Vec<&str> = match o.format {
        Format::Mp4 => vec![
            "-c:v",
            "libx264",
            "-preset",
            "veryfast",
            "-crf",
            crf,
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-b:a",
            audio,
            "-movflags",
            "+faststart",
            "-f",
            "mp4",
        ],
        Format::WebM => vec![
            "-c:v",
            "libvpx-vp9",
            "-crf",
            crf,
            "-b:v",
            "0",
            "-row-mt",
            "1",
            "-deadline",
            "good",
            "-cpu-used",
            "4",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "libopus",
            "-b:a",
            audio,
            "-f",
            "webm",
        ],
        Format::Gif => vec!["-loop", "0", "-f", "gif"],
    };
    args.into_iter().map(String::from).collect()
}
