use super::mcp::{Server, run_on};
use super::mcp_tools::{self, ToolDef};
use crate::cli::{Cli, Request};
use crate::run_tests::{PrivateBus, env};
use base64::Engine;
use clap::Parser;
use es_desktop::dbus::{Callbacks, Endpoint, Incoming, Service};
use es_protocol::agent::{AgentRoute, AgentTarget, WindowSelector};
use es_protocol::{JobStatus, Route};
use serde_json::{Value, json};
use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, DuplexStream, ReadHalf, WriteHalf};

const OUTPUTS: &str = r#"{"ok":true,"kind":"outputs","outputs":[{"name":"HEADLESS-1","x":0,"y":0,"width":1280,"height":720,"scale":1,"focused":true}]}"#;
const ACCESS_OFF: &str = r#"{"ok":false,"kind":"capture","error":{"code":"access_off","message":"agent access is off","hint":"Agent access is off. Turn it on in Settings › Agents, or run `easyshot agent enable`."}}"#;

fn args(value: &Value) -> serde_json::Map<String, Value> {
    value.as_object().cloned().unwrap_or_default()
}

fn def(name: &str) -> &'static ToolDef {
    mcp_tools::find(name).unwrap_or_else(|| panic!("no tool {name}"))
}

fn route_of(name: &str, arguments: &Value) -> Result<(AgentRoute, bool), String> {
    let (call, include_image) = super::mcp::prepare(def(name), &args(arguments))?;
    Ok((call.route, include_image))
}

#[test]
fn every_tool_schema_is_a_closed_object_whose_names_and_enums_make_sense() {
    for tool in mcp_tools::tools() {
        let schema = Value::Object((*tool.input_schema).clone());
        assert_eq!(schema["type"], "object", "{}", tool.name);
        assert_eq!(schema["additionalProperties"], false, "{}", tool.name);
        let properties = schema["properties"].as_object().expect("properties");
        for required in schema["required"].as_array().into_iter().flatten() {
            let name = required.as_str().expect("a name");
            assert!(properties.contains_key(name), "{}: {name}", tool.name);
        }
        for (name, p) in properties {
            let kind = p["type"].as_str().unwrap_or_default();
            assert!(
                ["string", "boolean", "integer", "number", "array", "object"].contains(&kind),
                "{}.{name}: {kind}",
                tool.name
            );
            for choice in p["enum"].as_array().into_iter().flatten() {
                let matches = match kind {
                    "string" => choice.is_string(),
                    _ => choice.is_number(),
                };
                assert!(matches, "{}.{name}: {choice}", tool.name);
            }
            if let (Some(min), Some(max)) = (p.get("minimum"), p.get("maximum")) {
                assert!(min.as_f64() < max.as_f64(), "{}.{name}", tool.name);
            }
        }
        let annotations = tool.annotations.as_ref().expect("annotations");
        assert_eq!(
            annotations.read_only_hint,
            Some(tool.name.starts_with("list_")),
            "{}",
            tool.name
        );
    }
}

#[test]
fn a_tool_call_becomes_the_route_the_command_line_would_send() {
    let (route, include_image) = route_of(
        "capture",
        &json!({"window": "-weird title", "include_image": false}),
    )
    .unwrap();
    assert!(!include_image);
    let AgentRoute::Capture(spec) = route else {
        panic!("not a capture: {route:?}");
    };
    assert_eq!(
        spec.target,
        AgentTarget::Window(WindowSelector::any("-weird title").unwrap())
    );
    let (route, _) = route_of("export", &json!({"input": "/v/a.mkv", "brightness": -0.2})).unwrap();
    let AgentRoute::Export(spec) = route else {
        panic!("not an export: {route:?}");
    };
    assert_eq!(spec.brightness_milli, -200);
    let (route, _) = route_of(
        "record_start",
        &json!({"fullscreen": true, "max_seconds": 5}),
    )
    .unwrap();
    assert!(matches!(route, AgentRoute::Record(_)));
    assert_eq!(
        route_of("list_windows", &json!({})).unwrap().0,
        AgentRoute::ListWindows
    );
}

