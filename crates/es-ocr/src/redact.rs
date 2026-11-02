use std::ops::Range;
use std::sync::LazyLock;

use es_geom::{Rect, Size};
use regex::Regex;

use crate::Word;

struct Rule {
    re: Regex,
    accepts: fn(&str) -> bool,
}

const SECRET_LABEL: &str = r"(?i)\b(?:password|passcode|secret|api\s*key|access\s*key|client\s*secret|private\s*key|authorization|bearer|token)\b\s*[:=]\s*\S+";

type Accept = fn(&str) -> bool;

const RULES: [(&str, Accept); 9] = [
    (r"(?i)\b[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}\b", any),
    (r#"(?i)\bhttps?://[^\s<>"']+"#, any),
    (r#"(?i)\bwww\.[^\s<>"']+"#, any),
    (r"(?i)\b\d{1,3}(?:\.\d{1,3}){3}\b", is_ipv4),
    (r"(?i)\b(?:\+?\d[\d\s().-]{7,}\d)\b", is_phone),
    (r"(?i)\b(?:\d[ -]*?){13,19}\b", is_card),
    (
        r"(?i)\beyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\b",
        any,
    ),
    (
        r"(?i)\b(?:sk|pk|rk|ghp|gho|ghu|github_pat|xox[baprs]?|AKIA|ASIA|AIza|ya29)[A-Za-z0-9_-]{8,}\b",
        any,
    ),
    (r"(?i)\b[A-Za-z0-9_-]{24,}\b", is_opaque_token),
];

static COMPILED: LazyLock<Vec<Rule>> = LazyLock::new(|| {
    RULES
        .iter()
        .filter_map(|(pattern, accepts)| {
            Some(Rule {
                re: Regex::new(pattern).ok()?,
                accepts: *accepts,
            })
        })
        .collect()
});

static SECRET: LazyLock<Option<Regex>> = LazyLock::new(|| Regex::new(SECRET_LABEL).ok());

const PAD_X_PX: f64 = 6.0;
const PAD_Y_PX: f64 = 4.0;
const PAD_MIN: f64 = 0.002;
const PAD_MAX: f64 = 0.012;
const MIN_FRACTION: f64 = 0.001;
const DUPLICATE_OVERLAP: f64 = 0.85;

fn any(_: &str) -> bool {
    true
}

fn is_ipv4(value: &str) -> bool {
    let parts: Vec<&str> = value.split('.').collect();
    parts.len() == 4
        && parts
            .iter()
            .all(|p| p.parse::<u16>().is_ok_and(|n| n <= 255))
}

fn is_phone(value: &str) -> bool {
    (8..=15).contains(&value.chars().filter(|c| c.is_numeric()).count())
}

fn is_card(value: &str) -> bool {
    let digits: Vec<u32> = value.chars().filter_map(|c| c.to_digit(10)).collect();
    (13..=19).contains(&digits.len()) && luhn(&digits)
}

fn is_opaque_token(value: &str) -> bool {
    let has_letter = value.chars().any(char::is_alphabetic);
    let has_number = value.chars().any(char::is_numeric);
    if !(has_letter && has_number) {
        return false;
    }
    let len = value.chars().count();
    let separators = value.chars().filter(|c| matches!(c, '-' | '_')).count();
    len >= 24 && separators <= 6.max(len / 8)
}

fn luhn(digits: &[u32]) -> bool {
    let sum: u32 = digits
        .iter()
        .rev()
        .enumerate()
        .map(|(i, &d)| {
            if i % 2 == 1 {
                let doubled = d * 2;
                if doubled > 9 { doubled - 9 } else { doubled }
            } else {
                d
            }
        })
        .sum();
    sum.is_multiple_of(10)
}

fn secret_value_ranges(text: &str) -> Vec<Range<usize>> {
    let Some(re) = SECRET.as_ref() else {
        return Vec::new();
    };
    re.find_iter(text)
        .map(|m| {
            let hit = &text[m.range()];
            let Some(sep) = hit.find([':', '=']) else {
                return m.range();
            };
            let after = &hit[sep + 1..];
            let skipped = after.len() - after.trim_start().len();
            let start = m.start() + sep + 1 + skipped;
            if start < m.end() {
                start..m.end()
            } else {
                m.range()
            }
        })
        .collect()
}

pub fn sensitive_ranges(text: &str) -> Vec<Range<usize>> {
    let mut ranges: Vec<Range<usize>> = COMPILED
        .iter()
        .flat_map(|rule| {
            rule.re
                .find_iter(text)
                .filter(|m| (rule.accepts)(m.as_str()))
                .map(|m| m.range())
        })
        .collect();
    ranges.extend(secret_value_ranges(text));
    merge_ranges(ranges)
}

fn merge_ranges(mut ranges: Vec<Range<usize>>) -> Vec<Range<usize>> {
    ranges.sort_by_key(|r| r.start);
    let mut merged: Vec<Range<usize>> = Vec::new();
    for r in ranges {
        match merged.last_mut() {
            Some(last) if r.start <= last.end => last.end = last.end.max(r.end),
            _ => merged.push(r),
        }
    }
    merged
}

pub fn find(words: &[Word], image: Size) -> Vec<Rect> {
    let bounds = Rect::new(0.0, 0.0, image.width, image.height);
    let pad_x = (PAD_X_PX / image.width.max(1.0)).clamp(PAD_MIN, PAD_MAX) * image.width;
    let pad_y = (PAD_Y_PX / image.height.max(1.0)).clamp(PAD_MIN, PAD_MAX) * image.height;

    let mut found: Vec<Rect> = Vec::new();
    for line in words.chunk_by(|a, b| a.line == b.line) {
        let (text, spans) = line_text(line);
        for range in sensitive_ranges(&text) {
            let Some(rect) = range_rect(line, &spans, &text, &range) else {
                continue;
            };
            let padded = rect.inflate(pad_x, pad_y).intersect(bounds);
            if padded.width() >= MIN_FRACTION * image.width
                && padded.height() >= MIN_FRACTION * image.height
            {
                found.push(padded);
            }
        }
    }
    dedupe(found)
}

fn line_text(line: &[Word]) -> (String, Vec<Range<usize>>) {
    let mut text = String::new();
    let mut spans = Vec::with_capacity(line.len());
    for w in line {
        if !text.is_empty() {
            text.push(' ');
        }
        let start = text.len();
        text.push_str(&w.text);
        spans.push(start..text.len());
    }
    (text, spans)
}

fn range_rect(
    line: &[Word],
    spans: &[Range<usize>],
    text: &str,
    hit: &Range<usize>,
) -> Option<Rect> {
    line.iter()
        .zip(spans)
        .filter(|(_, span)| span.start < hit.end && hit.start < span.end)
        .map(|(word, span)| {
            let total = text[span.clone()].chars().count().max(1) as f64;
            let from = text[span.start..hit.start.max(span.start)].chars().count() as f64;
            let to = total - text[hit.end.min(span.end)..span.end].chars().count() as f64;
            let w = word.rect.width();
            Rect::new(
                word.rect.x0 + w * from / total,
                word.rect.y0,
                word.rect.x0 + w * to / total,
                word.rect.y1,
            )
        })
        .reduce(|a, b| a.union(b))
}

fn dedupe(rects: Vec<Rect>) -> Vec<Rect> {
    let mut out: Vec<Rect> = Vec::new();
    for r in rects {
        let duplicate = out
            .iter()
            .any(|e| e.intersect(r).area().max(0.0) >= e.area().min(r.area()) * DUPLICATE_OVERLAP);
        if !duplicate {
            out.push(r);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matches(text: &str) -> Vec<&str> {
        sensitive_ranges(text)
            .into_iter()
            .map(|r| &text[r])
            .collect()
    }

    fn words(line: u32, parts: &[(&str, f64, f64)]) -> Vec<Word> {
        parts
            .iter()
            .map(|(t, x0, x1)| Word {
                text: (*t).to_owned(),
                rect: Rect::new(
                    *x0,
                    100.0 * f64::from(line),
                    *x1,
                    100.0 * f64::from(line) + 30.0,
                ),
                confidence: 90.0,
                line,
            })
            .collect()
    }

    #[test]
    fn every_rule_compiles() {
        assert_eq!(COMPILED.len(), RULES.len());
        assert!(SECRET.is_some());
    }

    #[test]
    fn email() {
        assert_eq!(
            matches("reach me at jane.doe@example.com today"),
            ["jane.doe@example.com"]
        );
    }

    #[test]
    fn urls() {
        let u = "see https://internal.corp/dashboard?token=1 for details";
        assert_eq!(matches(u), ["https://internal.corp/dashboard?token=1"]);
        assert_eq!(matches("open www.example.org/a now"), ["www.example.org/a"]);
    }

    #[test]
    fn ipv4_needs_octets_up_to_255() {
        assert_eq!(
            matches("server lives at 10.0.12.7 in the rack"),
            ["10.0.12.7"]
        );
        assert!(matches("version 999.1.2.3 build notes").is_empty());
    }

    #[test]
    fn phone_numbers() {
        assert_eq!(matches("call 415-555-0134 anytime"), ["415-555-0134"]);
        assert_eq!(matches("dial 415 555 0134 now"), ["415 555 0134"]);
        assert!(matches("ref 1234567 only").is_empty());
    }

    #[test]
    fn card_numbers_need_a_valid_checksum() {
        assert_eq!(
            matches("card 4539 1488 0343 6467 on file"),
            ["4539 1488 0343 6467"]
        );
        assert!(matches("order 1234 5678 9012 3456 shipped").is_empty());
    }

    #[test]
    fn luhn_known_values() {
        assert!(luhn(&[4, 5, 3, 9, 1, 4, 8, 8, 0, 3, 4, 3, 6, 4, 6, 7]));
        assert!(!luhn(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 1, 2, 3, 4, 5, 6]));
    }

    #[test]
    fn api_keys_and_tokens() {
        assert_eq!(
            matches("key sk-proj-AbCdEf123456 in env"),
            ["sk-proj-AbCdEf123456"]
        );
        let gh = "token ghp_16C7e42F292c6912E7710c838347Ae178B4a rotated";
        assert_eq!(matches(gh), ["ghp_16C7e42F292c6912E7710c838347Ae178B4a"]);
        assert_eq!(
            matches("id AKIAIOSFODNN7EXAMPLE end"),
            ["AKIAIOSFODNN7EXAMPLE"]
        );
    }

    #[test]
    fn jwt() {
        let t = "session eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjMifQ.sflKxwRJSMeKKF2QT4 expires";
        assert_eq!(
            matches(t),
            ["eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjMifQ.sflKxwRJSMeKKF2QT4"]
        );
    }

    #[test]
    fn opaque_token_needs_letters_and_digits() {
        assert_eq!(
            matches("x a1b2c3d4e5f6a1b2c3d4e5f6a1 y"),
            ["a1b2c3d4e5f6a1b2c3d4e5f6a1"]
        );
        assert!(matches("x abcdefghijklmnopqrstuvwxyz y").is_empty());
        assert!(matches("update index.min.js and archive.tar.gz today").is_empty());
    }

    #[test]
    fn labelled_secret_boxes_the_value_not_the_label() {
        assert_eq!(matches("password: hunter2rocks"), ["hunter2rocks"]);
        assert_eq!(matches("API Key = abc"), ["abc"]);
        assert!(matches("the password is in the vault").is_empty());
    }

    #[test]
    fn prose_is_untouched() {
        assert!(matches("The quick brown fox jumps over the lazy dog").is_empty());
        assert!(matches("meeting moved to Thursday at 3pm, room 204").is_empty());
    }

    #[test]
    fn overlapping_hits_merge_into_one_range() {
        let t = "https://api.example.com/v1?user=jane@example.com";
        assert_eq!(matches(t), [t]);
    }

    #[test]
    fn rects_cover_only_the_hit_words_with_padding() {
        let w = words(
            0,
            &[
                ("mail", 20.0, 80.0),
                ("jane@example.com", 100.0, 420.0),
                ("now", 440.0, 500.0),
            ],
        );
        let r = find(&w, Size::new(1000.0, 1000.0));
        assert_eq!(r.len(), 1);
        let pad_x = 6.0;
        let pad_y = 4.0;
        assert_eq!(
            r[0],
            Rect::new(100.0 - pad_x, 0.0, 420.0 + pad_x, 30.0 + pad_y)
        );
    }

    #[test]
    fn partial_word_hits_narrow_the_box() {
        let w = words(0, &[("password:hunter2rocks", 0.0, 210.0)]);
        let r = find(&w, Size::new(1000.0, 1000.0));
        assert_eq!(r.len(), 1);
        assert!((r[0].x0 - (90.0 - 6.0)).abs() < 1e-9, "{:?}", r[0]);
    }

    #[test]
    fn padding_is_clamped_to_the_image() {
        let w = words(0, &[("jane@example.com", 0.0, 200.0)]);
        let r = find(&w, Size::new(400.0, 40.0));
        assert_eq!(r[0].x0, 0.0);
        assert_eq!(r[0].y0, 0.0);
        assert!(r[0].y1 <= 40.0);
    }

    #[test]
    fn hits_on_separate_lines_stay_separate() {
        let mut w = words(0, &[("jane@example.com", 20.0, 300.0)]);
        w.extend(words(1, &[("10.0.12.7", 20.0, 200.0)]));
        assert_eq!(find(&w, Size::new(1000.0, 1000.0)).len(), 2);
    }

    #[test]
    fn near_identical_rects_are_deduplicated() {
        let a = Rect::new(0.0, 0.0, 100.0, 30.0);
        let b = Rect::new(2.0, 0.0, 100.0, 30.0);
        let c = Rect::new(300.0, 0.0, 400.0, 30.0);
        assert_eq!(dedupe(vec![a, b, c]), vec![a, c]);
    }

    #[test]
    fn no_sensitive_text_gives_no_rects() {
        let w = words(0, &[("Hello", 0.0, 50.0), ("world", 60.0, 120.0)]);
        assert!(find(&w, Size::new(500.0, 500.0)).is_empty());
    }
}
