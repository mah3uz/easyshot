use crate::Error;
use crate::output::{swaps_axes, transform_from_wl};
use crate::shm::{ShmBuffer, pick_format, supported, to_bgra};
use crate::stream_retry::{AfterFailure, Retries};
use crate::worker::State;
use es_geom::Rect;
use es_model::platform::{CaptureOpts, Frame, OutputId, Transform};
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};
use wayland_client::protocol::wl_shm::Format;
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle, WEnum};
use wayland_protocols::ext::image_capture_source::v1::client::{
    ext_foreign_toplevel_image_capture_source_manager_v1::ExtForeignToplevelImageCaptureSourceManagerV1,
    ext_image_capture_source_v1::ExtImageCaptureSourceV1,
    ext_output_image_capture_source_manager_v1::ExtOutputImageCaptureSourceManagerV1,
};
use wayland_protocols::ext::image_copy_capture::v1::client::{
    ext_image_copy_capture_frame_v1::{self, ExtImageCopyCaptureFrameV1, FailureReason},
    ext_image_copy_capture_manager_v1::{ExtImageCopyCaptureManagerV1, Options},
    ext_image_copy_capture_session_v1::{self, ExtImageCopyCaptureSessionV1},
};
use wayland_protocols_wlr::screencopy::v1::client::{
    zwlr_screencopy_frame_v1::{self, ZwlrScreencopyFrameV1},
    zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1,
};

pub(crate) const CAPTURE_TIMEOUT: Duration = Duration::from_secs(2);

pub(crate) enum Reply {
    Outputs(Sender<Result<Vec<Frame>, Error>>),
    Toplevel(Sender<Result<Frame, Error>>),
}

impl Reply {
    fn send(self, result: Result<Vec<Frame>, Error>) {
        match self {
            Reply::Outputs(tx) => {
                let _ = tx.send(result);
            }
            Reply::Toplevel(tx) => {
                let one =
                    result.and_then(|mut f| f.pop().ok_or(Error::CaptureFailed("no frame".into())));
                let _ = tx.send(one);
            }
        }
    }
}

pub(crate) struct Batch {
    reply: Reply,
    jobs: Vec<u64>,
    deadline: Instant,
}

enum Objects {
    Ext {
        source: ExtImageCaptureSourceV1,
        session: ExtImageCopyCaptureSessionV1,
        frame: Option<ExtImageCopyCaptureFrameV1>,
    },
    Wlr(ZwlrScreencopyFrameV1),
}

pub(crate) struct Job {
    output: Option<OutputId>,
    logical: Option<Rect>,
    transform: Transform,
    prefer_alpha: bool,
    size: (u32, u32),
    formats: Vec<Format>,
    constraints: Option<((u32, u32), Format)>,
    generation: u32,
    attached: u32,
    retries: Retries,
    wlr_buffer: Option<(Format, u32)>,
    y_invert: bool,
    buffer: Option<ShmBuffer>,
    objects: Option<Objects>,
    result: Option<Result<Frame, Error>>,
}

impl Job {
    fn new(output: Option<OutputId>, logical: Option<Rect>, transform: Transform) -> Self {
        Job {
            prefer_alpha: output.is_none(),
            output,
            logical,
            transform,
            size: (0, 0),
            formats: Vec::new(),
            constraints: None,
            generation: 0,
            attached: 0,
            retries: Retries::default(),
            wlr_buffer: None,
            y_invert: false,
            buffer: None,
            objects: None,
            result: None,
        }
    }

    fn release(&mut self) {
        match self.objects.take() {
            Some(Objects::Ext {
                source,
                session,
                frame,
            }) => {
                if let Some(frame) = frame {
                    frame.destroy();
                }
                session.destroy();
                source.destroy();
            }
            Some(Objects::Wlr(frame)) => frame.destroy(),
            None => {}
        }
        self.buffer = None;
    }

