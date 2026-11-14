use super::*;
use crate::daemon::agent::gate::tests::{GateRig, ME};
use es_app::events::Origin;
use es_geom::Rect;
use es_model::platform::{OutputId, Transform, WindowId};
use gpui::TestAppContext;
use serde_json::{Value, json};
use std::sync::atomic::{AtomicUsize, Ordering};

fn output(name: &str, description: &str, logical: Rect, focused: Option<bool>) -> OutputInfo {
    OutputInfo {
        id: OutputId(name.into()),
        description: description.into(),
        logical,
        physical: (0, 0),
        scale: 1.5,
        transform: Transform::Normal,
        refresh_mhz: 60_000,
        focused,
    }
}

fn window(
    id: &str,
    app_id: &str,
    title: &str,
    logical: Option<Rect>,
    rank: Option<u32>,
) -> WindowInfo {
    WindowInfo {
        id: WindowId(id.into()),
        app_id: app_id.into(),
        title: title.into(),
        pid: Some(42),
        logical,
        output: logical.map(|_| OutputId("DP-1".into())),
        workspace: Some("1".into()),
        floating: false,
        focus_rank: rank,
    }
}

struct Rig {
    sources: Sources,
    calls: Arc<AtomicUsize>,
}

fn rig(caps: Caps) -> Rig {
    let calls = Arc::new(AtomicUsize::new(0));
    let (outputs_calls, windows_calls) = (calls.clone(), calls.clone());
    Rig {
        sources: Sources {
            caps,
            outputs: Arc::new(move || {
                outputs_calls.fetch_add(1, Ordering::SeqCst);
                vec![
                    output(
                        "DP-1",
                        "Dell U2720Q",
                        Rect::new(0., 0., 2560., 1440.),
                        Some(true),
                    ),
                    output("HDMI-A-1", "", Rect::new(-1280.4, 0., 0., 720.), None),
                ]
            }),
            windows: Arc::new(move || {
                windows_calls.fetch_add(1, Ordering::SeqCst);
                vec![
                    window(
                        "0xa1",
                        "foot",
                        "~/src",
                        Some(Rect::new(10., 20., 810., 620.)),
                        Some(0),
                    ),
                    window("0xb2", "", "", None, Some(3)),
                    window("0xc3", "mpv", "clip", None, None),
                ]
            }),
        },
        calls,
    }
}

fn ask(
    rig: &Rig,
    route: AgentRoute,
    origin: Origin,
    cx: &mut TestAppContext,
) -> (JobStatus, Value) {
    let gate = GateRig::permissive(cx);
    gate.ask(&rig.sources, route, origin, Some(ME), cx)
}

#[gpui::test]
fn list_outputs_reports_each_output_in_logical_pixels(cx: &mut TestAppContext) {
    let rig = rig(Caps::all());
    let (done, reply) = ask(&rig, AgentRoute::ListOutputs, Origin::DBus, cx);
    assert_eq!(done, JobStatus::Ok);
    assert_eq!(
        reply,
        json!({
            "ok": true,
            "kind": "outputs",
            "outputs": [
                {"name": "DP-1", "description": "Dell U2720Q", "x": 0, "y": 0,
                 "width": 2560, "height": 1440, "scale": 1.5, "focused": true},
                {"name": "HDMI-A-1", "x": -1280, "y": 0, "width": 1280, "height": 720, "scale": 1.5}
            ]
        })
    );
}

#[gpui::test]
fn list_windows_leaves_out_what_the_compositor_did_not_say(cx: &mut TestAppContext) {
    let rig = rig(Caps::all());
    let (done, reply) = ask(&rig, AgentRoute::ListWindows, Origin::DBus, cx);
    assert_eq!(done, JobStatus::Ok);
    assert_eq!(
        reply,
        json!({
            "ok": true,
            "kind": "windows",
            "windows": [
                {"id": "0xa1", "app_id": "foot", "title": "~/src", "x": 10, "y": 20,
                 "width": 800, "height": 600, "output": "DP-1", "focused": true, "visible": true},
                {"id": "0xb2", "focused": false, "visible": false},
                {"id": "0xc3", "app_id": "mpv", "title": "clip", "visible": false}
            ]
        })
    );
}

#[gpui::test]
fn a_desktop_without_the_provider_answers_unsupported(cx: &mut TestAppContext) {
    let rig = rig(Caps::empty());
    let needs_desktop = AgentRoute::ALL
        .into_iter()
        .filter(|r| !matches!(r, AgentRoute::Export(_)));
    for route in needs_desktop {
        let (done, reply) = ask(&rig, route, Origin::DBus, cx);
        assert_eq!(done, JobStatus::Error);
        assert_eq!(reply["ok"], json!(false));
        assert_eq!(reply["error"]["code"], json!("unsupported"));
    }
    assert_eq!(rig.calls.load(Ordering::SeqCst), 0);
}

#[gpui::test]
fn the_gate_refuses_agent_routes_from_anywhere_but_the_bus_before_any_work(
    cx: &mut TestAppContext,
) {
    let rig = rig(Caps::all());
    for origin in [
        Origin::Tray,
        Origin::Shortcut,
        Origin::Url,
        Origin::Internal,
    ] {
        for route in AgentRoute::ALL {
            let (done, reply) = ask(&rig, route, origin, cx);
            assert_eq!(done, JobStatus::Error, "{origin:?}");
            assert_eq!(reply["error"]["code"], json!("access_denied"), "{origin:?}");
        }
    }
    assert_eq!(
        rig.calls.load(Ordering::SeqCst),
        0,
        "nothing was listed for a refused request"
    );
}

#[gpui::test]
fn export_needs_no_desktop_provider_and_answers_in_its_own_kind(cx: &mut TestAppContext) {
    let rig = rig(Caps::empty());
    let route = AgentRoute::Export(es_protocol::agent::ExportSpec::new(
        "/no/such/dir/video.mp4".into(),
    ));
    let (done, reply) = ask(&rig, route, Origin::DBus, cx);
    assert_eq!(done, JobStatus::Error);
    assert_eq!(reply["kind"], json!("export"));
    assert_eq!(reply["error"]["code"], json!("not_found"));
}
