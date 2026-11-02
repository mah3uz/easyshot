use crate::atomic::write_atomic;
use crate::error::{Error, Result};
use std::path::Path;

const BLOCK: usize = 512;

pub struct BundleEntry {
    pub name: String,
    pub bytes: Vec<u8>,
}

/// Writes an uncompressed ustar archive of diagnostics (JSON and logs, never images).
pub fn write_bundle(path: &Path, mtime: u64, entries: &[BundleEntry]) -> Result<()> {
    let mut out = Vec::new();
    for entry in entries {
        out.extend_from_slice(&header(entry, mtime, path)?);
        out.extend_from_slice(&entry.bytes);
        out.resize(out.len().next_multiple_of(BLOCK), 0);
    }
    out.resize(out.len() + 2 * BLOCK, 0);
    write_atomic(path, &out)
}

fn header(entry: &BundleEntry, mtime: u64, archive: &Path) -> Result<[u8; BLOCK]> {
    let name = entry.name.as_bytes();
    if name.is_empty() || name.len() > 100 {
        return Err(Error::Io {
            path: archive.to_path_buf(),
            source: std::io::Error::other(format!("bundle entry name unusable: {}", entry.name)),
        });
    }
    let mut h = [0u8; BLOCK];
    h[..name.len()].copy_from_slice(name);
    h[100..108].copy_from_slice(b"0000600\0");
    h[108..116].copy_from_slice(b"0000000\0");
    h[116..124].copy_from_slice(b"0000000\0");
    h[124..136].copy_from_slice(format!("{:011o}\0", entry.bytes.len()).as_bytes());
    h[136..148].copy_from_slice(format!("{mtime:011o}\0").as_bytes());
    h[148..156].fill(b' ');
    h[156] = b'0';
    h[257..263].copy_from_slice(b"ustar\0");
    h[263..265].copy_from_slice(b"00");
    let sum: u32 = h.iter().map(|b| u32::from(*b)).sum();
    h[148..156].copy_from_slice(format!("{sum:06o}\0 ").as_bytes());
    Ok(h)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archive_is_readable_by_tar() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("b.tar");
        let entries = [
            BundleEntry {
                name: "doctor.json".into(),
                bytes: b"{\"a\":1}".to_vec(),
            },
            BundleEntry {
                name: "log/x.log".into(),
                bytes: vec![b'x'; 700],
            },
        ];
        write_bundle(&path, 1_700_000_000, &entries).unwrap();
        let listing = std::process::Command::new("tar")
            .arg("-tvf")
            .arg(&path)
            .output()
            .unwrap();
        assert!(listing.status.success(), "{listing:?}");
        let text = String::from_utf8(listing.stdout).unwrap();
        assert!(
            text.contains("doctor.json") && text.contains("log/x.log"),
            "{text}"
        );
        let one = std::process::Command::new("tar")
            .arg("-xOf")
            .arg(&path)
            .arg("doctor.json")
            .output()
            .unwrap();
        assert_eq!(one.stdout, b"{\"a\":1}");
    }

    #[test]
    fn long_names_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let entry = BundleEntry {
            name: "n".repeat(101),
            bytes: vec![],
        };
        assert!(write_bundle(&dir.path().join("b.tar"), 0, &[entry]).is_err());
    }
}
