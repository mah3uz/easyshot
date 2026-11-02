use super::encoder::{Encoder, pick_encoder};
use crate::{Clock, Codec, Error, RecorderEvent, RecordingHandle, RecordingOutput, Result};
use futures::future::BoxFuture;
use gstreamer as gst;
use gstreamer::prelude::*;
use std::os::fd::{AsRawFd, OwnedFd};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

pub enum CaptureSource {
    PipeWire {
        fd: OwnedFd,
        node: u32,
        crop: Option<[i32; 4]>,
    },
    Test {
        width: i32,
        height: i32,
        fps: i32,
    },
}

#[derive(Default)]
struct Gate {
    paused: bool,
    resume_pending: bool,
    offset: u64,
    last_out: Option<u64>,
    first_us: Option<u64>,
}

type Cleanup = Box<dyn FnOnce() + Send>;

pub struct GstCapture {
    pipeline: gst::Pipeline,
    out: PathBuf,
    gate: Arc<Mutex<Gate>>,
    clock: Arc<dyn Clock>,
    eos: Arc<(Mutex<bool>, Condvar)>,
    quit: Arc<AtomicBool>,
    bus_thread: Option<std::thread::JoinHandle<()>>,
    events_tx: async_channel::Sender<RecorderEvent>,
    events_rx: async_channel::Receiver<RecorderEvent>,
    seg_start: u64,
    segments: Vec<(u64, u64)>,
    cleanup: Option<Cleanup>,
    _fd: Option<OwnedFd>,
}

fn proto(msg: impl std::fmt::Display) -> Error {
    Error::Protocol(msg.to_string())
}

fn make(factory: &str) -> Result<gst::Element> {
    gst::ElementFactory::make(factory)
        .build()
        .map_err(|e| proto(format!("{factory}: {e}")))
}

pub fn element_available(name: &str) -> bool {
    gst::ElementFactory::find(name).is_some()
}

pub fn gst_init() -> Result<()> {
    gst::init().map_err(proto)
}

fn source_chain(source: CaptureSource) -> Result<(Vec<gst::Element>, Option<OwnedFd>)> {
    let mut chain = Vec::new();
    match source {
        CaptureSource::PipeWire { fd, node, crop } => {
            let src = make("pipewiresrc")?;
            src.set_property("fd", fd.as_raw_fd());
            src.set_property("path", node.to_string());
            src.set_property("do-timestamp", true);
            src.set_property("always-copy", true);
            chain.push(src);
            chain.push(make("videoconvert")?);
            if let Some([l, t, r, b]) = crop {
                let c = make("videocrop")?;
                c.set_property("left", l);
                c.set_property("top", t);
                c.set_property("right", r);
                c.set_property("bottom", b);
                chain.push(c);
            }
            Ok((chain, Some(fd)))
        }
        CaptureSource::Test { width, height, fps } => {
            let src = make("videotestsrc")?;
            src.set_property("is-live", true);
            chain.push(src);
            let caps = make("capsfilter")?;
            caps.set_property(
                "caps",
                gst::Caps::builder("video/x-raw")
                    .field("width", width)
                    .field("height", height)
                    .field("framerate", gst::Fraction::new(fps, 1))
                    .build(),
            );
            chain.push(caps);
            Ok((chain, None))
        }
    }
}

fn install_gate(
    gate_el: &gst::Element,
    gate: Arc<Mutex<Gate>>,
    clock: Arc<dyn Clock>,
) -> Result<()> {
    let pad = gate_el
        .static_pad("src")
        .ok_or_else(|| proto("gate has no src pad"))?;
    pad.add_probe(gst::PadProbeType::BUFFER, move |_, info| {
        let Some(gst::PadProbeData::Buffer(buf)) = info.data.as_mut() else {
            return gst::PadProbeReturn::Ok;
        };
        let Ok(mut g) = gate.lock() else {
            return gst::PadProbeReturn::Ok;
        };
        if g.paused {
            return gst::PadProbeReturn::Drop;
        }
        if g.first_us.is_none() {
            g.first_us = Some(clock.monotonic_us());
        }
        let dur = buf.duration().map_or(0, |d| d.nseconds());
        let b = buf.make_mut();
        if let Some(pts) = b.pts().map(|p| p.nseconds()) {
            if g.resume_pending {
                g.resume_pending = false;
                if let Some(last) = g.last_out {
                    g.offset = pts.saturating_sub(last + dur);
                }
            }
            let out_pts = pts.saturating_sub(g.offset);
            b.set_pts(gst::ClockTime::from_nseconds(out_pts));
            if let Some(dts) = b.dts().map(|d| d.nseconds()) {
                b.set_dts(gst::ClockTime::from_nseconds(dts.saturating_sub(g.offset)));
            }
            g.last_out = Some(out_pts);
        }
        gst::PadProbeReturn::Ok
    });
    Ok(())
}

