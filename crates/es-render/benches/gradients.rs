#![allow(clippy::print_stdout)]

use es_render::beautify::gradients::{PRESETS, render_gradient};
use std::time::Instant;

fn main() {
    let (w, h) = (2560, 1600);
    let gradients: Vec<_> = PRESETS.iter().map(|p| p.style()).collect();
    for g in &gradients {
        let _ = render_gradient(g, w, h);
    }
    let mut ms: Vec<f64> = Vec::new();
    for _ in 0..8 {
        for g in &gradients {
            let t = Instant::now();
            let p = render_gradient(g, w, h);
            ms.push(t.elapsed().as_secs_f64() * 1000.0);
            std::hint::black_box(p);
        }
    }
    ms.sort_by(f64::total_cmp);
    let at = |q: f64| ms[((ms.len() - 1) as f64 * q).round() as usize];
    println!(
        "gradient {w}x{h}: p50 {:.2} ms, p95 {:.2} ms, max {:.2} ms ({} runs)",
        at(0.5),
        at(0.95),
        ms[ms.len() - 1],
        ms.len()
    );
}
