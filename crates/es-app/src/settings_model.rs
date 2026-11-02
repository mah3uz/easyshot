use crate::errors::{Error, Result};
use bitflags::bitflags;
use es_model::Config;
use es_store::{ConfigFile, FileWatcher};
use gpui::{Context, EventEmitter, Task};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;

pub const SAVE_DEBOUNCE: Duration = Duration::from_millis(300);

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct SectionMask: u16 {
        const GENERAL = 1 << 0;
        const CAPTURE = 1 << 1;
        const OVERLAY = 1 << 2;
        const AFTER_CAPTURE = 1 << 3;
        const RECORDING = 1 << 4;
        const DEFAULT_LOOK = 1 << 5;
        const COMPOSITOR = 1 << 6;
        const EDITOR = 1 << 7;
        const OCR = 1 << 8;
        const SHORTCUTS = 1 << 9;
    }
}

impl SectionMask {
    pub fn between(a: &Config, b: &Config) -> Self {
        let mut mask = Self::empty();
        mask.set(Self::GENERAL, a.general != b.general);
        mask.set(Self::CAPTURE, a.capture != b.capture);
        mask.set(Self::OVERLAY, a.overlay != b.overlay);
        mask.set(Self::AFTER_CAPTURE, a.after_capture != b.after_capture);
        mask.set(Self::RECORDING, a.recording != b.recording);
        mask.set(Self::DEFAULT_LOOK, a.default_look != b.default_look);
        mask.set(Self::COMPOSITOR, a.compositor != b.compositor);
        mask.set(Self::EDITOR, a.editor != b.editor);
        mask.set(Self::OCR, a.ocr != b.ocr);
        mask.set(Self::SHORTCUTS, a.shortcuts != b.shortcuts);
        mask
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsEvent {
    Changed(SectionMask),
    Invalid {
        line: usize,
        col: usize,
        message: String,
    },
    SaveFailed(String),
}

pub struct SettingsModel {
    config: Arc<Config>,
    file: ConfigFile,
    tx: watch::Sender<Arc<Config>>,
    save: Option<Task<()>>,
    _watcher: Option<FileWatcher>,
}

impl EventEmitter<SettingsEvent> for SettingsModel {}

impl SettingsModel {
    /// A file that does not parse starts the model on defaults and the file is left alone.
    pub fn new(file: ConfigFile) -> Self {
        let config = Arc::new(file.config().clone());
        let (tx, _) = watch::channel(config.clone());
        Self {
            config,
            file,
            tx,
            save: None,
            _watcher: None,
        }
    }

    pub fn config(&self) -> &Arc<Config> {
        &self.config
    }

    /// The initial parse error of the file, if it had one.
    pub fn load_error(&self) -> Option<Error> {
        match self.file.error() {
            Some(es_store::Error::Parse { line, col, message }) => Some(Error::Parse {
                line: *line,
                col: *col,
                message: message.clone(),
            }),
            Some(other) => Some(Error::Store(other.to_string())),
            None => None,
        }
    }

    /// For consumers outside GPUI; the value is always the latest applied config.
    pub fn subscribe_config(&self) -> watch::Receiver<Arc<Config>> {
        self.tx.subscribe()
    }

    /// `on_change` runs on a helper thread; the app should answer it by calling `reload`.
    pub fn watch(&mut self, on_change: impl Fn() + Send + 'static) -> Result<()> {
        self._watcher = Some(FileWatcher::new(self.file.path(), on_change)?);
        Ok(())
    }

    pub fn update(&mut self, f: impl FnOnce(&mut Config), cx: &mut Context<Self>) {
        let mut next = (*self.config).clone();
        f(&mut next);
        let mask = SectionMask::between(&self.config, &next);
        if mask.is_empty() {
            return;
        }
        self.apply(next, mask, cx);
        self.save = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SAVE_DEBOUNCE).await;
            this.update(cx, |this, cx| this.flush(cx)).ok();
        }));
    }

    /// Re-reads the file after an outside edit. A file that does not parse keeps the last good config.
    pub fn reload(&mut self, cx: &mut Context<Self>) {
        match self.file.reload() {
            Ok(false) => {}
            Ok(true) => {
                let next = self.file.config().clone();
                let mask = SectionMask::between(&self.config, &next);
                self.save = None;
                if !mask.is_empty() {
                    self.apply(next, mask, cx);
                }
            }
            Err(es_store::Error::Parse { line, col, message }) => {
                cx.emit(SettingsEvent::Invalid { line, col, message });
            }
            Err(error) => {
                tracing::warn!(%error, "config reload failed");
            }
        }
    }

    /// Writes any pending change now.
    pub fn flush(&mut self, cx: &mut Context<Self>) {
        self.save = None;
        match self.file.save(&self.config) {
            Ok(()) => {}
            Err(es_store::Error::Damaged | es_store::Error::NewerVersion { .. }) => {
                tracing::debug!("config file is read-only; changes stay in memory");
            }
            Err(error) => cx.emit(SettingsEvent::SaveFailed(error.to_string())),
        }
    }

    fn apply(&mut self, next: Config, mask: SectionMask, cx: &mut Context<Self>) {
        self.config = Arc::new(next);
        self.tx.send_replace(self.config.clone());
        cx.emit(SettingsEvent::Changed(mask));
        cx.notify();
    }
}
