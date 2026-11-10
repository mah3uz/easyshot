use super::cut::{Cut, RATE, Timeline, sample_at, time_of};
use crate::mute::AudioSource;
use crate::{Error, RecordSpec, Result};
use gstreamer as gst;
use gstreamer::prelude::*;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const CHANNELS: i32 = 2;
const FRAME_BYTES: u64 = 2 * CHANNELS as u64;
const OPUS_BITRATE: i32 = 128_000;
const FILLER_SAMPLES: i32 = 480;
// Lets a capture stream run this late before its samples are mixed as silence.
const MIX_LATENCY: u64 = 200_000_000;
const MAX_WAIT: Duration = Duration::from_millis(100);

pub(crate) const ELEMENTS: [&str; 6] = [
    "pulsesrc",
    "audioconvert",
    "audioresample",
    "audiotestsrc",
    "audiomixer",
    "opusenc",
];

pub(crate) type OpenAudio = dyn Fn(AudioSource, &RecordSpec) -> Result<gst::Element> + Send + Sync;

pub(crate) fn installed() -> bool {
    gst::init().is_ok()
        && ELEMENTS
            .iter()
            .all(|e| gst::ElementFactory::find(e).is_some())
}

fn audio_err(e: impl std::fmt::Display) -> Error {
    Error::Audio(e.to_string())
}

fn make(factory: &str) -> Result<gst::Element> {
    gst::ElementFactory::make(factory)
        .build()
        .map_err(audio_err)
}

/// The sound server's name for what `source` records; `None` is its default microphone.
pub(crate) fn pulse_device(source: AudioSource, mic: Option<&str>) -> Option<String> {
    match (source, mic.unwrap_or_default()) {
        (AudioSource::System, _) | (AudioSource::Mic, "default_output") => {
            Some("@DEFAULT_MONITOR@".into())
        }
        (AudioSource::Mic, "" | "default" | "default_input") => None,
        (AudioSource::Mic, m) => Some(m.strip_prefix("device:").unwrap_or(m).to_owned()),
    }
}

/// One client name per source, so the sound server lists the two streams apart and remembers
/// their state under easyshot's names.
pub(crate) fn client_name(source: AudioSource) -> &'static str {
    match source {
        AudioSource::System => "easyshot-system-sound",
        AudioSource::Mic => "easyshot-microphone",
    }
}

/// A capture stream from the sound server for `source`.
pub(crate) fn pulse_source(source: AudioSource, spec: &RecordSpec) -> Result<gst::Element> {
    let mut b = gst::ElementFactory::make("pulsesrc").property("client-name", client_name(source));
    if let Some(device) = pulse_device(source, spec.mic.as_deref()) {
        b = b.property("device", device);
    }
    b.build().map_err(audio_err)
}

/// One sound track: the capture mixed over live silence, so the track runs on unbroken when the
/// capture stalls or fails, then cut to the recorded timeline and encoded.
pub(crate) struct Track {
    pub source: AudioSource,
    capture: gst::Element,
    pad: gst::Pad,
}

impl Track {
    pub fn add(
        pipeline: &gst::Pipeline,
        mux: &gst::Element,
        timeline: &Arc<Mutex<Timeline>>,
        source: AudioSource,
        capture: gst::Element,
    ) -> Result<Track> {
        let convert = make("audioconvert")?;
        let resample = make("audioresample")?;
        let filler = gst::ElementFactory::make("audiotestsrc")
            .property("is-live", true)
            .property("samplesperbuffer", FILLER_SAMPLES)
            .property_from_str("wave", "silence")
            .build()
            .map_err(audio_err)?;
        let mix = gst::ElementFactory::make("audiomixer")
            .property("latency", MIX_LATENCY)
            .build()
            .map_err(audio_err)?;
        let caps = gst::Caps::builder("audio/x-raw")
            .field("format", "S16LE")
            .field("layout", "interleaved")
            .field("rate", RATE as i32)
            .field("channels", CHANNELS)
            .build();
        let filter = gst::ElementFactory::make("capsfilter")
            .property("caps", caps)
            .build()
            .map_err(audio_err)?;
        let encode = gst::ElementFactory::make("opusenc")
            .property("bitrate", OPUS_BITRATE)
            .build()
            .map_err(audio_err)?;
        let queue = gst::ElementFactory::make("queue")
            .property("max-size-buffers", 0u32)
            .property("max-size-bytes", 0u32)
            .property("max-size-time", 5_000_000_000u64)
            .build()
            .map_err(audio_err)?;
        pipeline
            .add_many([
                &capture, &convert, &resample, &filler, &mix, &filter, &encode, &queue,
            ])
            .map_err(audio_err)?;
        gst::Element::link_many([&capture, &convert, &resample]).map_err(audio_err)?;
        let pad = mix
            .request_pad_simple("sink_%u")
            .ok_or_else(|| Error::Audio("no mixer input".into()))?;
        link(&resample, &pad)?;
        filler.link(&mix).map_err(audio_err)?;
        gst::Element::link_many([&mix, &filter, &encode, &queue]).map_err(audio_err)?;
        let into = mux
            .request_pad_simple("audio_%u")
            .ok_or_else(|| Error::Audio("no muxer input".into()))?;
        link(&queue, &into)?;
        let out = filter
            .static_pad("src")
            .ok_or_else(|| Error::Audio("no mixer output".into()))?;
        let timeline = timeline.clone();
        out.add_probe(gst::PadProbeType::BUFFER, move |pad, info| {
            recut(pad, info, &timeline)
        });
        Ok(Track {
            source,
            capture,
            pad,
        })
    }

