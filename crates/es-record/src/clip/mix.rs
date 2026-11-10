use super::export::{ExportJob, codec_args, num};
use super::options::{ExportOptions, Format};

pub(super) fn mixes(job: &ExportJob) -> bool {
    job.audio_tracks >= 2 && job.options.format != Format::Gif && !job.remove_audio
}

/// Levels are exactly what the options say: `normalize=0` keeps `amix` from scaling the sum down.
pub(super) fn mix_filter(o: &ExportOptions) -> String {
    format!(
        "[0:a:0]volume={}[sys];[0:a:1]volume={}[mic];[sys][mic]amix=inputs=2:normalize=0:duration=longest[mix]",
        num(o.system_volume),
        num(o.mic_volume)
    )
}

pub(super) fn audio_codec_args(o: &ExportOptions) -> Vec<String> {
    let args = codec_args(o);
    args.iter()
        .position(|a| a == "-c:a")
        .map(|i| args[i..i + 4].to_vec())
        .unwrap_or_default()
}
