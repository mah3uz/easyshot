use std::ops::{Deref, DerefMut};
use std::sync::mpsc::Sender;

/// 32-bit pixel layouts named as wl_shm names them: little-endian words, so `Xrgb8888` is
/// B, G, R, X in memory and `Xbgr8888` is R, G, B, X.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    Argb8888,
    Xrgb8888,
    Abgr8888,
    Xbgr8888,
}

/// Pixel bytes that go back to the pool they came from when dropped.
pub struct PixelBuf {
    bytes: Vec<u8>,
    pool: Option<Sender<Vec<u8>>>,
}

impl PixelBuf {
    pub fn pooled(bytes: Vec<u8>, pool: Sender<Vec<u8>>) -> Self {
        PixelBuf {
            bytes,
            pool: Some(pool),
        }
    }
}

impl From<Vec<u8>> for PixelBuf {
    fn from(bytes: Vec<u8>) -> Self {
        PixelBuf { bytes, pool: None }
    }
}

impl Deref for PixelBuf {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        &self.bytes
    }
}

impl DerefMut for PixelBuf {
    fn deref_mut(&mut self) -> &mut [u8] {
        &mut self.bytes
    }
}

impl AsRef<[u8]> for PixelBuf {
    fn as_ref(&self) -> &[u8] {
        &self.bytes
    }
}

impl AsMut<[u8]> for PixelBuf {
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.bytes
    }
}

impl std::fmt::Debug for PixelBuf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PixelBuf")
            .field("len", &self.bytes.len())
            .field("pooled", &self.pool.is_some())
            .finish()
    }
}

impl Drop for PixelBuf {
    fn drop(&mut self) {
        if let Some(pool) = self.pool.take() {
            // A pool that has shut down just lets the bytes go.
            let _ = pool.send(std::mem::take(&mut self.bytes));
        }
    }
}

/// One captured video frame; rows are `stride` bytes apart.
#[derive(Debug)]
pub struct VideoFrame {
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub format: PixelFormat,
    pub data: PixelBuf,
    /// CLOCK_MONOTONIC nanoseconds at which the compositor presented this content, or at which
    /// the frame arrived when the compositor gives no presentation time.
    pub presented: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::channel;

    #[test]
    fn a_dropped_buffer_returns_its_allocation_to_the_pool() {
        let (tx, rx) = channel();
        let mut bytes = Vec::with_capacity(64);
        bytes.extend_from_slice(&[7; 16]);
        let ptr = bytes.as_ptr();
        let buf = PixelBuf::pooled(bytes, tx);
        assert_eq!(&buf[..], &[7; 16]);
        assert!(rx.try_recv().is_err());
        drop(buf);
        let back = rx.try_recv().expect("returned on drop");
        assert_eq!((back.as_ptr(), back.capacity()), (ptr, 64));
    }

    #[test]
    fn a_buffer_outliving_its_pool_or_without_one_drops_quietly() {
        let (tx, rx) = channel();
        let buf = PixelBuf::pooled(vec![1; 4], tx);
        drop(rx);
        drop(buf);
        drop(PixelBuf::from(vec![2; 4]));
    }
}