    pub fn set_muted(&self, muted: bool) {
        self.pad.set_property("mute", muted);
    }

    /// Clears a mute the sound server keeps for this stream, set outside easyshot or remembered
    /// from an earlier recording.
    pub fn unmute_stream(&self) {
        if self.capture.find_property("mute").is_some() {
            self.capture.set_property("mute", false);
        }
    }

    pub fn posted(&self, msg: &gst::Message) -> bool {
        msg.src().is_some_and(|s| {
            s == self.capture.upcast_ref::<gst::Object>() || s.has_as_ancestor(&self.capture)
        })
    }
}

fn link(element: &gst::Element, into: &gst::Pad) -> Result<()> {
    element
        .static_pad("src")
        .ok_or_else(|| Error::Audio("no output".into()))?
        .link(into)
        .map(|_| ())
        .map_err(audio_err)
}

fn running_time(pad: &gst::Pad, pts: gst::ClockTime) -> Option<u64> {
    let event = pad.sticky_event::<gst::event::Segment>(0)?;
    let segment = event.segment().downcast_ref::<gst::ClockTime>()?;
    segment.to_running_time(pts).map(|t| t.nseconds())
}

/// Waits until the clock has passed running time `end`. A buffer is cut only once every sample
/// in it lies in the past, so a pause, resume or stop marked at clock time `t` applies exactly to
/// the samples after `t`, however early the source handed the buffer over.
fn wait_until_heard(pad: &gst::Pad, end: u64) {
    let Some(element) = pad.parent_element() else {
        return;
    };
    let (Some(clock), Some(base)) = (element.clock(), element.base_time()) else {
        return;
    };
    let ahead = (base.nseconds() + end).saturating_sub(clock.time().nseconds());
    std::thread::sleep(Duration::from_nanos(ahead).min(MAX_WAIT));
}

fn recut(
    pad: &gst::Pad,
    info: &mut gst::PadProbeInfo,
    timeline: &Mutex<Timeline>,
) -> gst::PadProbeReturn {
    let Some(buffer) = info.buffer() else {
        return gst::PadProbeReturn::Ok;
    };
    let Some(at) = buffer.pts().and_then(|t| running_time(pad, t)) else {
        return gst::PadProbeReturn::Drop;
    };
    let (at, len) = (sample_at(at), buffer.size() as u64 / FRAME_BYTES);
    wait_until_heard(pad, time_of(at + len));
    let cut = match timeline.lock() {
        Ok(t) => t.cut(at, len),
        Err(_) => Cut::Drop,
    };
    match cut {
        Cut::Drop => gst::PadProbeReturn::Drop,
        Cut::End => {
            pad.push_event(gst::event::Eos::new());
            gst::PadProbeReturn::Drop
        }
        Cut::Keep { pieces, at: out } => {
            let kept = if pieces.len() == 1 && pieces[0] == (at..at + len) {
                buffer.clone()
            } else {
                let mut joined = gst::Buffer::new();
                for p in &pieces {
                    let bytes = (p.start - at) * FRAME_BYTES..(p.end - at) * FRAME_BYTES;
                    let Ok(part) = buffer.copy_region(
                        gst::BufferCopyFlags::MEMORY,
                        bytes.start as usize..bytes.end as usize,
                    ) else {
                        return gst::PadProbeReturn::Drop;
                    };
                    joined.append(part);
                }
                joined
            };
            let n: u64 = pieces.iter().map(|p| p.end - p.start).sum();
            let mut kept = kept;
            {
                let b = kept.make_mut();
                b.set_pts(gst::ClockTime::from_nseconds(time_of(out)));
                b.set_duration(gst::ClockTime::from_nseconds(
                    time_of(out + n) - time_of(out),
                ));
                b.set_offset(out);
                b.set_offset_end(out + n);
                // A mixed gap is silence the encoder would skip, leaving a hole in the track.
                b.unset_flags(gst::BufferFlags::GAP);
            }
            info.data = Some(gst::PadProbeData::Buffer(kept));
            gst::PadProbeReturn::Ok
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_sound_is_the_default_output_and_the_mic_is_the_one_asked_for() {
        let system = pulse_device(AudioSource::System, Some("usb-mic"));
        assert_eq!(system.as_deref(), Some("@DEFAULT_MONITOR@"));
        for default in [None, Some(""), Some("default"), Some("default_input")] {
            assert_eq!(pulse_device(AudioSource::Mic, default), None);
        }
        assert_eq!(
            pulse_device(AudioSource::Mic, Some("device:alsa_input.usb")).as_deref(),
            Some("alsa_input.usb")
        );
        assert_eq!(
            pulse_device(AudioSource::Mic, Some("alsa_input.usb")).as_deref(),
            Some("alsa_input.usb")
        );
        assert_eq!(
            pulse_device(AudioSource::Mic, Some("default_output")).as_deref(),
            Some("@DEFAULT_MONITOR@")
        );
        assert_ne!(
            client_name(AudioSource::System),
            client_name(AudioSource::Mic)
        );
    }
}
