use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicBool, Ordering};

/// Above this a challenge would take hours; the service asks for about 20.
pub const MAX_DIFFICULTY: u32 = 32;

/// None when cancelled or the difficulty is over [`MAX_DIFFICULTY`].
pub fn solve(challenge: &str, difficulty: u32, cancel: &AtomicBool) -> Option<u64> {
    if difficulty > MAX_DIFFICULTY {
        return None;
    }
    let mut prefix = Sha256::new();
    prefix.update(challenge.as_bytes());
    prefix.update(b":");
    let mut nonce: u64 = 0;
    loop {
        if nonce.is_multiple_of(4096) && cancel.load(Ordering::Relaxed) {
            return None;
        }
        let mut hash = prefix.clone();
        hash.update(nonce.to_string().as_bytes());
        if leading_zero_bits(&hash.finalize()) >= difficulty {
            return Some(nonce);
        }
        nonce += 1;
    }
}

pub fn leading_zero_bits(digest: &[u8]) -> u32 {
    let mut bits = 0;
    for byte in digest {
        bits += byte.leading_zeros();
        if *byte != 0 {
            break;
        }
    }
    bits
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn cases() -> Vec<Value> {
        let text = include_str!("../../../services/share/contract/pow.json");
        let doc: Value = serde_json::from_str(text).expect("pow.json parses");
        doc["cases"].as_array().expect("cases").clone()
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    #[test]
    fn finds_the_contract_nonce_the_service_will_accept() {
        let cases = cases();
        assert!(!cases.is_empty());
        for case in cases {
            let challenge = case["challenge"].as_str().expect("challenge");
            let difficulty = case["difficulty"].as_u64().expect("difficulty") as u32;
            let nonce = solve(challenge, difficulty, &AtomicBool::new(false)).expect("solved");
            assert_eq!(nonce.to_string(), case["nonce"].as_str().expect("nonce"));
            let digest = Sha256::digest(format!("{challenge}:{nonce}").as_bytes());
            assert_eq!(hex(&digest), case["sha256"].as_str().expect("sha256"));
            assert!(leading_zero_bits(&digest) >= difficulty);
        }
    }

    #[test]
    fn counts_bits_across_byte_boundaries() {
        assert_eq!(leading_zero_bits(&[0x00, 0x5f]), 9);
        assert_eq!(leading_zero_bits(&[0x00, 0x00, 0x00]), 24);
        assert_eq!(leading_zero_bits(&[0x80]), 0);
    }

    #[test]
    fn a_hostile_difficulty_or_a_cancel_stops_the_search() {
        let challenge = "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8";
        assert_eq!(solve(challenge, 255, &AtomicBool::new(false)), None);
        assert_eq!(solve(challenge, 30, &AtomicBool::new(true)), None);
    }
}
