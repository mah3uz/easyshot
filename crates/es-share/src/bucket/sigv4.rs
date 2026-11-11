use crate::error::ShareError;
use hmac::{Hmac, KeyInit, Mac};
use sha2::{Digest, Sha256};
use std::fmt::Write;
use time::OffsetDateTime;
use time::macros::format_description;

pub(crate) const UNSIGNED_PAYLOAD: &str = "UNSIGNED-PAYLOAD";

pub(crate) struct Credentials<'a> {
    pub(crate) access_key_id: &'a str,
    pub(crate) secret_access_key: &'a str,
    pub(crate) region: &'a str,
    pub(crate) service: &'a str,
}

pub(crate) struct Request<'a> {
    pub(crate) method: &'a str,
    /// Already encoded with `encode_path`; the same string goes on the wire.
    pub(crate) path: &'a str,
    pub(crate) query: &'a [(&'a str, &'a str)],
    /// Every header here is signed; `host` must be among them.
    pub(crate) headers: &'a [(&'a str, &'a str)],
    pub(crate) payload_hash: &'a str,
}

pub(crate) struct Signed {
    pub(crate) amz_date: String,
    pub(crate) authorization: String,
}

pub(crate) fn sign(
    creds: &Credentials<'_>,
    req: &Request<'_>,
    now: OffsetDateTime,
) -> Result<Signed, ShareError> {
    let amz_date = format_time(now, true)?;
    let day = format_time(now, false)?;
    let (canonical, signed_headers) = canonical_request(req, &amz_date);
    let scope = format!("{day}/{}/{}/aws4_request", creds.region, creds.service);
    let to_sign = format!(
        "AWS4-HMAC-SHA256\n{amz_date}\n{scope}\n{}",
        hex(&Sha256::digest(canonical.as_bytes()))
    );
    let mut key = hmac(
        format!("AWS4{}", creds.secret_access_key).as_bytes(),
        day.as_bytes(),
    )?;
    for part in [creds.region, creds.service, "aws4_request"] {
        key = hmac(&key, part.as_bytes())?;
    }
    let signature = hex(&hmac(&key, to_sign.as_bytes())?);
    Ok(Signed {
        authorization: format!(
            "AWS4-HMAC-SHA256 Credential={}/{scope}, SignedHeaders={signed_headers}, Signature={signature}",
            creds.access_key_id
        ),
        amz_date,
    })
}

fn canonical_request(req: &Request<'_>, amz_date: &str) -> (String, String) {
    let mut headers: Vec<(String, String)> = req
        .headers
        .iter()
        .map(|(name, value)| {
            let value = value.split_whitespace().collect::<Vec<_>>().join(" ");
            (name.to_ascii_lowercase(), value)
        })
        .chain([("x-amz-date".to_owned(), amz_date.to_owned())])
        .collect();
    headers.sort();
    let mut query: Vec<(String, String)> = req
        .query
        .iter()
        .map(|(k, v)| (encode(k, false), encode(v, false)))
        .collect();
    query.sort();
    let query = query
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("&");
    let mut canonical = format!("{}\n{}\n{query}\n", req.method, req.path);
    for (name, value) in &headers {
        let _ = writeln!(canonical, "{name}:{value}");
    }
    let signed = headers
        .iter()
        .map(|(name, _)| name.as_str())
        .collect::<Vec<_>>()
        .join(";");
    let _ = write!(canonical, "\n{signed}\n{}", req.payload_hash);
    (canonical, signed)
}

/// S3 signs the path as sent, encoded once and never normalised.
pub(crate) fn encode_path(path: &str) -> String {
    encode(path, true)
}

fn encode(text: &str, keep_slash: bool) -> String {
    let mut out = String::with_capacity(text.len());
    for b in text.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(char::from(b))
            }
            b'/' if keep_slash => out.push('/'),
            _ => {
                let _ = write!(out, "%{b:02X}");
            }
        }
    }
    out
}

pub(crate) fn sha256_hex(data: &[u8]) -> String {
    hex(&Sha256::digest(data))
}

fn hmac(key: &[u8], data: &[u8]) -> Result<[u8; 32], ShareError> {
    let mut mac = Hmac::<Sha256>::new_from_slice(key)
        .map_err(|_| ShareError::Protocol("signing key".into()))?;
    mac.update(data);
    Ok(mac.finalize().into_bytes().into())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::with_capacity(64), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

fn format_time(now: OffsetDateTime, with_time: bool) -> Result<String, ShareError> {
    let now = now.to_offset(time::UtcOffset::UTC);
    let formatted = match with_time {
        true => now.format(format_description!(
            "[year][month][day]T[hour][minute][second]Z"
        )),
        false => now.format(format_description!("[year][month][day]")),
    };
    formatted.map_err(|e| ShareError::Protocol(e.to_string()))
}

#[cfg(test)]
mod tests;
