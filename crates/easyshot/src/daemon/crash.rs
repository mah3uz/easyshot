use es_desktop::{Labels, Notification, NotificationId, Notifier, Urgency};
use es_store::Paths;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::fmt::Write as _;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use tracing_subscriber::fmt::MakeWriter;

const RING_LINES: usize = 200;
const REPORTED: &str = ".reported.txt";
pub const OPEN_ACTION: &str = "open-crash-report";

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The last log lines, kept in memory so a crash report has them even when the log file is behind.
#[derive(Clone, Default)]
pub struct LogRing(Arc<Mutex<VecDeque<String>>>);

impl LogRing {
    pub fn lines(&self) -> Vec<String> {
        lock(&self.0).iter().cloned().collect()
    }
}

/// One event's text arrives in pieces; it joins the ring when the writer is dropped.
pub struct RingWriter(LogRing, Vec<u8>);

impl io::Write for RingWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.1.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Drop for RingWriter {
    fn drop(&mut self) {
        let mut ring = lock(&(self.0).0);
        for line in String::from_utf8_lossy(&self.1).lines() {
            if ring.len() == RING_LINES {
                ring.pop_front();
            }
            ring.push_back(line.to_owned());
        }
    }
}

impl<'a> MakeWriter<'a> for LogRing {
    type Writer = RingWriter;

    fn make_writer(&'a self) -> RingWriter {
        RingWriter(self.clone(), Vec::new())
    }
}

/// What a crash report says about the build and the machine, filled in as the daemon learns it.
#[derive(Clone, Default)]
pub struct Context {
    pub version: String,
    pub features: Vec<&'static str>,
    pub providers: Arc<Mutex<Vec<(String, String)>>>,
}

impl Context {
    pub fn set_providers(&self, providers: Vec<(String, String)>) {
        *lock(&self.providers) = providers;
    }
}

pub fn render(context: &Context, panic: &str, backtrace: &str, log: &[String]) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "easyshot crash report");
    let _ = writeln!(out, "version: {}", context.version);
    let _ = writeln!(out, "features: {}", context.features.join(", "));
    let _ = writeln!(out, "panic: {panic}");
    let _ = writeln!(out, "\nproviders:");
    for (capability, chosen) in lock(&context.providers).iter() {
        let _ = writeln!(out, "  {capability}: {chosen}");
    }
    let _ = writeln!(out, "\nbacktrace:\n{backtrace}");
    let _ = writeln!(out, "\nlast {} log lines:", log.len());
    for line in log {
        let _ = writeln!(out, "{line}");
    }
    out
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Writes `<dir>/<timestamp>.txt` and returns its path.
pub fn write_report(dir: &Path, timestamp: u64, text: &str) -> es_store::Result<PathBuf> {
    let path = dir.join(format!("{timestamp}.txt"));
    es_store::atomic::write_atomic(&path, text.as_bytes())?;
    Ok(path)
}

/// Chains the previous hook, so the usual panic message still reaches stderr.
pub fn install_hook(context: Context, ring: LogRing, dir: PathBuf) {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let backtrace = std::backtrace::Backtrace::force_capture().to_string();
        let text = render(&context, &info.to_string(), &backtrace, &ring.lines());
        if let Err(e) = write_report(&dir, now(), &text) {
            eprintln!("easyshot: crash report not written: {e}");
        }
        previous(info);
    }));
}

fn is_unreported(path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    name.ends_with(".txt") && !name.ends_with(REPORTED)
}

/// Reports from earlier runs not yet announced, newest last.
pub fn unreported(dir: &Path) -> Vec<PathBuf> {
    let mut found: Vec<(u64, PathBuf)> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| is_unreported(p))
        .filter_map(|p| {
            let stamp = p.file_stem()?.to_str()?.parse().ok()?;
            Some((stamp, p))
        })
        .collect();
    found.sort();
    found.into_iter().map(|(_, p)| p).collect()
}

/// A notification offers the newest unannounced report; each is marked announced so it shows once.
pub struct Announcement {
    pub id: NotificationId,
    pub path: PathBuf,
}

pub fn announce(paths: &Paths, notifier: &dyn Notifier, labels: &Labels) -> Option<Announcement> {
    let mut pending = unreported(&paths.crash_dir());
    let newest = pending.pop()?;
    let mut shown = newest.clone();
    for path in pending.iter().chain([&newest]) {
        let marked = path.with_extension("").with_extension(&REPORTED[1..]);
        match std::fs::rename(path, &marked) {
            Ok(()) if *path == newest => shown = marked,
            Ok(()) => {}
            Err(e) => tracing::warn!(error = %e, "crash report not marked"),
        }
    }
    let sent = notifier.notify(Notification {
        title: labels("crash-notice-title"),
        body: labels("crash-notice-body"),
        icon: None,
        image: None,
        actions: vec![(OPEN_ACTION.to_owned(), labels("crash-notice-open"))],
        urgency: Urgency::Normal,
        timeout: None,
    });
    match sent {
        Ok(id) => Some(Announcement { id, path: shown }),
        Err(e) => {
            tracing::warn!(error = %e, "crash notice");
            None
        }
    }
}

