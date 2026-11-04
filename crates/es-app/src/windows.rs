use crate::errors::{Error, Result};
use gpui::{AnyWindowHandle, App};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WindowKind {
    Home,
    Editor,
    CaptureBar,
    Onboarding,
    Preview,
}

impl WindowKind {
    fn toggles(self) -> bool {
        self == WindowKind::CaptureBar
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum WindowKey {
    Single,
    Path(PathBuf),
}

impl WindowKey {
    fn canonical(self) -> Self {
        match self {
            WindowKey::Path(p) => WindowKey::Path(p.canonicalize().unwrap_or(p)),
            single => single,
        }
    }
}

pub trait WindowHandle {
    fn is_open(&self, cx: &App) -> bool;
    fn focus(&self, cx: &mut App);
    fn close(&self, cx: &mut App);
}

impl WindowHandle for AnyWindowHandle {
    fn is_open(&self, cx: &App) -> bool {
        cx.windows().contains(self)
    }

    fn focus(&self, cx: &mut App) {
        self.update(cx, |_, window, _| window.activate_window())
            .ok();
    }

    fn close(&self, cx: &mut App) {
        self.update(cx, |_, window, _| window.remove_window()).ok();
    }
}

pub type Opener = Rc<dyn Fn(&WindowKey, &mut App) -> Result<Rc<dyn WindowHandle>>>;

#[derive(Default)]
struct Inner {
    openers: RefCell<HashMap<WindowKind, Opener>>,
    open: RefCell<HashMap<(WindowKind, WindowKey), Rc<dyn WindowHandle>>>,
}

#[derive(Clone, Default)]
pub struct Windows {
    inner: Rc<Inner>,
}

impl Windows {
    pub fn register(&self, kind: WindowKind, opener: Opener) {
        self.inner.openers.borrow_mut().insert(kind, opener);
    }

    /// Focuses the window already open for `key` (closes it, for a kind that toggles), otherwise opens one.
    pub fn open(&self, kind: WindowKind, key: WindowKey, cx: &mut App) -> Result<()> {
        let key = key.canonical();
        let existing = self.inner.open.borrow().get(&(kind, key.clone())).cloned();
        if let Some(handle) = existing {
            if handle.is_open(cx) {
                if kind.toggles() {
                    handle.close(cx);
                    self.forget(kind, &key);
                } else {
                    handle.focus(cx);
                }
                return Ok(());
            }
            self.forget(kind, &key);
        }
        let opener = self
            .inner
            .openers
            .borrow()
            .get(&kind)
            .cloned()
            .ok_or(Error::Unsupported)?;
        let handle = opener(&key, cx)?;
        self.inner.open.borrow_mut().insert((kind, key), handle);
        Ok(())
    }

    /// True when the window was open.
    pub fn close(&self, kind: WindowKind, key: &WindowKey, cx: &mut App) -> bool {
        let key = key.clone().canonical();
        let existing = self.inner.open.borrow().get(&(kind, key.clone())).cloned();
        let Some(handle) = existing else {
            return false;
        };
        let open = handle.is_open(cx);
        if open {
            handle.close(cx);
        }
        self.forget(kind, &key);
        open
    }

    pub fn forget(&self, kind: WindowKind, key: &WindowKey) {
        self.inner
            .open
            .borrow_mut()
            .remove(&(kind, key.clone().canonical()));
    }

    pub fn is_open(&self, kind: WindowKind, key: &WindowKey, cx: &App) -> bool {
        self.inner
            .open
            .borrow()
            .get(&(kind, key.clone().canonical()))
            .is_some_and(|h| h.is_open(cx))
    }
}