    fn frame(&self) -> Result<Frame, Error> {
        let buf = self
            .buffer
            .as_ref()
            .ok_or(Error::CaptureFailed("ready without a buffer".into()))?;
        let size_px = (buf.width, buf.height);
        let bgra = to_bgra(buf.bytes(), size_px, buf.stride, buf.format, self.y_invert)?;
        let shown_w = if swaps_axes(self.transform) {
            size_px.1
        } else {
            size_px.0
        };
        let (logical, scale) = match self.logical {
            Some(l) if l.width() > 0.0 => (l, f64::from(shown_w) / l.width()),
            _ => (
                Rect::new(0.0, 0.0, f64::from(size_px.0), f64::from(size_px.1)),
                1.0,
            ),
        };
        Ok(Frame {
            output: self.output.clone(),
            size_px,
            logical,
            scale,
            bgra,
            transform: self.transform,
            captured_at: Instant::now(),
        })
    }
}

impl State {
    pub(crate) fn capture_outputs(
        &mut self,
        ids: &[OutputId],
        opts: CaptureOpts,
        reply: Reply,
        qh: &QueueHandle<State>,
    ) {
        let mut targets = Vec::with_capacity(ids.len());
        for id in ids {
            match self
                .outputs
                .iter()
                .find(|o| o.announced.as_ref().is_some_and(|i| i.id == *id))
            {
                Some(o) => targets.push((o.wl.clone(), o.announced.clone(), o.transform())),
                None => return reply.send(Err(Error::UnknownOutput(id.0.clone()))),
            }
        }
        let ext = match (&self.ext_output_source, &self.ext_copy) {
            (Some(s), Some(c)) if self.globals.ext_output_capture() => Some((s.clone(), c.clone())),
            _ => None,
        };
        let wlr = self.wlr_screencopy.clone();
        if self.shm.is_none() || (ext.is_none() && wlr.is_none()) {
            return reply.send(Err(Error::Unsupported("output capture")));
        }
        let mut jobs = Vec::with_capacity(targets.len());
        for (wl, info, transform) in targets {
            let id = self.next_job_id();
            let mut job = Job::new(
                info.as_ref().map(|i| i.id.clone()),
                info.map(|i| i.logical),
                transform,
            );
            job.objects = match (&ext, &wlr) {
                (Some((sources, copy)), _) => {
                    job.transform = Transform::Normal;
                    let source = sources.create_source(&wl, qh, ());
                    let session = copy.create_session(&source, options(opts), qh, id);
                    Some(Objects::Ext {
                        source,
                        session,
                        frame: None,
                    })
                }
                (None, Some(m)) => Some(Objects::Wlr(m.capture_output(
                    i32::from(opts.cursor),
                    &wl,
                    qh,
                    id,
                ))),
                (None, None) => None,
            };
            self.jobs.insert(id, job);
            jobs.push(id);
        }
        self.add_batch(jobs, reply);
    }

    pub(crate) fn capture_toplevel(
        &mut self,
        identifier: &str,
        opts: CaptureOpts,
        reply: Reply,
        qh: &QueueHandle<State>,
    ) {
        let id = self.next_job_id();
        let (Some(sources), Some(copy)) = (&self.ext_toplevel_source, &self.ext_copy) else {
            return reply.send(Err(Error::Unsupported("window capture")));
        };
        if self.shm.is_none() {
            return reply.send(Err(Error::Unsupported("window capture")));
        }
        let Some(handle) = self.toplevel_handle(identifier) else {
            return reply.send(Err(Error::UnknownToplevel(identifier.to_owned())));
        };
        let source = sources.create_source(handle, qh, ());
        let session = copy.create_session(&source, options(opts), qh, id);
        let mut job = Job::new(None, None, Transform::Normal);
        job.objects = Some(Objects::Ext {
            source,
            session,
            frame: None,
        });
        self.jobs.insert(id, job);
        self.add_batch(vec![id], reply);
    }

    fn next_job_id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    fn add_batch(&mut self, jobs: Vec<u64>, reply: Reply) {
        let id = self.next_job_id();
        let deadline = Instant::now() + CAPTURE_TIMEOUT;
        self.batches.insert(
            id,
            Batch {
                reply,
                jobs,
                deadline,
            },
        );
        self.complete_batches();
    }

    fn finish(&mut self, job: u64, result: Result<Frame, Error>) {
        if let Some(j) = self.jobs.get_mut(&job) {
            j.release();
            j.result.get_or_insert(result);
        }
        self.complete_batches();
    }

