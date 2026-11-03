use es_model::{ActionRecord, AgentCfg, AlwaysGrant, Consent};
use es_protocol::agent::{GrantSpan, GrantStatus};
use es_store::agent::AgentFile;
use futures::FutureExt as _;
use futures::future::Shared;
use gpui::{App, Context, Entity, EventEmitter, Global, Task};
use std::rc::Rc;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

pub type Clock = Rc<dyn Fn() -> OffsetDateTime>;

pub const HOUR: time::Duration = time::Duration::HOUR;

pub fn system_clock() -> Clock {
    Rc::new(OffsetDateTime::now_utc)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    Allow(GrantSpan),
    /// This request only, with nothing remembered.
    Once,
    Deny,
}

impl Answer {
    pub fn allows(self) -> bool {
        !matches!(self, Answer::Deny)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsentRequest {
    pub route: &'static str,
    pub target: Option<String>,
    pub pid: Option<u32>,
    pub comm: Option<String>,
    /// False when every request is asked about, so the prompt offers this request only.
    pub remember: bool,
}

/// A settings change, asked for from the command line, that gives agents more than they had.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Loosening {
    TurnOn,
    AlwaysAllow,
    WiderScope,
}

impl Loosening {
    pub fn between(now: &AgentCfg, next: &AgentCfg) -> Option<Loosening> {
        let wider = |now: &[String], next: &[String]| {
            !now.is_empty() && (next.is_empty() || next.iter().any(|n| !now.contains(n)))
        };
        if next.enabled && !now.enabled {
            Some(Loosening::TurnOn)
        } else if next.consent == Consent::AlwaysAllow && now.consent != Consent::AlwaysAllow {
            Some(Loosening::AlwaysAllow)
        } else if wider(&now.allowed_outputs, &next.allowed_outputs)
            || wider(&now.allowed_windows, &next.allowed_windows)
        {
            Some(Loosening::WiderScope)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeRequest {
    pub change: Loosening,
    pub pid: Option<u32>,
    pub comm: Option<String>,
}

/// Asks the user whether an agent may go ahead; the daemon awaits the answer.
pub trait ConsentPrompt {
    fn ask(&self, request: ConsentRequest, cx: &mut App) -> Task<Answer>;
    /// Resolves true only when the user clicks to allow the change.
    fn confirm(&self, request: ChangeRequest, cx: &mut App) -> Task<bool>;
}

/// Used until a prompt is installed: every request it is asked about is refused.
pub struct NoPrompt;

impl ConsentPrompt for NoPrompt {
    fn ask(&self, request: ConsentRequest, _cx: &mut App) -> Task<Answer> {
        tracing::warn!(route = request.route, "no consent prompt available; denied");
        Task::ready(Answer::Deny)
    }

    fn confirm(&self, request: ChangeRequest, _cx: &mut App) -> Task<bool> {
        tracing::warn!(change = ?request.change, "no consent prompt available; refused");
        Task::ready(false)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grant {
    Until(OffsetDateTime),
    Session,
    Always(OffsetDateTime),
}

impl Grant {
    pub fn status(self) -> GrantStatus {
        match self {
            Grant::Until(t) => GrantStatus {
                span: GrantSpan::Hour,
                expires: t.format(&Rfc3339).ok(),
            },
            Grant::Session => GrantStatus {
                span: GrantSpan::Session,
                expires: None,
            },
            Grant::Always(_) => GrantStatus {
                span: GrantSpan::Always,
                expires: None,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccessEvent {
    /// The grant now in force, if any.
    GrantChanged(Option<Grant>),
    Logged,
}

/// The daemon's agent grants and its log of agent actions.
pub struct AgentAccess {
    session: Option<Grant>,
    file: AgentFile,
    clock: Clock,
    prompt: Rc<dyn ConsentPrompt>,
    pending: Option<Shared<Task<Answer>>>,
    expiry: Option<Task<()>>,
}

impl EventEmitter<AccessEvent> for AgentAccess {}

pub struct AgentAccessHandle(pub Entity<AgentAccess>);

impl Global for AgentAccessHandle {}

impl AgentAccess {
    pub fn new(file: AgentFile, clock: Clock) -> Self {
        Self {
            session: None,
            file,
            clock,
            prompt: Rc::new(NoPrompt),
            pending: None,
            expiry: None,
        }
    }

    pub fn set_prompt(&mut self, prompt: Rc<dyn ConsentPrompt>) {
        self.prompt = prompt;
    }

    pub fn prompt(&self) -> Rc<dyn ConsentPrompt> {
        self.prompt.clone()
    }

    pub fn now(&self) -> OffsetDateTime {
        (self.clock)()
    }

    pub fn grant(&self) -> Option<Grant> {
        if let Some(always) = self.file.value.always {
            return Some(Grant::Always(always.granted_at));
        }
        match self.session {
            Some(Grant::Until(t)) if t <= self.now() => None,
            other => other,
        }
    }

    /// Newest first.
    pub fn log(&self) -> &[ActionRecord] {
        &self.file.value.log
    }

    pub fn allow(&mut self, span: GrantSpan, cx: &mut Context<Self>) {
        let now = self.now();
        match span {
            GrantSpan::Hour => {
                self.session = Some(Grant::Until(now + HOUR));
                self.schedule_expiry(cx);
            }
            GrantSpan::Session => {
                self.session = Some(Grant::Session);
                self.expiry = None;
            }
            GrantSpan::Always => {
                self.file.value.always = Some(AlwaysGrant { granted_at: now });
                self.save();
            }
        }
        cx.emit(AccessEvent::GrantChanged(self.grant()));
    }

    /// Ends every grant, the lasting one included.
    pub fn revoke(&mut self, cx: &mut Context<Self>) {
        let had = self.grant().is_some() || self.session.is_some();
        self.session = None;
        self.expiry = None;
        if self.file.value.always.take().is_some() {
            self.save();
        }
        if had {
            cx.emit(AccessEvent::GrantChanged(None));
        }
    }

    pub fn record(&mut self, entry: ActionRecord, limit: usize, cx: &mut Context<Self>) {
        self.file.value.push(entry, limit);
        self.save();
        cx.emit(AccessEvent::Logged);
    }

    /// Resolves true when the request may go ahead. Concurrent requests share one prompt
    /// unless every request is to be asked about.
    pub fn consent(
        this: &Entity<Self>,
        consent: Consent,
        request: ConsentRequest,
        cx: &mut App,
    ) -> Task<bool> {
        match consent {
            Consent::AlwaysAllow => Task::ready(true),
            Consent::OncePerSession if this.read(cx).grant().is_some() => Task::ready(true),
            Consent::OncePerSession => {
                let answer = Self::shared_prompt(this, request, cx);
                cx.spawn(async move |_| answer.await.allows())
            }
            Consent::AlwaysAsk => {
                let prompt = this.read(cx).prompt.clone();
                let answer = prompt.ask(
                    ConsentRequest {
                        remember: false,
                        ..request
                    },
                    cx,
                );
                cx.spawn(async move |_| answer.await.allows())
            }
        }
    }

    fn shared_prompt(
        this: &Entity<Self>,
        request: ConsentRequest,
        cx: &mut App,
    ) -> Shared<Task<Answer>> {
        if let Some(pending) = this.read(cx).pending.clone() {
            return pending;
        }
        let prompt = this.read(cx).prompt.clone();
        let asked = prompt.ask(
            ConsentRequest {
                remember: true,
                ..request
            },
            cx,
        );
        let entity = this.downgrade();
        let answered = cx
            .spawn(async move |cx| {
                let answer = asked.await;
                let _ = entity.update(cx, |access, cx| {
                    access.pending = None;
                    if let Answer::Allow(span) = answer {
                        access.allow(span, cx);
                    }
                });
                answer
            })
            .shared();
        this.update(cx, |access, _| access.pending = Some(answered.clone()));
        answered
    }

    fn schedule_expiry(&mut self, cx: &mut Context<Self>) {
        let Some(Grant::Until(until)) = self.session else {
            self.expiry = None;
            return;
        };
        let left = std::time::Duration::try_from(until - self.now()).unwrap_or_default();
        self.expiry = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(left).await;
            let _ = this.update(cx, |access, cx| match access.session {
                Some(Grant::Until(t)) if t > access.now() => access.schedule_expiry(cx),
                Some(Grant::Until(_)) => {
                    access.session = None;
                    access.expiry = None;
                    cx.emit(AccessEvent::GrantChanged(access.grant()));
                }
                _ => {}
            });
        }));
    }

    fn save(&self) {
        match self.file.save() {
            Ok(()) => {}
            Err(es_store::Error::NewerVersion { .. }) => {
                tracing::debug!("agent record is newer; kept in memory");
            }
            Err(e) => tracing::warn!(error = %e, "agent record not written"),
        }
    }
}