/// Answers the notification's "Open crash report" button; any other action is not ours.
pub fn on_action(
    slot: &RefCell<Option<Announcement>>,
    opener: &dyn Fn(&Path),
    id: NotificationId,
    action: &str,
) {
    if action != OPEN_ACTION {
        return;
    }
    let slot = slot.borrow();
    if let Some(shown) = slot.as_ref().filter(|a| a.id == id) {
        opener(&shown.path);
    }
}

pub fn open_with_default_app(path: &Path) {
    let spawned = es_desktop::spawn_reaped(
        std::process::Command::new("xdg-open")
            .arg(path)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null()),
    );
    if let Err(e) = spawned {
        tracing::warn!(error = %e, "xdg-open");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use es_desktop::RecordingNotifier;
    use std::process::Command;

    fn labels() -> Labels {
        Arc::new(|id: &str| format!("<{id}>"))
    }

    fn context() -> Context {
        let context = Context {
            version: "9.9.9".into(),
            features: vec!["gui", "sound"],
            providers: Arc::default(),
        };
        context.set_providers(vec![("capture".into(), "wlr-screencopy".into())]);
        context
    }

    #[test]
    fn the_ring_keeps_only_the_last_two_hundred_lines() {
        use std::io::Write as _;
        let ring = LogRing::default();
        for n in 0..250 {
            writeln!(ring.make_writer(), "line {n}").unwrap();
        }
        let lines = ring.lines();
        assert_eq!(lines.len(), RING_LINES);
        assert_eq!(lines.first().unwrap(), "line 50");
        assert_eq!(lines.last().unwrap(), "line 249");
    }

    #[test]
    fn a_report_names_the_build_the_providers_and_the_log() {
        let text = render(
            &context(),
            "boom at x.rs:1",
            "frame 0",
            &["a".into(), "b".into()],
        );
        for want in [
            "version: 9.9.9",
            "features: gui, sound",
            "panic: boom at x.rs:1",
            "capture: wlr-screencopy",
            "frame 0",
            "last 2 log lines:\na\nb",
        ] {
            assert!(text.contains(want), "{want} missing from\n{text}");
        }
    }

    #[test]
    fn a_real_panic_in_a_child_process_leaves_a_report_with_its_log() {
        const CHILD: &str = "EASYSHOT_CRASH_TEST_DIR";
        if let Some(dir) = std::env::var_os(CHILD) {
            let ring = LogRing::default();
            {
                use std::io::Write as _;
                writeln!(ring.make_writer(), "last words before the crash").unwrap();
            }
            install_hook(context(), ring, PathBuf::from(dir));
            panic!("forced for the test");
        }
        let dir = tempfile::tempdir().unwrap();
        let status = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "daemon::crash::tests::a_real_panic_in_a_child_process_leaves_a_report_with_its_log",
                "--nocapture",
            ])
            .env(CHILD, dir.path())
            .env_remove("WAYLAND_DISPLAY")
            .env_remove("DISPLAY")
            .env_remove("DBUS_SESSION_BUS_ADDRESS")
            .output()
            .unwrap();
        assert!(!status.status.success());
        let reports = unreported(dir.path());
        assert_eq!(reports.len(), 1, "{reports:?}");
        let text = std::fs::read_to_string(&reports[0]).unwrap();
        assert!(text.contains("forced for the test"), "{text}");
        assert!(text.contains("last words before the crash"), "{text}");
        assert!(
            text.contains("version: 9.9.9") && text.contains("backtrace:"),
            "{text}"
        );
    }

    #[test]
    fn the_next_start_offers_the_report_once() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::rooted(dir.path());
        paths.ensure_all().unwrap();
        let older = write_report(&paths.crash_dir(), 100, "old").unwrap();
        let newer = write_report(&paths.crash_dir(), 200, "new").unwrap();
        let notifier = RecordingNotifier::default();

        let shown = announce(&paths, &notifier, &labels()).unwrap();
        let sent = notifier.sent();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].title, "<crash-notice-title>");
        assert_eq!(sent[0].actions[0].0, OPEN_ACTION);
        assert_eq!(std::fs::read_to_string(&shown.path).unwrap(), "new");
        assert!(!older.exists() && !newer.exists(), "both marked announced");

        assert!(announce(&paths, &notifier, &labels()).is_none());
        assert_eq!(notifier.sent().len(), 1, "nothing new, nothing announced");
    }

    #[test]
    fn only_the_open_button_of_our_notification_opens_the_report() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::rooted(dir.path());
        paths.ensure_all().unwrap();
        write_report(&paths.crash_dir(), 5, "r").unwrap();
        let notifier = RecordingNotifier::default();
        let slot = RefCell::new(announce(&paths, &notifier, &labels()));
        let id = slot.borrow().as_ref().unwrap().id;
        let opened = RefCell::new(Vec::new());
        let opener = |p: &Path| opened.borrow_mut().push(p.to_owned());

        on_action(&slot, &opener, NotificationId(id.0 + 1), OPEN_ACTION);
        on_action(&slot, &opener, id, "something-else");
        assert!(opened.borrow().is_empty());
        on_action(&slot, &opener, id, OPEN_ACTION);
        assert_eq!(opened.borrow().len(), 1);
        assert!(opened.borrow()[0].ends_with("5.reported.txt"));
    }
}
