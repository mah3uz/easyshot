use crate::Error;
use crate::worker::State;
use rustix::fs::{MemfdFlags, ftruncate, memfd_create};
use rustix::mm::{MapFlags, ProtFlags, mmap, munmap};
use std::ffi::c_void;
use std::os::fd::AsFd;
use std::ptr::NonNull;
use std::sync::Arc;
use wayland_client::protocol::wl_shm::{self, Format, WlShm};
use wayland_client::protocol::{wl_buffer::WlBuffer, wl_shm_pool::WlShmPool};
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle};

pub(crate) struct ShmBuffer {
    ptr: NonNull<c_void>,
    len: usize,
    pool: WlShmPool,
    pub buffer: WlBuffer,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub format: Format,
}

impl ShmBuffer {
    pub fn new(
        shm: &WlShm,
        qh: &QueueHandle<State>,
        (width, height): (u32, u32),
        stride: u32,
        format: Format,
    ) -> Result<Self, Error> {
        let len = (stride as usize)
            .checked_mul(height as usize)
            .filter(|&l| l > 0 && l <= i32::MAX as usize && stride >= width.saturating_mul(4))
            .ok_or_else(|| {
                Error::Protocol(format!("bad buffer {width}x{height} stride {stride}"))
            })?;
        let fd = memfd_create("easyshot-capture", MemfdFlags::CLOEXEC)?;
        ftruncate(&fd, len as u64)?;
        #[allow(unsafe_code)]
        // SAFETY: a fresh shared mapping of a memfd we own, sized exactly by ftruncate above; nothing else aliases it in this process.
        let ptr = unsafe {
            mmap(
                std::ptr::null_mut(),
                len,
                ProtFlags::READ | ProtFlags::WRITE,
                MapFlags::SHARED,
                fd.as_fd(),
                0,
            )
        }?;
        let ptr = NonNull::new(ptr).ok_or(Error::Protocol("mmap returned null".into()))?;
        let pool = shm.create_pool(fd.as_fd(), len as i32, qh, ());
        let buffer = pool.create_buffer(
            0,
            width as i32,
            height as i32,
            stride as i32,
            format,
            qh,
            (),
        );
        Ok(ShmBuffer {
            ptr,
            len,
            pool,
            buffer,
            width,
            height,
            stride,
            format,
        })
    }

    pub fn bytes(&self) -> &[u8] {
        #[allow(unsafe_code)]
        // SAFETY: ptr/len describe our live mapping (unmapped only in Drop); the compositor writes it only between capture and ready, before we read.
        unsafe {
            std::slice::from_raw_parts(self.ptr.as_ptr().cast::<u8>(), self.len)
        }
    }
}

impl Drop for ShmBuffer {
    fn drop(&mut self) {
        self.buffer.destroy();
        self.pool.destroy();
        #[allow(unsafe_code)]
        // SAFETY: the mapping was created in new with this ptr and len and no slice borrowed from it outlives &self.
        let _ = unsafe { munmap(self.ptr.as_ptr(), self.len) };
    }
}

pub(crate) fn pick_format(offered: &[Format], prefer_alpha: bool) -> Option<Format> {
    let order: [Format; 4] = if prefer_alpha {
        [
            Format::Argb8888,
            Format::Abgr8888,
            Format::Xrgb8888,
            Format::Xbgr8888,
        ]
    } else {
        [
            Format::Xrgb8888,
            Format::Xbgr8888,
            Format::Argb8888,
            Format::Abgr8888,
        ]
    };
    order.into_iter().find(|f| offered.contains(f))
}

pub(crate) fn supported(format: Format) -> bool {
    pick_format(&[format], true).is_some()
}

pub(crate) fn to_bgra(
    src: &[u8],
    (width, height): (u32, u32),
    stride: u32,
    format: Format,
    y_invert: bool,
) -> Result<Arc<[u8]>, Error> {
    let (w, h, stride) = (width as usize, height as usize, stride as usize);
    if !supported(format) || stride < w * 4 || src.len() < stride * h {
        return Err(Error::Protocol(format!("unusable buffer {format:?}")));
    }
    let swap = matches!(format, Format::Abgr8888 | Format::Xbgr8888);
    let opaque = matches!(format, Format::Xrgb8888 | Format::Xbgr8888);
    let mut out = vec![0u8; w * h * 4];
    for (y, row) in out.chunks_exact_mut(w * 4).enumerate() {
        let sy = if y_invert { h - 1 - y } else { y };
        row.copy_from_slice(&src[sy * stride..sy * stride + w * 4]);
        if swap || opaque {
            for px in row.as_chunks_mut::<4>().0 {
                if swap {
                    px.swap(0, 2);
                }
                if opaque {
                    px[3] = 255;
                }
            }
        }
    }
    Ok(out.into())
}

impl Dispatch<WlShm, ()> for State {
    fn event(
        _: &mut Self,
        _: &WlShm,
        _: wl_shm::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlShmPool, ()> for State {
    fn event(
        _: &mut Self,
        _: &WlShmPool,
        _: <WlShmPool as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlBuffer, ()> for State {
    fn event(
        _: &mut Self,
        _: &WlBuffer,
        _: <WlBuffer as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xrgb_keeps_byte_order_and_forces_opaque_alpha() {
        let src = [1, 2, 3, 0, 4, 5, 6, 7, 99, 99];
        let out = to_bgra(&src, (2, 1), 10, Format::Xrgb8888, false).unwrap();
        assert_eq!(&*out, &[1, 2, 3, 255, 4, 5, 6, 255]);
    }

    #[test]
    fn abgr_is_swizzled_to_bgra_and_keeps_alpha() {
        let out = to_bgra(&[10, 20, 30, 40], (1, 1), 4, Format::Abgr8888, false).unwrap();
        assert_eq!(&*out, &[30, 20, 10, 40]);
    }

    #[test]
    fn y_invert_flips_rows_and_stride_padding_is_dropped() {
        let src = [1, 1, 1, 1, 0, 0, 2, 2, 2, 2, 0, 0];
        let out = to_bgra(&src, (1, 2), 6, Format::Argb8888, true).unwrap();
        assert_eq!(&*out, &[2, 2, 2, 2, 1, 1, 1, 1]);
    }

    #[test]
    fn short_buffers_and_ten_bit_formats_are_refused() {
        assert!(to_bgra(&[0; 7], (1, 2), 4, Format::Argb8888, false).is_err());
        assert!(to_bgra(&[0; 4], (1, 1), 4, Format::Xrgb2101010, false).is_err());
    }

    #[test]
    fn outputs_prefer_opaque_formats_and_windows_prefer_alpha() {
        let offered = [Format::Argb8888, Format::Xrgb8888, Format::Xrgb2101010];
        assert_eq!(pick_format(&offered, false), Some(Format::Xrgb8888));
        assert_eq!(pick_format(&offered, true), Some(Format::Argb8888));
        assert_eq!(pick_format(&[Format::Xrgb2101010], false), None);
    }
}
