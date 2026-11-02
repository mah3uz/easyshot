#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub width: usize,
    pub height: usize,
    pub data: Vec<u8>,
}

impl Frame {
    pub fn new(width: usize, height: usize, data: Vec<u8>) -> Option<Self> {
        (width > 0 && height > 0 && data.len() == width.checked_mul(height)?.checked_mul(4)?)
            .then_some(Self {
                width,
                height,
                data,
            })
    }

    pub(crate) fn stride(&self) -> usize {
        self.width * 4
    }

    pub(crate) fn row(&self, y: usize) -> &[u8] {
        &self.data[y * self.stride()..(y + 1) * self.stride()]
    }
}

pub fn frame_hash(frame: &Frame) -> u64 {
    const K: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut h = ((frame.width as u64) << 32) ^ frame.height as u64;
    let (chunks, rest) = frame.data.as_chunks::<8>();
    for c in chunks {
        h = (h.rotate_left(5) ^ u64::from_le_bytes(*c)).wrapping_mul(K);
    }
    for &b in rest {
        h = (h.rotate_left(5) ^ u64::from(b)).wrapping_mul(K);
    }
    h
}
