use crate::Error;
use crate::stream_capture::Session;
use es_model::VideoFrame;
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamOptions {
    /// Capture requests per second at most; 0 takes every frame the compositor offers.
    pub fps: u32,
    pub paint_cursor: bool,
}

#[derive(Debug)]
pub enum StreamEvent {
    Frame(VideoFrame),
    /// The window closed.
    Stopped,
    Failed(Error),
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct StreamStats {
    pub requests: u64,
    pub frames: u64,
    /// Frames replaced in the mailbox by a newer one before they were taken.
    pub replaced: u64,
    /// Pixel buffers allocated; flat once the pool covers every frame in flight.
    pub allocations: u64,
    /// Shared-memory buffers created; one more for every change of size or format.
    pub shm_buffers: u64,
    pub failures: u64,
}

#[derive(Default)]
pub(crate) struct Counters {
    pub requests: AtomicU64,
    pub frames: AtomicU64,
    pub replaced: AtomicU64,
    pub allocations: AtomicU64,
    pub shm_buffers: AtomicU64,
    pub failures: AtomicU64,
}

pub(crate) fn bump(counter: &AtomicU64) {
    counter.fetch_add(1, Ordering::Relaxed);
}

#[derive(Default)]
struct Slot {
    frame: Option<VideoFrame>,
    end: Option<StreamEvent>,
    closed: bool,
}

impl Slot {
    fn take(&mut self) -> Option<StreamEvent> {
        self.frame
            .take()
            .map(StreamEvent::Frame)
            .or_else(|| self.end.take())
    }
}

/// The one-slot mailbox between the capture thread and the consumer: a new frame replaces an
/// untaken one, and the final event waits behind the last frame.
#[derive(Default)]
pub(crate) struct Shared {
    slot: Mutex<Slot>,
    ready: Condvar,
    pub counters: Counters,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, Slot> {
        self.slot.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn post_frame(&self, frame: VideoFrame) {
        let replaced = self.lock().frame.replace(frame);
        self.ready.notify_all();
        if replaced.is_some() {
            bump(&self.counters.replaced);
        }
    }

    pub fn post_end(&self, event: StreamEvent) {
        let mut slot = self.lock();
        if slot.end.is_none() && !slot.closed {
            slot.end = Some(event);
        }
        drop(slot);
        self.ready.notify_all();
    }

    fn close(&self) {
        self.lock().closed = true;
        self.ready.notify_all();
    }

    fn recv_timeout(&self, timeout: Duration) -> Option<StreamEvent> {
        let deadline = Instant::now().checked_add(timeout);
        let mut slot = self.lock();
        loop {
            if let Some(event) = slot.take() {
                return Some(event);
            }
            if slot.closed {
                return None;
            }
            let left = match deadline {
                Some(d) => d.checked_duration_since(Instant::now())?,
                None => timeout,
            };
            slot = self
                .ready
                .wait_timeout(slot, left)
                .unwrap_or_else(|e| e.into_inner())
                .0;
        }
    }
}

struct CloseOnExit(Arc<Shared>);

impl Drop for CloseOnExit {
    fn drop(&mut self) {
        self.0.close();
    }
}

/// One window's frames, captured on their own Wayland connection and thread until the window
/// closes, capture fails, or the stream is stopped or dropped.
pub struct ToplevelStream {
    shared: Arc<Shared>,
    wake: Option<UnixStream>,
    thread: Option<JoinHandle<()>>,
}

impl ToplevelStream {
    /// Opens a stream of the window with this foreign-toplevel identifier.
    pub fn open(identifier: &str, options: StreamOptions) -> Result<Self, Error> {
        let (wake, wake_rx) = UnixStream::pair()?;
        let shared = Arc::new(Shared::default());
        let (setup_tx, setup_rx) = std::sync::mpsc::channel();
        let identifier = identifier.to_owned();
        let thread = std::thread::Builder::new()
            .name("es-toplevel-stream".into())
            .spawn({
                let shared = shared.clone();
                move || {
                    let _close = CloseOnExit(shared.clone());
                    match Session::open(&identifier, options, shared) {
                        Ok(session) => {
                            let _ = setup_tx.send(Ok(()));
                            session.run(&wake_rx);
                        }
                        Err(e) => {
                            let _ = setup_tx.send(Err(e));
                        }
                    }
                }
            })?;
        let stream = ToplevelStream {
            shared,
            wake: Some(wake),
            thread: Some(thread),
        };
        match setup_rx.recv() {
            Ok(Ok(())) => Ok(stream),
            Ok(Err(e)) => Err(e),
            Err(_) => Err(Error::Disconnected),
        }
    }

