use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::Ordering;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use image::{ImageFormat, RgbaImage};

use crate::tsv::parse_tsv;
use crate::{Error, OcrOptions, Word};

const POLL: Duration = Duration::from_millis(10);

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Probe {
    pub available: bool,
    pub path: Option<PathBuf>,
    pub version: Option<String>,
    pub languages: Vec<String>,
}

impl Probe {
    pub fn missing_languages<'a>(&self, wanted: &'a [String]) -> Vec<&'a str> {
        wanted
            .iter()
            .map(String::as_str)
            .filter(|l| !self.languages.iter().any(|have| have == l))
            .collect()
    }
}

fn command(path: Option<&Path>) -> Command {
    let mut cmd = Command::new(path.unwrap_or_else(|| Path::new("tesseract")));
    cmd.env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .env("LC_ALL", "C");
    cmd
}

pub fn probe(path: Option<&Path>) -> Probe {
    let Ok(out) = command(path).arg("--version").output() else {
        return Probe::default();
    };
    if !out.status.success() {
        return Probe::default();
    }
    let banner = String::from_utf8_lossy(if out.stdout.is_empty() {
        &out.stderr
    } else {
        &out.stdout
    })
    .into_owned();
    let version = banner
        .lines()
        .next()
        .and_then(|l| l.strip_prefix("tesseract "))
        .map(|v| v.trim().to_owned());
    let languages = command(path)
        .arg("--list-langs")
        .output()
        .map(|o| parse_languages(&String::from_utf8_lossy(&o.stdout)))
        .unwrap_or_default();
    Probe {
        available: true,
        path: path.map(Path::to_path_buf),
        version,
        languages,
    }
}

fn parse_languages(listing: &str) -> Vec<String> {
    listing
        .lines()
        .skip_while(|l| !l.starts_with("List of available languages"))
        .skip(1)
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect()
}

pub(crate) fn run(image: &RgbaImage, options: &OcrOptions) -> Result<Vec<Word>, Error> {
    let mut png = Cursor::new(Vec::new());
    image
        .write_to(&mut png, ImageFormat::Png)
        .map_err(Error::Encode)?;
    let png = png.into_inner();

    let mut cmd = command(options.tesseract_path.as_deref());
    cmd.args(["stdin", "stdout", "-l"]);
    cmd.arg(options.languages.join("+"));
    if let Some(psm) = options.psm {
        cmd.arg("--psm").arg(psm.to_string());
    }
    cmd.arg("tsv")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => Error::NotInstalled,
        _ => Error::Spawn(e),
    })?;

    let stdin = child.stdin.take();
    let writer = thread::spawn(move || {
        if let Some(mut stdin) = stdin {
            let _ = stdin.write_all(&png);
        }
    });
    let stdout = drain(child.stdout.take());
    let stderr = drain(child.stderr.take());

    let outcome = wait(&mut child, options);
    let _ = writer.join();
    let stdout = stdout.join().unwrap_or_default();
    let stderr = stderr.join().unwrap_or_default();

    let status = outcome?;
    if !status.success() {
        return Err(Error::Failed {
            status: status.to_string(),
            stderr: String::from_utf8_lossy(&stderr).trim().to_owned(),
        });
    }
    Ok(parse_tsv(&String::from_utf8_lossy(&stdout)))
}

fn drain<R: Read + Send + 'static>(pipe: Option<R>) -> JoinHandle<Vec<u8>> {
    thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut pipe) = pipe {
            let _ = pipe.read_to_end(&mut buf);
        }
        buf
    })
}

fn wait(child: &mut Child, options: &OcrOptions) -> Result<ExitStatus, Error> {
    let deadline = Instant::now() + options.timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) => {}
            Err(e) => return Err(Error::Spawn(e)),
        }
        let cancelled = options
            .cancel
            .as_ref()
            .is_some_and(|c| c.load(Ordering::Relaxed));
        if cancelled || Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(if cancelled {
                Error::Cancelled
            } else {
                Error::Timeout
            });
        }
        thread::sleep(POLL);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_listing_skips_the_banner() {
        let out = "List of available languages in \"/usr/share/tessdata/\" (3):\nben\neng\nosd\n";
        assert_eq!(parse_languages(out), ["ben", "eng", "osd"]);
    }

    #[test]
    fn missing_languages_are_reported() {
        let p = Probe {
            available: true,
            languages: vec!["eng".into()],
            ..Probe::default()
        };
        let want = ["eng".to_owned(), "deu".to_owned()];
        assert_eq!(p.missing_languages(&want), ["deu"]);
    }

    #[test]
    fn absent_binary_is_not_available() {
        assert!(!probe(Some(Path::new("/nonexistent/tesseract"))).available);
    }
}