impl GstCapture {
    pub fn start(
        source: CaptureSource,
        out: &Path,
        encoder: Option<Encoder>,
        clock: Arc<dyn Clock>,
        cleanup: Option<Cleanup>,
    ) -> Result<Self> {
        gst_init()?;
        let enc = encoder
            .or_else(|| pick_encoder(element_available))
            .ok_or(Error::Unsupported)?;
        let pipeline = gst::Pipeline::new();
        let (mut chain, keep_fd) = source_chain(source)?;
        let gate_el = make("identity")?;
        chain.push(gate_el.clone());
        chain.push(make("videoconvert")?);
        let enc_el = make(enc.element)?;
        for (k, v) in enc.props {
            enc_el.set_property_from_str(k, v);
        }
        chain.push(enc_el);
        if enc.codec == Codec::H264 && element_available("h264parse") {
            chain.push(make("h264parse")?);
        }
        chain.push(make("matroskamux")?);
        let sink = make("filesink")?;
        sink.set_property("location", out.to_string_lossy().to_string());
        chain.push(sink);
        let refs: Vec<&gst::Element> = chain.iter().collect();
        pipeline.add_many(refs.clone()).map_err(proto)?;
        gst::Element::link_many(refs).map_err(proto)?;

        let gate = Arc::new(Mutex::new(Gate::default()));
        install_gate(&gate_el, gate.clone(), clock.clone())?;

        let (tx, rx) = async_channel::unbounded();
        let eos = Arc::new((Mutex::new(false), Condvar::new()));
        let quit = Arc::new(AtomicBool::new(false));
        let bus = pipeline.bus().ok_or_else(|| proto("no bus"))?;
        let (eos_t, quit_t, tx_t) = (eos.clone(), quit.clone(), tx.clone());
        let bus_thread = std::thread::spawn(move || {
            while !quit_t.load(Ordering::Relaxed) {
                let Some(msg) = bus.timed_pop_filtered(
                    gst::ClockTime::from_mseconds(50),
                    &[gst::MessageType::Error, gst::MessageType::Eos],
                ) else {
                    continue;
                };
                match msg.view() {
                    gst::MessageView::Error(e) => {
                        let _ = tx_t.try_send(RecorderEvent::Failed(e.error().to_string()));
                    }
                    gst::MessageView::Eos(_) => {
                        if let Ok(mut d) = eos_t.0.lock() {
                            *d = true;
                        }
                        eos_t.1.notify_all();
                    }
                    _ => {}
                }
            }
        });

        pipeline.set_state(gst::State::Playing).map_err(proto)?;
        let _ = tx.try_send(RecorderEvent::Started);
        let seg_start = clock.monotonic_us();
        Ok(Self {
            pipeline,
            out: out.to_path_buf(),
            gate,
            clock,
            eos,
            quit,
            bus_thread: Some(bus_thread),
            events_tx: tx,
            events_rx: rx,
            seg_start,
            segments: Vec::new(),
            cleanup,
            _fd: keep_fd,
        })
    }

    fn shutdown(&mut self) {
        let _ = self.pipeline.set_state(gst::State::Null);
        self.quit.store(true, Ordering::Relaxed);
        if let Some(t) = self.bus_thread.take() {
            let _ = t.join();
        }
        if let Some(c) = self.cleanup.take() {
            c();
        }
    }

    fn finish(&mut self) -> Result<RecordingOutput> {
        let now = self.clock.monotonic_us();
        let (paused, first) = {
            let g = self.gate.lock().map_err(|_| proto("gate poisoned"))?;
            (g.paused, g.first_us)
        };
        if !paused {
            self.segments.push((self.seg_start, now));
        }
        let first = first.ok_or_else(|| proto("no frames were captured"))?;
        self.pipeline.send_event(gst::event::Eos::new());
        let timed_out = {
            let (m, cv) = &*self.eos;
            let guard = m.lock().map_err(|_| proto("eos poisoned"))?;
            cv.wait_timeout_while(guard, Duration::from_secs(10), |d| !*d)
                .map_err(|_| proto("eos poisoned"))?
                .1
                .timed_out()
        };
        self.shutdown();
        if timed_out {
            return Err(Error::Timeout);
        }
        Ok(RecordingOutput {
            file: self.out.clone(),
            first_frame_monotonic_us: first,
            segments: std::mem::take(&mut self.segments),
        })
    }
}

impl Drop for GstCapture {
    fn drop(&mut self) {
        self.shutdown();
    }
}

impl RecordingHandle for GstCapture {
    fn pause(&mut self) -> Result<()> {
        let now = self.clock.monotonic_us();
        let mut g = self.gate.lock().map_err(|_| proto("gate poisoned"))?;
        if g.paused {
            return Ok(());
        }
        g.paused = true;
        self.segments.push((self.seg_start, now));
        let _ = self.events_tx.try_send(RecorderEvent::Paused);
        Ok(())
    }

    fn resume(&mut self) -> Result<()> {
        let now = self.clock.monotonic_us();
        let mut g = self.gate.lock().map_err(|_| proto("gate poisoned"))?;
        if !g.paused {
            return Ok(());
        }
        g.paused = false;
        g.resume_pending = true;
        self.seg_start = now;
        let _ = self.events_tx.try_send(RecorderEvent::Resumed);
        Ok(())
    }

    fn stop(mut self: Box<Self>) -> BoxFuture<'static, Result<RecordingOutput>> {
        let (tx, rx) = futures::channel::oneshot::channel();
        std::thread::spawn(move || {
            let _ = tx.send(self.finish());
        });
        Box::pin(async move { rx.await.map_err(|_| Error::Cancelled)? })
    }

    fn discard(mut self: Box<Self>) -> BoxFuture<'static, Result<()>> {
        let (tx, rx) = futures::channel::oneshot::channel();
        std::thread::spawn(move || {
            self.shutdown();
            let _ = std::fs::remove_file(&self.out);
            let _ = tx.send(Ok(()));
        });
        Box::pin(async move { rx.await.map_err(|_| Error::Cancelled)? })
    }

    fn events(&self) -> async_channel::Receiver<RecorderEvent> {
        self.events_rx.clone()
    }
}
