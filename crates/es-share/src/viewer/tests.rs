use super::*;
use serde_json::Value;

fn contract() -> Vec<Value> {
    let text = include_str!("../../../../services/share/contract/viewer.json");
    let doc: Value = serde_json::from_str(text).expect("viewer.json parses");
    doc["cases"].as_array().expect("cases").clone()
}

fn error_code(err: &FillError) -> &'static str {
    match err {
        FillError::MissingValue(_) => "missing_value",
        FillError::NotHttps(_) => "not_https",
        FillError::Unclosed(_) => "unclosed",
    }
}

#[test]
fn every_contract_case_fills_as_the_service_does() {
    let cases = contract();
    assert!(cases.len() >= 7);
    for case in cases {
        let name = case["name"].as_str().expect("name");
        let values: HashMap<&str, &str> = case["values"]
            .as_object()
            .expect("values")
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str().expect("string value")))
            .collect();
        let blocks: Vec<&str> = case["blocks"]
            .as_array()
            .expect("blocks")
            .iter()
            .map(|b| b.as_str().expect("block name"))
            .collect();
        let got = fill(
            case["template"].as_str().expect("template"),
            &values,
            &blocks,
        );
        match (case.get("output"), case.get("error")) {
            (Some(output), None) => {
                assert_eq!(got.as_deref().ok(), output.as_str(), "case {name}");
            }
            (None, Some(error)) => {
                let err = got.expect_err(name);
                assert_eq!(
                    error_code(&err),
                    error.as_str().expect("error"),
                    "case {name}"
                );
            }
            _ => panic!("case {name} has neither output nor error"),
        }
    }
}

fn page_values() -> HashMap<&'static str, &'static str> {
    HashMap::from([
        ("title", "Login bug"),
        (
            "page_url",
            "https://share.easyshot.site/3Qw8eR2tY6uI0oP4aS1dF7",
        ),
        (
            "media_url",
            "https://m.easyshot.site/s/3d/3Qw8eR2tY6uI0oP4aS1dF7/media.mp4",
        ),
        (
            "poster_url",
            "https://m.easyshot.site/s/3d/3Qw8eR2tY6uI0oP4aS1dF7/poster.jpg",
        ),
        ("mime", "video/mp4"),
        ("width", "1920"),
        ("height", "1080"),
        ("expires_at", "2026-11-14T10:00:00Z"),
        ("download_name", "easyshot-3Qw8eR2tY6uI0oP4aS1dF7.mp4"),
        ("home_url", "https://easyshot.site"),
    ])
}

#[test]
fn the_shipped_page_fills_for_every_state_with_nothing_left_over() {
    let states: [&[&str]; 6] = [
        &["ready", "image", "expires"],
        &["ready", "video", "expires"],
        &["ready", "image"],
        &["ready", "video"],
        &["uploading"],
        &["missing"],
    ];
    for blocks in states {
        let page = fill(PAGE, &page_values(), blocks).expect("fills");
        for left in ["{{", "<!--if:", "<!--end:"] {
            assert!(!page.contains(left), "{blocks:?} leaves {left}");
        }
        let ready = blocks.contains(&"ready");
        let image = blocks.contains(&"image");
        let video = blocks.contains(&"video");
        assert_eq!(
            page.contains("<img class=\"media\""),
            ready && image,
            "{blocks:?}"
        );
        assert_eq!(
            page.contains("<video class=\"media\""),
            ready && video,
            "{blocks:?}"
        );
        assert_eq!(
            page.contains("og:video"),
            ready && video,
            "{blocks:?}: video inside ready"
        );
        assert_eq!(
            page.contains("data-expires="),
            blocks.contains(&"expires"),
            "{blocks:?}"
        );
        assert_eq!(
            page.contains(" data-reload>"),
            blocks.contains(&"uploading"),
            "{blocks:?}"
        );
    }
}

#[test]
fn placeholders_never_sit_in_script_or_style() {
    for tag in ["script", "style"] {
        let mut rest = PAGE;
        while let Some(at) = rest.find(&format!("<{tag}")) {
            let body = &rest[at..];
            let end = body.find(&format!("</{tag}>")).expect("closed");
            assert!(!body[..end].contains("{{"), "a placeholder inside <{tag}>");
            rest = &body[end..];
        }
    }
}

#[test]
fn a_value_carrying_markers_or_placeholders_stays_text() {
    let values = HashMap::from([("title", "<!--if:gone-->{{home_url}}<!--end:gone-->")]);
    let out = fill("<h1>{{title}}</h1>", &values, &["gone"]).expect("filled");
    assert_eq!(
        out,
        "<h1>&lt;!--if:gone--&gt;{{home_url}}&lt;!--end:gone--&gt;</h1>"
    );
}

#[test]
fn an_unclosed_block_is_an_error_not_a_silent_drop() {
    assert_eq!(
        fill("<!--if:ready--><p>x</p>", &HashMap::new(), &[]),
        Err(FillError::Unclosed("ready".into()))
    );
}

#[test]
fn text_that_only_looks_like_a_placeholder_is_kept() {
    let out = fill("{{ x }} {{Title}} {{", &HashMap::new(), &[]).expect("no placeholders");
    assert_eq!(out, "{{ x }} {{Title}} {{");
}