#[test]
fn bad_arguments_are_refused_before_the_daemon_is_asked() {
    for (tool, arguments) in [
        ("capture", json!({})),
        ("capture", json!({"output": "DP-1", "window": "foot"})),
        ("capture", json!({"fullscreen": true, "shell": "rm"})),
        ("capture", json!({"fullscreen": "yes"})),
        ("capture", json!({"geometry": "10 by 10"})),
        ("capture", json!({"fullscreen": true, "delay": 61})),
        ("capture", json!({"fullscreen": true, "include_image": 1})),
        ("ocr", json!({"file": "/tmp/notes.txt"})),
        ("export", json!({})),
        ("export", json!({"input": "/v/a.mkv", "brightness": 0.9})),
        ("list_outputs", json!({"output": "DP-1"})),
    ] {
        let refused = route_of(tool, &arguments);
        assert!(refused.is_err(), "{tool} {arguments}: {refused:?}");
    }
}

#[test]
fn a_capture_is_quiet_unless_the_agent_asks_for_the_card_with_ui() {
    let ui = |arguments: Value| match route_of("capture", &arguments).unwrap().0 {
        AgentRoute::Capture(spec) => spec.ui,
        other => panic!("not a capture: {other:?}"),
    };
    assert!(!ui(json!({"fullscreen": true})));
    assert!(!ui(json!({"fullscreen": true, "ui": false})));
    assert!(ui(json!({"fullscreen": true, "ui": true})));
}

fn fake_daemon(
    endpoint: Endpoint,
    jobs: usize,
    respond: impl Fn(&Route) -> (JobStatus, String) + Send + 'static,
) -> (std::thread::JoinHandle<Vec<Route>>, mpsc::Sender<()>) {
    let (ready_tx, ready_rx) = mpsc::channel();
    let (stop, stopped) = mpsc::channel::<()>();
    let handle = std::thread::spawn(move || {
        futures::executor::block_on(async {
            let callbacks = Callbacks {
                doctor: Arc::new(|| Box::pin(async { String::new() })),
                dump: Arc::new(|| Box::pin(async { String::new() })),
            };
            let svc = Service::start(endpoint, callbacks).await.unwrap();
            let incoming = svc.incoming();
            ready_tx.send(()).unwrap();
            let mut seen = Vec::new();
            for _ in 0..jobs {
                let Incoming::Command { job, route, .. } = incoming.recv().await.unwrap() else {
                    panic!("expected a command");
                };
                let (status, result) = respond(&route);
                seen.push(route);
                svc.finish(job, status, &result).await.unwrap();
            }
            let _ = stopped.recv();
            seen
        })
    });
    ready_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    (handle, stop)
}

struct Client {
    write: WriteHalf<DuplexStream>,
    read: BufReader<ReadHalf<DuplexStream>>,
    next: u64,
}

impl Client {
    async fn send(&mut self, message: Value) {
        let line = format!("{message}\n");
        self.write.write_all(line.as_bytes()).await.unwrap();
    }

    async fn request(&mut self, method: &str, params: Value) -> Value {
        self.next += 1;
        let id = self.next;
        self.send(json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}))
            .await;
        loop {
            let mut line = String::new();
            let read =
                tokio::time::timeout(Duration::from_secs(20), self.read.read_line(&mut line))
                    .await
                    .expect("the server answers")
                    .unwrap();
            assert!(read > 0, "the server closed the stream");
            let message: Value =
                serde_json::from_str(line.trim()).expect("one JSON message a line");
            if message["id"] == id {
                return message;
            }
        }
    }

    async fn call(&mut self, tool: &str, arguments: Value) -> Value {
        let reply = self
            .request("tools/call", json!({"name": tool, "arguments": arguments}))
            .await;
        reply["result"].clone()
    }
}

fn start_client(endpoint: Endpoint) -> (Client, tokio::task::JoinHandle<Result<(), String>>) {
    let (near, far) = tokio::io::duplex(1 << 22);
    let server = Server::new(endpoint, false, Duration::from_millis(300));
    let task = tokio::spawn(run_on(server, far));
    let (read, write) = tokio::io::split(near);
    let client = Client {
        write,
        read: BufReader::new(read),
        next: 0,
    };
    (client, task)
}

