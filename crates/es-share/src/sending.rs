use std::io::{self, Read};
use std::sync::atomic::{AtomicBool, Ordering};

/// A short source is an error, so a short body is never sent.
pub(crate) struct Sending<'a, R> {
    inner: R,
    left: u64,
    sent: u64,
    on_sent: &'a mut dyn FnMut(u64),
    cancel: &'a AtomicBool,
    /// The source's own failure, kept so it is not mistaken for a network one.
    pub(crate) failed: Option<io::Error>,
}

impl<'a, R: Read> Sending<'a, R> {
    pub(crate) fn new(
        inner: R,
        len: u64,
        sent_before: u64,
        on_sent: &'a mut dyn FnMut(u64),
        cancel: &'a AtomicBool,
    ) -> Self {
        Self {
            inner,
            left: len,
            sent: sent_before,
            on_sent,
            cancel,
            failed: None,
        }
    }

    fn fail(&mut self, err: io::Error) -> io::Error {
        let kind = err.kind();
        self.failed = Some(err);
        io::Error::from(kind)
    }
}

impl<R: Read> Read for Sending<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.cancel.load(Ordering::Relaxed) {
            return Err(io::Error::other("cancelled"));
        }
        if self.left == 0 || buf.is_empty() {
            return Ok(0);
        }
        let want = buf
            .len()
            .min(usize::try_from(self.left).unwrap_or(usize::MAX));
        let n = match self.inner.read(&mut buf[..want]) {
            Ok(0) => return Err(self.fail(io::ErrorKind::UnexpectedEof.into())),
            Ok(n) => n,
            Err(err) if err.kind() == io::ErrorKind::Interrupted => return Err(err),
            Err(err) => return Err(self.fail(err)),
        };
        self.left -= n as u64;
        self.sent += n as u64;
        (self.on_sent)(self.sent);
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_exactly_len_and_reports_the_running_total() {
        let mut seen = Vec::new();
        let mut on_sent = |n| seen.push(n);
        let cancel = AtomicBool::new(false);
        let mut out = Vec::new();
        let mut body = Sending::new(&b"abcdefgh"[..], 5, 100, &mut on_sent, &cancel);
        body.read_to_end(&mut out).expect("read");
        assert_eq!(out, b"abcde");
        assert_eq!(seen.last(), Some(&105));
    }

    #[test]
    fn a_short_file_fails_instead_of_sending_a_short_part() {
        let mut on_sent = |_| {};
        let cancel = AtomicBool::new(false);
        let mut body = Sending::new(&b"abc"[..], 5, 0, &mut on_sent, &cancel);
        let err = body.read_to_end(&mut Vec::new()).expect_err("short");
        assert_eq!(err.kind(), io::ErrorKind::UnexpectedEof);
        assert!(body.failed.is_some());
    }

    #[test]
    fn cancel_stops_the_body_mid_way() {
        let cancel = AtomicBool::new(false);
        let mut on_sent = |_| cancel.store(true, Ordering::Relaxed);
        let mut body = Sending::new(&b"abcdef"[..], 6, 0, &mut on_sent, &cancel);
        let mut buf = [0u8; 2];
        assert_eq!(body.read(&mut buf).expect("first"), 2);
        assert!(body.read(&mut buf).is_err());
        assert!(body.failed.is_none(), "a cancel is not a file error");
    }
}