    fn complete_batches(&mut self) {
        let done: Vec<u64> = self
            .batches
            .iter()
            .filter(|(_, b)| {
                b.jobs
                    .iter()
                    .all(|j| self.jobs.get(j).is_none_or(|j| j.result.is_some()))
            })
            .map(|(id, _)| *id)
            .collect();
        for id in done {
            if let Some(b) = self.batches.remove(&id) {
                let results: Result<Vec<Frame>, Error> = b
                    .jobs
                    .iter()
                    .map(|j| match self.jobs.remove(j).and_then(|j| j.result) {
                        Some(r) => r,
                        None => Err(Error::CaptureFailed("lost job".into())),
                    })
                    .collect();
                b.reply.send(results);
            }
        }
    }

    pub(crate) fn expire_captures(&mut self, now: Instant) {
        let late: Vec<u64> = self
            .batches
            .values()
            .filter(|b| b.deadline <= now)
            .flat_map(|b| b.jobs.iter().copied())
            .collect();
        for job in late {
            if let Some(j) = self.jobs.get_mut(&job) {
                j.release();
                j.result.get_or_insert(Err(Error::Timeout));
            }
        }
        self.complete_batches();
    }

    pub(crate) fn capture_deadline(&self) -> Option<Instant> {
        self.batches.values().map(|b| b.deadline).min()
    }

    fn attach_ext_buffer(&mut self, job: u64, qh: &QueueHandle<State>) -> Result<(), Error> {
        let shm = self.shm.clone().ok_or(Error::Unsupported("wl_shm"))?;
        let Some(j) = self.jobs.get_mut(&job) else {
            return Ok(());
        };
        if j.buffer.is_some() {
            return Ok(());
        }
        let (Some(Objects::Ext { session, frame, .. }), Some((size, format))) =
            (&mut j.objects, j.constraints)
        else {
            return Ok(());
        };
        let (w, h) = size;
        let buf = ShmBuffer::new(&shm, qh, size, w.saturating_mul(4), format)?;
        let f = session.create_frame(qh, job);
        f.attach_buffer(&buf.buffer);
        f.damage_buffer(0, 0, w as i32, h as i32);
        f.capture();
        *frame = Some(f);
        j.buffer = Some(buf);
        j.attached = j.generation;
        Ok(())
    }

    /// Hyprland fails each frame of a resizing window with new constraints; the deadline bounds the retries.
    fn ext_failed(&mut self, job: u64, reason: WEnum<FailureReason>, qh: &QueueHandle<State>) {
        let Some(j) = self.jobs.get_mut(&job) else {
            return;
        };
        let newer = j.generation > j.attached;
        let after = j.retries.failed(reason, newer);
        let retry = reason == WEnum::Value(FailureReason::BufferConstraints)
            && matches!(after, AfterFailure::RequestNow | AfterFailure::RetryLater);
        if !retry {
            return self.finish(job, Err(Error::CaptureFailed(format!("{reason:?}"))));
        }
        if let Some(Objects::Ext { frame, .. }) = &mut j.objects
            && let Some(f) = frame.take()
        {
            f.destroy();
        }
        j.buffer = None;
        // Otherwise the constraints that are due attach the next buffer.
        if newer && let Err(e) = self.attach_ext_buffer(job, qh) {
            self.finish(job, Err(e));
        }
    }

    fn copy_wlr_buffer(&mut self, job: u64, qh: &QueueHandle<State>) -> Result<(), Error> {
        let shm = self.shm.clone().ok_or(Error::Unsupported("wl_shm"))?;
        let Some(j) = self.jobs.get_mut(&job) else {
            return Ok(());
        };
        let Some(Objects::Wlr(frame)) = &j.objects else {
            return Ok(());
        };
        if j.buffer.is_some() {
            return Ok(());
        }
        let (format, stride) = j.wlr_buffer.ok_or(Error::Unsupported("shm format"))?;
        let buf = ShmBuffer::new(&shm, qh, j.size, stride, format)?;
        frame.copy(&buf.buffer);
        j.buffer = Some(buf);
        Ok(())
    }