    /// The newest frame not yet taken, then `Stopped` or `Failed` once; `None` when nothing
    /// arrives in time or the stream has ended and everything was taken.
    pub fn recv_timeout(&self, timeout: Duration) -> Option<StreamEvent> {
        self.shared.recv_timeout(timeout)
    }

    pub fn try_recv(&self) -> Option<StreamEvent> {
        self.shared.lock().take()
    }

    pub fn stats(&self) -> StreamStats {
        let c = &self.shared.counters;
        let get = |a: &AtomicU64| a.load(Ordering::Relaxed);
        StreamStats {
            requests: get(&c.requests),
            frames: get(&c.frames),
            replaced: get(&c.replaced),
            allocations: get(&c.allocations),
            shm_buffers: get(&c.shm_buffers),
            failures: get(&c.failures),
        }
    }

    /// Ends the capture thread and releases its Wayland objects; dropping the stream does the same.
    pub fn stop(self) {}
}

impl Drop for ToplevelStream {
    fn drop(&mut self) {
        // Closing our end wakes the capture thread's poll.
        self.wake = None;
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use es_model::{PixelBuf, PixelFormat};
    use std::sync::mpsc::{Sender, channel};

    fn frame(n: u8, pool: &Sender<Vec<u8>>) -> VideoFrame {
        VideoFrame {
            width: 1,
            height: 1,
            stride: 4,
            format: PixelFormat::Xrgb8888,
            data: PixelBuf::pooled(vec![n; 4], pool.clone()),
            presented: u64::from(n),
        }
    }

    fn presented(e: Option<StreamEvent>) -> u64 {
        match e {
            Some(StreamEvent::Frame(f)) => f.presented,
            other => panic!("expected a frame, got {other:?}"),
        }
    }

    #[test]
    fn the_newest_frame_wins_and_the_replaced_one_returns_to_the_pool() {
        let (pool, returned) = channel();
        let shared = Shared::default();
        shared.post_frame(frame(1, &pool));
        shared.post_frame(frame(2, &pool));
        assert_eq!(
            returned.try_recv().expect("replaced frame returned"),
            [1; 4]
        );
        assert_eq!(shared.counters.replaced.load(Ordering::Relaxed), 1);
        assert_eq!(presented(shared.recv_timeout(Duration::ZERO)), 2);
        assert!(shared.recv_timeout(Duration::from_millis(20)).is_none());
    }

    #[test]
    fn the_final_event_comes_after_the_last_frame_and_then_the_stream_is_empty() {
        let (pool, _returned) = channel();
        let shared = Shared::default();
        shared.post_frame(frame(3, &pool));
        shared.post_end(StreamEvent::Stopped);
        shared.post_end(StreamEvent::Failed(Error::Timeout));
        shared.close();
        assert_eq!(presented(shared.recv_timeout(Duration::ZERO)), 3);
        assert!(matches!(
            shared.recv_timeout(Duration::ZERO),
            Some(StreamEvent::Stopped)
        ));
        let t = Instant::now();
        assert!(shared.recv_timeout(Duration::from_secs(5)).is_none());
        assert!(
            t.elapsed() < Duration::from_secs(1),
            "a closed stream does not wait"
        );
    }

    #[test]
    fn a_waiting_consumer_wakes_when_a_frame_arrives() {
        let shared = Arc::new(Shared::default());
        let poster = {
            let shared = shared.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(50));
                shared.post_frame(frame(4, &channel().0));
            })
        };
        assert_eq!(presented(shared.recv_timeout(Duration::from_secs(5))), 4);
        let _ = poster.join();
    }
}
