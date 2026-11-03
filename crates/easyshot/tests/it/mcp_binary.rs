#![cfg(feature = "test-hooks")]
#![allow(clippy::unwrap_used)]
use es_desktop::dbus::{Callbacks, Endpoint, Incoming, Service};
use es_protocol::JobStatus;
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, mpsc};
use std::time::Duration;

const OUTPUTS: &str = r#"{"ok":true,"kind":"outputs","outputs":[{"name":"HEADLESS-1","x":0,"y":0,"width":1280,"height":720,"scale":1,"focused":true}]}"#;

struct Bus {
    child: Child,
    address: String,
    _dir: tempfile::TempDir,
}

impl Bus {
    fn start() -> Bus {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("bus.conf");
        std::fs::write(
            &config,
            format!(
                r#"<busconfig><type>session</type><listen>unix:tmpdir={}</listen><policy context="default"><allow send_destination="*" eavesdrop="true"/><allow eavesdrop="true"/><allow own="*"/></policy></busconfig>"#,
                dir.path().display()
            ),
        )
        .unwrap();
        let mut child = Command::new("dbus-daemon")
            .arg(format!("--config-file={}", config.display()))
            .args(["--nofork", "--print-address=1"])
            .stdout(Stdio::piped())
            .spawn()
            .expect("dbus-daemon is installed");
        let mut address = String::new();
        BufReader::new(child.stdout.take().unwrap())
            .read_line(&mut address)
            .unwrap();
        Bus {
            child,
            address: address.trim().to_owned(),
            _dir: dir,
        }
    }
}

impl Drop for Bus {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn fake_daemon(address: String) -> mpsc::Sender<()> {
    let (ready_tx, ready_rx) = mpsc::channel();
    let (stop, stopped) = mpsc::channel::<()>();
    std::thread::spawn(move || {
        futures::executor::block_on(async {
            let callbacks = Callbacks {
                doctor: Arc::new(|| Box::pin(async { String::new() })),
                dump: Arc::new(|| Box::pin(async { String::new() })),
            };
            let endpoint = Endpoint::test().on_address(address);
            let svc = Service::start(endpoint, callbacks).await.unwrap();
            let incoming = svc.incoming();
            ready_tx.send(()).unwrap();
            if let Ok(Incoming::Command { job, .. }) = incoming.recv().await {
                svc.finish(job, JobStatus::Ok, OUTPUTS).await.unwrap();
            }
            let _ = stopped.recv();
        })
    });
    ready_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    stop
}

fn jsonrpc(line: &str) -> Value {
    let message: Value = serde_json::from_str(line).unwrap_or_else(|e| {
        panic!("standard output carried something other than JSON: {line:?}: {e}")
    });
    assert_eq!(message["jsonrpc"], "2.0", "{line}");
    message
}

#[test]
fn the_built_binary_serves_mcp_with_only_json_rpc_on_standard_output() {
    let bus = Bus::start();
    let stop = fake_daemon(bus.address.clone());
    let mut child = Command::new(env!("CARGO_BIN_EXE_easyshot"))
        .args(["agent", "mcp"])
        .env("DBUS_SESSION_BUS_ADDRESS", &bus.address)
        .env("EASYSHOT_TEST", "1")
        .env("RUST_LOG", "trace")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let (lines_tx, lines) = mpsc::channel();
    let stdout = child.stdout.take().unwrap();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            if lines_tx.send(line).is_err() {
                break;
            }
        }
    });
    let mut seen = Vec::new();
    let mut ask = |message: Value| {
        writeln!(stdin, "{message}").unwrap();
        let id = message["id"].clone();
        if id.is_null() {
            return Value::Null;
        }
        loop {
            let line = lines
                .recv_timeout(Duration::from_secs(20))
                .expect("an answer");
            let answer = jsonrpc(&line);
            seen.push(line);
            if answer["id"] == id {
                return answer;
            }
        }
    };

    let init = ask(
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
        "protocolVersion": "2025-06-18", "capabilities": {},
        "clientInfo": {"name": "test", "version": "0"}}}),
    );
    assert_eq!(init["result"]["serverInfo"]["name"], "easyshot");
    ask(json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
    let tools = ask(json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}}));
    let names: Vec<_> = tools["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_owned())
        .collect();
    assert!(names.contains(&"capture".to_owned()) && names.contains(&"annotate".to_owned()));
    let outputs = ask(json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call",
        "params": {"name": "list_outputs", "arguments": {}}}));
    assert_eq!(
        outputs["result"]["structuredContent"]["outputs"][0]["name"],
        "HEADLESS-1"
    );

    drop(stdin);
    let _ = child.wait();
    for line in lines.iter() {
        jsonrpc(&line);
        seen.push(line);
    }
    let _ = stop.send(());
    assert!(seen.len() >= 3, "{seen:?}");
}
