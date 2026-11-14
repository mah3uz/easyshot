const MAX_NUMBERS: usize = 2;
const MAX_NUMBER_LEN: usize = 3;
const MAX_PART_LEN: usize = 15;
const MAX_CONSONANT_RUN: usize = 4;

/// A long name in code, such as `ext_image_copy_capture_session_v1` or `ZwlrScreencopyManagerV1`,
/// is made of pronounceable parts with few numbers, each ending a part; random keys are not.
pub(super) fn is_code_name(word: &str) -> bool {
    let bytes = word.as_bytes();
    if !bytes
        .iter()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
        || bytes.first().is_none_or(u8::is_ascii_digit)
    {
        return false;
    }
    let mixed =
        bytes.iter().any(u8::is_ascii_lowercase) && bytes.iter().any(u8::is_ascii_uppercase);
    let number_inside_part = bytes.windows(2).any(|w| {
        w[0].is_ascii_digit() && w[1].is_ascii_alphabetic() && !(mixed && w[1].is_ascii_uppercase())
    });
    if number_inside_part {
        return false;
    }

    let parts = parts(bytes);
    let numbers: Vec<&&[u8]> = parts.iter().filter(|p| p[0].is_ascii_digit()).collect();
    if numbers.len() > MAX_NUMBERS || numbers.iter().any(|p| p.len() > MAX_NUMBER_LEN) {
        return false;
    }
    let mut short = 0;
    let mut acronyms = 0;
    for (i, part) in parts.iter().enumerate() {
        if part[0].is_ascii_digit() {
            continue;
        }
        if !pronounceable(part) {
            return false;
        }
        let numbered = parts.get(i + 1).is_some_and(|n| n[0].is_ascii_digit());
        if part.len() <= 2 && !numbered {
            short += 1;
        }
        if part.len() >= 2 && part.iter().all(u8::is_ascii_uppercase) {
            acronyms += 1;
        }
    }
    if mixed {
        short <= 1 && acronyms <= 1
    } else {
        short <= 3
    }
}

/// Splits at `_` and `-`, between letters and digits, and at camel-case humps, keeping an
/// acronym whole: `HTTPServerV2` is `HTTP`, `Server`, `V`, `2`.
fn parts(word: &[u8]) -> Vec<&[u8]> {
    let mut out = Vec::new();
    for chunk in word
        .split(|b| matches!(b, b'_' | b'-'))
        .filter(|c| !c.is_empty())
    {
        let mut start = 0;
        for i in 1..chunk.len() {
            let (prev, cur) = (chunk[i - 1], chunk[i]);
            let next_lower = chunk.get(i + 1).is_some_and(u8::is_ascii_lowercase);
            let cut = prev.is_ascii_digit() != cur.is_ascii_digit()
                || (prev.is_ascii_lowercase() && cur.is_ascii_uppercase())
                || (prev.is_ascii_uppercase() && cur.is_ascii_uppercase() && next_lower);
            if cut {
                out.push(&chunk[start..i]);
                start = i;
            }
        }
        out.push(&chunk[start..]);
    }
    out
}

fn pronounceable(part: &[u8]) -> bool {
    if part.len() > MAX_PART_LEN {
        return false;
    }
    let mut run = 0;
    for b in part {
        run = if matches!(
            b.to_ascii_lowercase(),
            b'a' | b'e' | b'i' | b'o' | b'u' | b'y'
        ) {
            0
        } else {
            run + 1
        };
        if run > MAX_CONSONANT_RUN {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_snake_kebab_and_camel_case() {
        let split = |w: &'static str| -> Vec<&str> {
            parts(w.as_bytes())
                .into_iter()
                .map(|p| std::str::from_utf8(p).unwrap_or_default())
                .collect()
        };
        assert_eq!(split("HTTPServerV2"), ["HTTP", "Server", "V", "2"]);
        assert_eq!(split("__init_subclass__"), ["init", "subclass"]);
        assert_eq!(split("x86_64-unknown"), ["x", "86", "64", "unknown"]);
    }

    #[test]
    fn names_from_real_code_read_as_names() {
        for name in [
            "ext_foreign_toplevel_image_capture_source_manager_v1",
            "ExtForeignToplevelImageCaptureSourceManagerV1",
            "ext_image_copy_capture_session_v1",
            "ZwlrScreencopyManagerV1",
            "zwlr_screencopy_frame_v1",
            "x86_64_unknown_linux_gnu",
            "aarch64-unknown-linux-musl",
            "sha256_digest_for_payload",
            "XMLHttpRequestUploadV2Handler",
            "org_freedesktop_portal_ScreenCast_v5",
            "wp_cursor_shape_device_v1_shape",
            "AuthenticationProviderV2Impl",
        ] {
            assert!(is_code_name(name), "{name} should stay readable");
        }
    }

    #[test]
    fn known_random_shapes_are_not_names() {
        for token in [
            "a1b2c3d4e5f6a1b2c3d4e5f6a1",
            "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08",
            "3F2504E0-4F89-11D3-9A0C-0305E82C3301",
            concat!("AC", "0123456789abcdef0123456789abcdef"),
            "RkbtivPnenHADZHOBcxkRkW1x7Qp",
            "kP9xQ2mZ7vB4nL8wR3tY6uI1",
        ] {
            assert!(!is_code_name(token), "{token} must stay hidden");
        }
    }

    // A tiny xorshift so the sample is the same on every run.
    fn random_words(alphabet: &[u8], len: usize, count: usize) -> Vec<String> {
        let mut state: u64 = 0x9e37_79b9_7f4a_7c15;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        (0..count)
            .map(|_| {
                (0..len)
                    .map(|_| char::from(alphabet[(next() % alphabet.len() as u64) as usize]))
                    .collect()
            })
            .collect()
    }

    #[test]
    fn random_keys_of_32_or_more_characters_are_never_taken_for_names() {
        let alphabets: [&[u8]; 5] = [
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789",
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_",
            b"0123456789abcdef",
            b"abcdefghijklmnopqrstuvwxyz0123456789",
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789",
        ];
        for alphabet in alphabets {
            for len in [32, 40, 64] {
                let missed: Vec<String> = random_words(alphabet, len, 3000)
                    .into_iter()
                    .filter(|w| w.bytes().any(|b| b.is_ascii_digit()))
                    .filter(|w| is_code_name(w))
                    .collect();
                assert!(missed.is_empty(), "taken for names: {missed:?}");
            }
        }
    }
}