    fn ready(&mut self, job: u64) {
        let result = match self.jobs.get(&job) {
            Some(j) => j.frame(),
            None => return,
        };
        self.finish(job, result);
    }
}

fn options(opts: CaptureOpts) -> Options {
    if opts.cursor {
        Options::PaintCursors
    } else {
        Options::empty()
    }
}

impl Dispatch<ExtImageCopyCaptureSessionV1, u64> for State {
    fn event(
        state: &mut Self,
        _: &ExtImageCopyCaptureSessionV1,
        event: ext_image_copy_capture_session_v1::Event,
        job: &u64,
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        use ext_image_copy_capture_session_v1::Event;
        let job = *job;
        let Some(j) = state.jobs.get_mut(&job) else {
            return;
        };
        match event {
            Event::BufferSize { width, height } => j.size = (width, height),
            Event::ShmFormat {
                format: WEnum::Value(f),
            } => j.formats.push(f),
            Event::Done => {
                let formats = std::mem::take(&mut j.formats);
                let Some(format) = pick_format(&formats, j.prefer_alpha) else {
                    return state.finish(job, Err(Error::Unsupported("shm format")));
                };
                let constraints = Some((j.size, format));
                j.retries.constraints(j.constraints != constraints);
                j.constraints = constraints;
                j.generation += 1;
                if let Err(e) = state.attach_ext_buffer(job, qh) {
                    state.finish(job, Err(e));
                }
            }
            Event::Stopped => {
                state.finish(job, Err(Error::CaptureFailed("session stopped".into())))
            }
            _ => {}
        }
    }
}

impl Dispatch<ExtImageCopyCaptureFrameV1, u64> for State {
    fn event(
        state: &mut Self,
        _: &ExtImageCopyCaptureFrameV1,
        event: ext_image_copy_capture_frame_v1::Event,
        job: &u64,
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        use ext_image_copy_capture_frame_v1::Event;
        let job = *job;
        match event {
            Event::Transform { transform } => {
                if let Some(j) = state.jobs.get_mut(&job) {
                    j.transform = transform_from_wl(transform);
                }
            }
            Event::Ready => state.ready(job),
            Event::Failed { reason } => state.ext_failed(job, reason, qh),
            _ => {}
        }
    }
}

impl Dispatch<ZwlrScreencopyFrameV1, u64> for State {
    fn event(
        state: &mut Self,
        frame: &ZwlrScreencopyFrameV1,
        event: zwlr_screencopy_frame_v1::Event,
        job: &u64,
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        use zwlr_screencopy_frame_v1::Event;
        let job = *job;
        let Some(j) = state.jobs.get_mut(&job) else {
            return;
        };
        let copy_now = match event {
            Event::Buffer {
                format: WEnum::Value(format),
                width,
                height,
                stride,
            } if supported(format) && j.wlr_buffer.is_none() => {
                j.size = (width, height);
                j.wlr_buffer = Some((format, stride));
                frame.version() < 3
            }
            Event::Flags {
                flags: WEnum::Value(flags),
            } => {
                j.y_invert = flags.contains(zwlr_screencopy_frame_v1::Flags::YInvert);
                false
            }
            Event::BufferDone => true,
            Event::Ready { .. } => {
                state.ready(job);
                false
            }
            Event::Failed => {
                state.finish(job, Err(Error::CaptureFailed("screencopy failed".into())));
                false
            }
            _ => false,
        };
        if copy_now && let Err(e) = state.copy_wlr_buffer(job, qh) {
            state.finish(job, Err(e));
        }
    }
}

macro_rules! no_events {
    ($($t:ty),*) => {$(
        impl Dispatch<$t, ()> for State {
            fn event(_: &mut Self, _: &$t, _: <$t as Proxy>::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
        }
    )*};
}
pub(crate) use no_events;

no_events!(
    ExtImageCopyCaptureManagerV1,
    ExtOutputImageCaptureSourceManagerV1,
    ExtForeignToplevelImageCaptureSourceManagerV1,
    ExtImageCaptureSourceV1,
    ZwlrScreencopyManagerV1
);
