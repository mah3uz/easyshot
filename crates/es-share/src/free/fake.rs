use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

const KEEP_BODY: usize = 256 * 1024;

#[derive(Debug, Clone)]
pub(crate) struct Seen {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    pub body_len: usize,
}

impl Seen {
    pub fn header(&self, name: &str) -> Option<&str> {
        let name = name.to_ascii_lowercase();
        self.headers
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, v)| v.as_str())
    }

    pub fn json(&self) -> Value {
        serde_json::from_slice(&self.body).expect("JSON body")
    }

    pub fn is(&self, method: &str, path: &str) -> bool {
        self.method == method && self.path == path
    }
}

pub(crate) enum Answer {
    Reply(u16, Option<Value>),
    Bytes(u16, Vec<u8>),
    Drop,
}

type Handler = dyn Fn(&Seen) -> Answer + Send + Sync;

pub(crate) struct Fake {
    pub base: String,
    seen: Arc<Mutex<Vec<Seen>>>,
}

impl Fake {
    pub fn start(handler: impl Fn(&Seen) -> Answer + Send + Sync + 'static) -> Fake {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let base = format!("http://{}", listener.local_addr().expect("addr"));
        let seen = Arc::new(Mutex::new(Vec::new()));
        let handler: Arc<Handler> = Arc::new(handler);
        let log = seen.clone();
        thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { return };
                let (log, handler) = (log.clone(), handler.clone());
                thread::spawn(move || serve(stream, &log, &*handler));
            }
        });
        Fake { base, seen }
    }

    pub fn seen(&self) -> Vec<Seen> {
        self.seen.lock().expect("log").clone()
    }

    pub fn contract() -> Fake {
        Fake::start(contract_answer)
    }
}

fn serve(stream: TcpStream, log: &Mutex<Vec<Seen>>, handler: &Handler) {
    let mut writer = stream.try_clone().expect("clone");
    let mut reader = BufReader::new(stream);
    while let Some(seen) = read_request(&mut reader) {
        log.lock().expect("log").push(seen.clone());
        let (status, body) = match handler(&seen) {
            Answer::Drop => return,
            Answer::Reply(status, body) => (
                status,
                body.map(|b| b.to_string().into_bytes()).unwrap_or_default(),
            ),
            Answer::Bytes(status, body) => (status, body),
        };
        let mut head = format!("HTTP/1.1 {status} X\r\n");
        if status != 204 {
            head.push_str(&format!(
                "Content-Type: application/json\r\nContent-Length: {}\r\n",
                body.len()
            ));
        }
        head.push_str("\r\n");
        if writer.write_all(head.as_bytes()).is_err() || writer.write_all(&body).is_err() {
            return;
        }
    }
}

fn read_request(reader: &mut BufReader<TcpStream>) -> Option<Seen> {
    let mut line = String::new();
    if reader.read_line(&mut line).ok()? == 0 {
        return None;
    }
    let mut parts = line.split_whitespace();
    let method = parts.next()?.to_owned();
    let path = parts.next()?.to_owned();
    let mut headers = Vec::new();
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).ok()?;
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        let (name, value) = line.split_once(':')?;
        headers.push((name.trim().to_ascii_lowercase(), value.trim().to_owned()));
    }
    assert!(
        !headers.iter().any(|(n, _)| n == "transfer-encoding"),
        "uploads must carry a Content-Length, not a chunked body"
    );
    let len: usize = headers
        .iter()
        .find(|(n, _)| n == "content-length")
        .map_or(0, |(_, v)| v.parse().expect("content-length"));
    let mut body = Vec::new();
    let mut left = len;
    let mut buf = [0u8; 64 * 1024];
    while left > 0 {
        let n = reader.read(&mut buf[..left.min(64 * 1024)]).ok()?;
        if n == 0 {
            return None;
        }
        if body.len() < KEEP_BODY {
            body.extend_from_slice(&buf[..n]);
        }
        left -= n;
    }
    Some(Seen {
        method,
        path,
        headers,
        body,
        body_len: len,
    })
}

pub(super) fn exchange(name: &str) -> Value {
    let doc: Value =
        serde_json::from_str(include_str!("../../../../services/share/contract/api.json"))
            .expect("api.json parses");
    doc["exchanges"]
        .as_array()
        .expect("exchanges")
        .iter()
        .find(|e| e["name"] == name)
        .unwrap_or_else(|| panic!("no exchange {name}"))
        .clone()
}

fn response(name: &str) -> Answer {
    let ex = exchange(name);
    let status = ex["response"]["status"].as_u64().expect("status") as u16;
    Answer::Reply(status, ex["response"].get("body").cloned())
}

pub(super) fn error(status: u16, code: &str) -> Answer {
    Answer::Reply(
        status,
        Some(json!({ "error": { "code": code, "message": "for logs" } })),
    )
}

pub(super) fn contract_answer(seen: &Seen) -> Answer {
    let image_id = "/v1/shares/7Kp2mX9qR4tY8wZ1aB3cD5";
    let parts: Vec<&str> = seen.path.split('/').collect();
    match (seen.method.as_str(), parts.as_slice()) {
        ("GET", ["", "v1", "challenge"]) => response("challenge"),
        ("POST", ["", "v1", "install"]) => response("install"),
        ("GET", ["", "v1", "limits"]) => response("limits"),
        ("POST", ["", "v1", "shares"]) if seen.json()["kind"] == "video" => {
            response("create_video")
        }
        ("POST", ["", "v1", "shares"]) => response("create_image"),
        ("PUT", ["", "v1", "shares", _, "parts", n]) => Answer::Reply(
            200,
            Some(json!({ "part": n.parse::<u64>().expect("part") })),
        ),
        ("PUT", ["", "v1", "shares", _, "poster"]) => response("poster"),
        ("POST", ["", "v1", "shares", _, "complete"]) if seen.path.starts_with(image_id) => {
            let created = &exchange("create_image")["response"]["body"];
            let keys = ["page_url", "media_url", "poster_url", "expires_at"];
            let body: serde_json::Map<String, Value> = keys
                .iter()
                .map(|k| ((*k).to_owned(), created[*k].clone()))
                .collect();
            Answer::Reply(200, Some(Value::Object(body)))
        }
        ("POST", ["", "v1", "shares", _, "complete"]) => response("complete"),
        ("DELETE", ["", "v1", "shares", _]) => Answer::Reply(204, None),
        _ => error(404, "not_found"),
    }
}
