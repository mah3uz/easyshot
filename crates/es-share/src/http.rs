use crate::error::{Code, ShareError};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use std::io::Read;
use std::time::Duration;
use ureq::{Agent, SendBody};

const REPLY_LIMIT: u64 = 64 * 1024;

pub(crate) struct Http {
    agent: Agent,
    base: String,
    version: String,
    user_agent: String,
}

pub(crate) struct Reply {
    pub(crate) status: u16,
    body: Vec<u8>,
}

impl Http {
    pub(crate) fn new(base: &str, version: &str, https_only: bool) -> Self {
        let agent = Agent::config_builder()
            .https_only(https_only)
            .http_status_as_error(false)
            // A redirect would carry an upload body or a token to a host the protocol never names.
            .max_redirects(0)
            .max_redirects_will_error(false)
            .timeout_resolve(Some(Duration::from_secs(15)))
            .timeout_connect(Some(Duration::from_secs(15)))
            .timeout_send_request(Some(Duration::from_secs(30)))
            // One part is 25 MiB; this lets it through at about 350 kbit/s.
            .timeout_send_body(Some(Duration::from_secs(600)))
            .timeout_recv_response(Some(Duration::from_secs(60)))
            .timeout_recv_body(Some(Duration::from_secs(30)))
            .build()
            .new_agent();
        Self {
            agent,
            base: base.trim_end_matches('/').to_owned(),
            version: version.to_owned(),
            user_agent: format!("easyshot/{version}"),
        }
    }

    pub(crate) fn get(&self, path: &str, bearer: Option<&str>) -> Result<Reply, ShareError> {
        let req = self.agent.get(self.url(path));
        let req = self.headers(req, bearer);
        reply(req.call())
    }

    pub(crate) fn delete(&self, path: &str, bearer: &str) -> Result<Reply, ShareError> {
        let req = self.agent.delete(self.url(path));
        reply(self.headers(req, Some(bearer)).call())
    }

    pub(crate) fn post(
        &self,
        path: &str,
        bearer: Option<&str>,
        json: Option<&serde_json::Value>,
    ) -> Result<Reply, ShareError> {
        let req = self.headers(self.agent.post(self.url(path)), bearer);
        match json {
            Some(value) => {
                let bytes =
                    serde_json::to_vec(value).map_err(|e| ShareError::Protocol(e.to_string()))?;
                reply(
                    req.header("Content-Type", "application/json")
                        .send(&bytes[..]),
                )
            }
            None => reply(req.send_empty()),
        }
    }

    pub(crate) fn put(
        &self,
        path: &str,
        bearer: &str,
        content_type: Option<&str>,
        len: u64,
        body: &mut dyn Read,
    ) -> Result<Reply, ShareError> {
        let mut req = self
            .headers(self.agent.put(self.url(path)), Some(bearer))
            .header("Content-Length", len.to_string());
        if let Some(content_type) = content_type {
            req = req.header("Content-Type", content_type);
        }
        reply(req.send(SendBody::from_reader(body)))
    }

    /// Sends only the given headers and a User-Agent; `url` is absolute.
    pub(crate) fn send(
        &self,
        method: &str,
        url: &str,
        headers: &[(&str, &str)],
        body: Option<(u64, &mut dyn Read)>,
    ) -> Result<Reply, ShareError> {
        let mut req = ureq::http::Request::builder()
            .method(method)
            .uri(url)
            .header("User-Agent", &self.user_agent);
        for (name, value) in headers {
            req = req.header(*name, *value);
        }
        let result = match body {
            Some((len, body)) => {
                let req = req
                    .header("Content-Length", len.to_string())
                    .body(SendBody::from_reader(body))
                    .map_err(|e| ShareError::Protocol(e.to_string()))?;
                let req = self
                    .agent
                    .configure_request(req)
                    .timeout_send_body(Some(send_time(len)))
                    .build();
                self.agent.run(req)
            }
            None => {
                let req = req
                    .body(())
                    .map_err(|e| ShareError::Protocol(e.to_string()))?;
                self.agent.run(req)
            }
        };
        reply(result)
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base)
    }

    fn headers<B>(
        &self,
        req: ureq::RequestBuilder<B>,
        bearer: Option<&str>,
    ) -> ureq::RequestBuilder<B> {
        let req = req
            .header("X-Easyshot-Version", &self.version)
            .header("User-Agent", &self.user_agent);
        match bearer {
            Some(token) => req.header("Authorization", format!("Bearer {token}")),
            None => req,
        }
    }
}

// One 25 MiB part gets 600 s; a larger single body gets as long at the same rate.
fn send_time(len: u64) -> Duration {
    Duration::from_secs((len / (25 * 1024 * 1024 / 600)).max(600))
}

fn reply(
    result: Result<ureq::http::Response<ureq::Body>, ureq::Error>,
) -> Result<Reply, ShareError> {
    let mut response = result.map_err(|e| ShareError::Network(e.to_string()))?;
    let status = response.status().as_u16();
    let body = response
        .body_mut()
        .with_config()
        .limit(REPLY_LIMIT)
        .read_to_vec()
        .map_err(|e| ShareError::Network(e.to_string()))?;
    Ok(Reply { status, body })
}

impl Reply {
    pub(crate) fn body(&self) -> &[u8] {
        &self.body
    }

    pub(crate) fn expect<T: DeserializeOwned>(&self, status: u16) -> Result<T, ShareError> {
        if self.status != status {
            return Err(self.error());
        }
        serde_json::from_slice(&self.body)
            .map_err(|e| ShareError::Protocol(format!("HTTP {status}: {e}")))
    }

    pub(crate) fn error(&self) -> ShareError {
        #[derive(Deserialize)]
        struct Envelope {
            error: Body,
        }
        #[derive(Deserialize)]
        struct Body {
            code: Code,
            retry_after_s: Option<u64>,
        }
        match serde_json::from_slice::<Envelope>(&self.body) {
            Ok(Envelope { error }) => ShareError::Service {
                code: error.code,
                retry_after_s: error.retry_after_s,
            },
            Err(_) => ShareError::Protocol(format!("HTTP {}", self.status)),
        }
    }
}