fn image_of(result: &Value) -> image::DynamicImage {
    let item = result["content"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["type"] == "image")
        .expect("an image item");
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(item["data"].as_str().unwrap())
        .unwrap();
    image::load_from_memory(&bytes).unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_stdio_session_lists_tools_and_returns_a_small_image_and_the_daemons_errors() {
    let bus = PrivateBus::start();
    let dir = tempfile::tempdir().unwrap();
    let big = dir.path().join("big.png");
    image::RgbaImage::from_fn(3000, 1500, |x, y| {
        image::Rgba([(x % 251) as u8, (y % 241) as u8, 90, 255])
    })
    .save(&big)
    .unwrap();
    let capture = json!({
        "ok": true, "kind": "capture", "path": big, "width": 3000, "height": 1500,
        "scale": 1.0, "redaction": {"applied": true, "regions": 2}
    })
    .to_string();
    let (daemon, stop) = fake_daemon(
        env(&bus, dir.path()).endpoint,
        4,
        move |route| match route {
            Route::Agent(AgentRoute::ListOutputs) => (JobStatus::Ok, OUTPUTS.to_owned()),
            Route::Agent(AgentRoute::Capture(spec))
                if matches!(spec.target, AgentTarget::Window(_)) =>
            {
                (JobStatus::Ok, ACCESS_OFF.to_owned())
            }
            _ => (JobStatus::Ok, capture.clone()),
        },
    );
    let (mut client, task) = start_client(env(&bus, dir.path()).endpoint);

    let init = client
        .request(
            "initialize",
            json!({"protocolVersion": "2025-06-18", "capabilities": {},
                   "clientInfo": {"name": "test", "version": "0"}}),
        )
        .await;
    assert_eq!(init["result"]["serverInfo"]["name"], "easyshot");
    client
        .send(json!({"jsonrpc": "2.0", "method": "notifications/initialized"}))
        .await;

    let listed = client.request("tools/list", json!({})).await;
    let names: Vec<_> = listed["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(
        names,
        [
            "list_outputs",
            "list_windows",
            "capture",
            "ocr",
            "record_start",
            "record_stop",
            "export",
            "annotate"
        ]
    );
    assert_eq!(
        listed["result"]["tools"][0]["annotations"]["readOnlyHint"],
        true
    );

    let outputs = client.call("list_outputs", json!({})).await;
    assert_ne!(outputs["isError"], true);
    assert_eq!(
        outputs["structuredContent"]["outputs"][0]["name"],
        "HEADLESS-1"
    );

    let shot = client.call("capture", json!({"fullscreen": true})).await;
    assert_ne!(shot["isError"], true, "{shot}");
    let preview = image_of(&shot);
    assert_eq!((preview.width(), preview.height()), (1280, 640));
    let text: Value = serde_json::from_str(shot["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(text["path"], big.to_str().unwrap());
    assert_eq!(text["width"], 3000);
    assert_eq!(text["redaction"], json!({"applied": true, "regions": 2}));
    assert_eq!(shot["structuredContent"]["preview"]["width"], 1280);

    let bare = client
        .call(
            "capture",
            json!({"fullscreen": true, "include_image": false}),
        )
        .await;
    assert!(
        bare["content"]
            .as_array()
            .unwrap()
            .iter()
            .all(|c| c["type"] != "image")
    );

    let refused = client.call("capture", json!({"window": "foot"})).await;
    assert_eq!(refused["isError"], true);
    let message = refused["content"][0]["text"].as_str().unwrap();
    assert!(message.contains("access_off"), "{message}");
    assert_eq!(
        message,
        "access_off: agent access is off. Agent access is off. Turn it on in Settings › Agents, \
         or run `easyshot agent enable`."
    );
    assert_eq!(refused["structuredContent"]["error"]["code"], "access_off");

    let invalid = client.call("capture", json!({})).await;
    assert_eq!(invalid["isError"], true);

    drop(client);
    let _ = tokio::time::timeout(Duration::from_secs(5), task).await;
    let _ = stop.send(());
    let seen = daemon.join().unwrap();
    assert_eq!(seen.len(), 4);
    assert!(matches!(seen[0], Route::Agent(AgentRoute::ListOutputs)));
    assert!(
        seen[1..]
            .iter()
            .all(|r| matches!(r, Route::Agent(AgentRoute::Capture(_))))
    );
}

#[test]
fn the_server_command_builds_no_route_of_its_own() {
    let cli = Cli::try_parse_from(["easyshot", "agent", "mcp"]).unwrap();
    assert_eq!(cli.request().unwrap(), Request::AgentMcp);
}

async fn initialised(client: &mut Client) {
    client
        .request(
            "initialize",
            json!({"protocolVersion": "2025-06-18", "capabilities": {},
                   "clientInfo": {"name": "test", "version": "0"}}),
        )
        .await;
    client
        .send(json!({"jsonrpc": "2.0", "method": "notifications/initialized"}))
        .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn annotate_draws_on_a_file_without_the_daemon_and_returns_a_preview() {
    let bus = PrivateBus::start();
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("in.png");
    image::RgbaImage::from_pixel(2000, 1000, image::Rgba([240, 240, 240, 255]))
        .save(&input)
        .unwrap();
    let out = dir.path().join("out.png");
    let (mut client, _task) = start_client(env(&bus, dir.path()).endpoint);
    initialised(&mut client).await;

    let ops = json!([
        {"op": "rect", "x": 100, "y": 100, "w": 400, "h": 300, "color": "#ff0000", "width": 12},
        {"op": "counter", "x": 600, "y": 200}
    ]);
    let done = client
        .call("annotate", json!({"image": input, "out": out, "ops": ops}))
        .await;
    assert_ne!(done["isError"], true, "{done}");
    let body = &done["structuredContent"];
    assert_eq!(
        (
            body["kind"].as_str(),
            body["width"].as_u64(),
            body["ops"].as_u64()
        ),
        (Some("annotate"), Some(2000), Some(2))
    );
    assert_eq!(body["path"], json!(out));
    let preview = image_of(&done);
    assert_eq!((preview.width(), preview.height()), (1280, 640));
    let written = image::open(&out).unwrap().to_rgba8();
    assert_eq!(written.dimensions(), (2000, 1000));
    let edge = written.get_pixel(100, 250);
    assert!(edge[0] > 200 && edge[1] < 80, "{edge:?}");

    let again = client
        .call("annotate", json!({"image": input, "out": out, "ops": ops}))
        .await;
    assert_eq!(again["isError"], true);
    let text = again["content"][0]["text"].as_str().unwrap();
    assert!(
        text.starts_with("invalid_arguments:") && text.contains("force"),
        "{text}"
    );

    let bad = client
        .call(
            "annotate",
            json!({"image": input, "out": dir.path().join("b.png"),
                   "ops": [{"op": "rect", "x": 1, "y": 1, "w": 0, "h": 5}]}),
        )
        .await;
    assert_eq!(bad["isError"], true);
    assert!(
        bad["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("ops[0]")
    );
    let relative = client
        .call(
            "annotate",
            json!({"image": "in.png", "out": out, "ops": []}),
        )
        .await;
    assert_eq!(relative["isError"], true);
}

#[test]
fn a_refusal_is_the_message_and_its_hint_only_when_there_is_one() {
    use es_protocol::agent::{AgentReply, ErrorCode, ReplyKind};
    let text = |reply: &AgentReply| {
        let refused = super::mcp::refusal(reply).unwrap();
        serde_json::to_value(&refused).unwrap()["content"][0]["text"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    let plain = AgentReply::failed(ReplyKind::Capture, ErrorCode::NotFound, "no window matches");
    assert_eq!(text(&plain), "not_found: no window matches.");
    let hinted = plain.hinted(Some("Open the window first.".into()));
    assert_eq!(
        text(&hinted),
        "not_found: no window matches. Open the window first."
    );
}
