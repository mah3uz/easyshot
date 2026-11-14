use gstreamer as gst;

const CORE: &str = "gstreamer";
const BASE: &str = "gst-plugins-base-libs";
const BASE_EXTRA: &str = "gst-plugins-base";
const GOOD: &str = "gst-plugins-good";
const BAD: &str = "gst-plugins-bad-libs";
const UGLY: &str = "gst-plugins-ugly";
const LIBAV: &str = "gst-libav";

/// Any one of `any` satisfies the need; `package` is the Arch package that ships them.
struct Need {
    any: &'static [&'static str],
    package: &'static str,
}

const fn need(any: &'static [&'static str], package: &'static str) -> Need {
    Need { any, package }
}

const PLAYBACK: &[Need] = &[
    need(&["uridecodebin"], BASE),
    need(&["videobalance"], GOOD),
    need(&["videoconvert"], BASE),
    need(&["videoscale"], BASE),
    need(&["capsfilter"], CORE),
    need(&["appsink"], BASE),
    need(&["audiomixer"], BASE),
    need(&["audioconvert"], BASE),
    need(&["audioresample"], BASE),
    need(&["volume"], BASE),
    need(&["qtdemux"], GOOD),
    need(&["matroskademux"], GOOD),
    need(&["avdec_h264", "vah264dec", "openh264dec"], LIBAV),
];

const WINDOW_RECORDING: &[Need] = &[
    need(&["appsrc"], BASE),
    need(&["videoscale"], BASE),
    need(&["videoconvert"], BASE),
    need(&["queue"], CORE),
    need(&["h264parse"], BAD),
    need(&["matroskamux"], GOOD),
    need(&["filesink"], CORE),
    need(&["pulsesrc"], GOOD),
    need(&["audioconvert"], BASE),
    need(&["audioresample"], BASE),
    need(&["audiotestsrc"], BASE),
    need(&["audiomixer"], BASE),
    need(&["opusenc"], BASE_EXTRA),
    need(&["nvh264enc", "vah264enc", "x264enc"], UGLY),
];

/// A GStreamer element the registry does not have, and the package that provides it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Gap {
    pub element: &'static str,
    pub package: &'static str,
}

fn gaps(needs: &[Need], has: &dyn Fn(&str) -> bool) -> Vec<Gap> {
    needs
        .iter()
        .filter(|n| !n.any.iter().any(|e| has(e)))
        .map(|n| Gap {
            element: n.any[0],
            package: n.package,
        })
        .collect()
}

pub fn in_registry(element: &str) -> bool {
    gst::init().is_ok() && gst::ElementFactory::find(element).is_some()
}

/// What the video player is missing, judged by `has`.
pub fn playback_gaps(has: &dyn Fn(&str) -> bool) -> Vec<Gap> {
    gaps(PLAYBACK, has)
}

/// What recording a window from its own frames is missing, judged by `has`.
pub fn window_recording_gaps(has: &dyn Fn(&str) -> bool) -> Vec<Gap> {
    gaps(WINDOW_RECORDING, has)
}

/// The distinct packages of `gaps`, in order.
pub fn packages(gaps: &[Gap]) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for g in gaps {
        if !out.contains(&g.package) {
            out.push(g.package);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_videobalance_points_at_the_good_plugins() {
        let gaps = playback_gaps(&|e| e != "videobalance");
        assert_eq!(
            gaps,
            [Gap {
                element: "videobalance",
                package: "gst-plugins-good"
            }]
        );
    }

    #[test]
    fn any_one_h264_decoder_is_enough_and_none_points_at_gst_libav() {
        assert!(playback_gaps(&|e| e != "avdec_h264").is_empty());
        let gaps = playback_gaps(&|e| !["avdec_h264", "vah264dec", "openh264dec"].contains(&e));
        assert_eq!(packages(&gaps), ["gst-libav"]);
    }

    #[test]
    fn a_bare_install_names_each_package_once() {
        let names = packages(&playback_gaps(&|_| false));
        assert!(names.contains(&"gst-plugins-good") && names.contains(&"gst-libav"));
        let distinct: std::collections::HashSet<_> = names.iter().collect();
        assert_eq!(names.len(), distinct.len());
    }

    #[test]
    fn window_recording_needs_an_encoder_and_the_h264_parser() {
        let gaps = window_recording_gaps(&|e| {
            !matches!(e, "x264enc" | "nvh264enc" | "vah264enc" | "h264parse")
        });
        assert_eq!(
            packages(&gaps),
            ["gst-plugins-bad-libs", "gst-plugins-ugly"]
        );
        assert!(window_recording_gaps(&|e| e != "nvh264enc").is_empty());
    }

    #[test]
    fn every_element_the_window_recorder_builds_is_listed() {
        let listed: Vec<&str> = WINDOW_RECORDING
            .iter()
            .flat_map(|n| n.any)
            .copied()
            .collect();
        for e in crate::window::required_elements() {
            assert!(listed.contains(&e), "{e} is not in the install list");
        }
    }
}
