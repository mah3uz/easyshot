use super::*;
use serde_json::Value;
use time::format_description::well_known::Rfc3339;

fn pairs(value: &Value) -> Vec<(&str, &str)> {
    value
        .as_array()
        .expect("pairs")
        .iter()
        .map(|p| (p[0].as_str().expect("name"), p[1].as_str().expect("value")))
        .collect()
}

fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key]
        .as_str()
        .unwrap_or_else(|| panic!("{key} is a string"))
}

#[test]
fn signs_every_published_aws_vector() {
    let doc: Value =
        serde_json::from_str(include_str!("../sigv4-vectors.json")).expect("vectors parse");
    let mut checked = 0;
    for suite in doc["suites"].as_array().expect("suites") {
        let creds = Credentials {
            access_key_id: text(suite, "access_key_id"),
            secret_access_key: text(suite, "secret_access_key"),
            region: text(suite, "region"),
            service: text(suite, "service"),
        };
        let now = OffsetDateTime::parse(text(suite, "time"), &Rfc3339).expect("time");
        for case in suite["cases"].as_array().expect("cases") {
            let name = text(case, "name");
            let path = encode_path(text(case, "path"));
            let (query, headers) = (pairs(&case["query"]), pairs(&case["headers"]));
            let req = Request {
                method: text(case, "method"),
                path: &path,
                query: &query,
                headers: &headers,
                payload_hash: text(case, "payload_hash"),
            };
            let (canonical, signed_headers) = canonical_request(&req, "20150830T123600Z");
            if let Some(want) = case["canonical_request"].as_str() {
                assert_eq!(canonical, want, "{name}: canonical request");
            }
            let signed = sign(&creds, &req, now).expect("sign");
            let (canonical, _) = canonical_request(&req, &signed.amz_date);
            let to_sign = text(case, "string_to_sign");
            assert_eq!(
                to_sign.lines().last(),
                Some(sha256_hex(canonical.as_bytes()).as_str()),
                "{name}: canonical request hash"
            );
            let want = text(case, "signature");
            assert_eq!(
                signed.authorization,
                format!(
                    "AWS4-HMAC-SHA256 Credential={}/{}, SignedHeaders={signed_headers}, Signature={want}",
                    creds.access_key_id,
                    to_sign.lines().nth(2).expect("scope"),
                ),
                "{name}"
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 16);
}

#[test]
fn a_key_is_encoded_once_keeping_its_slashes() {
    assert_eq!(
        encode_path("/b/my shots/a+b=c&d?#%/ü.png"),
        "/b/my%20shots/a%2Bb%3Dc%26d%3F%23%25/%C3%BC.png"
    );
    assert_eq!(encode("a/b", false), "a%2Fb");
}

#[test]
fn x_amz_date_is_utc_whatever_the_clock_offset() {
    let local = OffsetDateTime::parse("2015-08-30T14:36:00+02:00", &Rfc3339).expect("time");
    assert_eq!(format_time(local, true).expect("date"), "20150830T123600Z");
}
