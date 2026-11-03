use crate::capture_coordinator::{CaptureCoordinator, CaptureKind, CaptureRequest, Timer};
use crate::errors::{Error, Result};
use crate::events::Origin;
use crate::jobs::{JobId, Jobs};
use crate::windows::{WindowKey, WindowKind, Windows};
use es_model::capture::CaptureVariant;
use es_protocol::{AgentRoute, Route, SettingsPage, TimedMode};
use gpui::App;
use std::cell::RefCell;
use std::rc::Rc;

/// Routes whose work belongs to a crate above es-app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    DeckToggle,
    DeckSaveAll,
    DeckCloseAll,
    RestoreLast,
    PinLast,
    UnpinAll,
    EditClipboard,
    RecordStop,
    RecordPause,
    RecordRestart,
    RecordDiscard,
    RevokeAgent,
    ShowHome(HomeTab),
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HomeTab {
    Gallery,
    Settings(Option<SettingsPage>),
}

pub type ActionHandler = Rc<dyn Fn(Action, &mut App) -> Result<()>>;
/// Owns the job it is handed: it must finish it.
pub type AgentHandler = Rc<dyn Fn(JobId, AgentRoute, Origin, &mut App)>;

enum Target {
    Capture(CaptureRequest),
    Window(WindowKind, WindowKey),
    Home(Option<HomeTab>),
    Action(Action),
    Agent(AgentRoute),
}

fn capture(kind: CaptureKind, variant: CaptureVariant) -> Target {
    Target::Capture(CaptureRequest::new(kind, variant))
}

fn timed(mode: TimedMode, timer: Timer) -> Target {
    let kind = match mode {
        TimedMode::Region => CaptureKind::Region,
        TimedMode::Window => CaptureKind::Window,
        TimedMode::Fullscreen => CaptureKind::Fullscreen,
    };
    Target::Capture(CaptureRequest::new(kind, CaptureVariant::Normal).timed(timer))
}

fn target(route: Route) -> Target {
    use CaptureVariant::Normal;
    match route {
        Route::CaptureRegion => capture(CaptureKind::Region, Normal),
        Route::CaptureFullscreen => capture(CaptureKind::Fullscreen, Normal),
        Route::CaptureWindow => capture(CaptureKind::Window, Normal),
        Route::CapturePrevious => capture(CaptureKind::Previous, Normal),
        Route::CaptureTimed => timed(TimedMode::Region, Timer::Setting),
        Route::Timed { mode, seconds } => {
            timed(mode, seconds.map_or(Timer::Setting, Timer::Seconds))
        }
        Route::CaptureRegionCopy => capture(CaptureKind::Region, CaptureVariant::RegionCopy),
        Route::CaptureRegionSave => capture(CaptureKind::Region, CaptureVariant::RegionSave),
        Route::CaptureRegionAnnotate => {
            capture(CaptureKind::Region, CaptureVariant::RegionAnnotate)
        }
        Route::CaptureRegionPin => capture(CaptureKind::Region, CaptureVariant::RegionPin),
        Route::CaptureScroll => capture(CaptureKind::Scroll, Normal),
        Route::Ocr => capture(CaptureKind::Text, Normal),
        Route::OcrSingleLine => capture(CaptureKind::TextLine, Normal),
        Route::ColorPicker => capture(CaptureKind::Color, Normal),
        Route::Record => capture(CaptureKind::Recording, Normal),
        Route::RecordArea => capture(CaptureKind::RecordingArea, Normal),
        Route::RecordStop => Target::Action(Action::RecordStop),
        Route::RecordPause => Target::Action(Action::RecordPause),
        Route::RecordRestart => Target::Action(Action::RecordRestart),
        Route::RecordDiscard => Target::Action(Action::RecordDiscard),
        Route::CaptureBar => Target::Window(WindowKind::CaptureBar, WindowKey::Single),
        Route::Home => Target::Home(None),
        Route::Gallery => Target::Home(Some(HomeTab::Gallery)),
        Route::Open(path) => Target::Window(WindowKind::Editor, WindowKey::Path(path)),
        Route::Settings(page) => Target::Home(Some(HomeTab::Settings(page))),
        Route::RestoreLast => Target::Action(Action::RestoreLast),
        Route::PinLast => Target::Action(Action::PinLast),
        Route::UnpinAll => Target::Action(Action::UnpinAll),
        Route::EditClipboard => Target::Action(Action::EditClipboard),
        Route::DeckToggle => Target::Action(Action::DeckToggle),
        Route::DeckSaveAll => Target::Action(Action::DeckSaveAll),
        Route::DeckCloseAll => Target::Action(Action::DeckCloseAll),
        Route::Quit => Target::Action(Action::Quit),
        Route::AgentRevoke => Target::Action(Action::RevokeAgent),
        Route::Agent(route) => Target::Agent(route),
    }
}

#[derive(Clone)]
pub struct RouteDispatcher {
    jobs: Jobs,
    coordinator: CaptureCoordinator,
    windows: Windows,
    actions: Rc<RefCell<Option<ActionHandler>>>,
    agent: Rc<RefCell<Option<AgentHandler>>>,
}

impl RouteDispatcher {
    pub fn new(jobs: Jobs, coordinator: CaptureCoordinator, windows: Windows) -> Self {
        Self {
            jobs,
            coordinator,
            windows,
            actions: Rc::default(),
            agent: Rc::default(),
        }
    }

    pub fn set_action_handler(&self, handler: ActionHandler) {
        *self.actions.borrow_mut() = Some(handler);
    }

    pub fn set_agent_handler(&self, handler: AgentHandler) {
        *self.agent.borrow_mut() = Some(handler);
    }

    pub fn dispatch(&self, route: Route, origin: Origin, cx: &mut App) -> JobId {
        let job = self.jobs.allocate();
        self.dispatch_as(job, route, origin, cx);
        job
    }

    /// For commands that already carry the job id their caller waits on.
    pub fn dispatch_as(&self, job: JobId, route: Route, origin: Origin, cx: &mut App) {
        tracing::debug!(%route, ?origin, job = job.0, "dispatch");
        let result = match target(route) {
            Target::Capture(request) => {
                self.coordinator.submit(job, request, cx);
                return;
            }
            Target::Window(kind, key) => self.windows.open(kind, key, cx),
            Target::Home(tab) => self.open_home(tab, cx),
            Target::Action(action) => self.act(action, cx),
            Target::Agent(route) => {
                let handler = self.agent.borrow().clone();
                match handler {
                    Some(handler) => return handler(job, route, origin, cx),
                    None => Err(Error::Unsupported),
                }
            }
        };
        self.jobs.report(job, &result);
    }

    pub fn cancel(&self, job: JobId) -> bool {
        self.jobs.cancel(job)
    }

    fn open_home(&self, tab: Option<HomeTab>, cx: &mut App) -> Result<()> {
        self.windows.open(WindowKind::Home, WindowKey::Single, cx)?;
        match tab {
            Some(tab) => self.act(Action::ShowHome(tab), cx),
            None => Ok(()),
        }
    }

    fn act(&self, action: Action, cx: &mut App) -> Result<()> {
        let handler = self.actions.borrow().clone().ok_or(Error::Unsupported)?;
        handler(action, cx)
    }
}
