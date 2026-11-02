use rayon::prelude::*;
use tiny_skia::{Mask, Pixmap};

/// Premultiplied source-over at an integer offset, through an optional coverage mask the size of `src`.
pub fn over(target: &mut Pixmap, src: &Pixmap, x: i32, y: i32, mask: Option<&Mask>) {
    if mask.is_some_and(|m| (m.width(), m.height()) != (src.width(), src.height())) {
        return;
    }
    let (x, y) = (i64::from(x), i64::from(y));
    let (tw, th) = (i64::from(target.width()), i64::from(target.height()));
    let (sw, sh) = (i64::from(src.width()), i64::from(src.height()));
    let (x0, x1) = (x.max(0), (x + sw).min(tw));
    let (y0, y1) = (y.max(0), (y + sh).min(th));
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    let (tw, sw) = (tw as usize, sw as usize);
    let cols = (x1 - x0) as usize;
    let (sx, dx) = ((x0 - x) as usize, x0 as usize);
    let (first, sy0) = (y0 as usize, (y0 - y) as usize);
    let rows = &mut target.data_mut()[first * tw * 4..y1 as usize * tw * 4];
    let source = src.data();
    rows.par_chunks_exact_mut(tw * 4)
        .enumerate()
        .for_each(|(row, dst)| {
            let sy = sy0 + row;
            let s = &source[(sy * sw + sx) * 4..(sy * sw + sx + cols) * 4];
            let d = &mut dst[dx * 4..(dx + cols) * 4];
            let cover = mask.map(|m| &m.data()[sy * sw + sx..sy * sw + sx + cols]);
            over_row(d, s, cover);
        });
}

fn over_row(dst: &mut [u8], src: &[u8], cover: Option<&[u8]>) {
    let (dst, src) = (dst.as_chunks_mut::<4>().0, src.as_chunks::<4>().0);
    for (i, (d, s)) in dst.iter_mut().zip(src).enumerate() {
        let c = cover.map_or(255, |c| c[i]);
        let s = if c == 255 { *s } else { s.map(|v| mul(v, c)) };
        match s[3] {
            255 => *d = s,
            0 => {}
            a => {
                let keep = 255 - a;
                for k in 0..4 {
                    d[k] = s[k].saturating_add(mul(d[k], keep));
                }
            }
        }
    }
}

pub(crate) fn mul(a: u8, b: u8) -> u8 {
    ((u16::from(a) * u16::from(b) + 127) / 255) as u8
}
