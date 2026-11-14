use es_app::agent_access::{
    AccessEvent, AgentAccess, Answer, ChangeRequest, Clock, ConsentPrompt, ConsentRequest, Grant,
    HOUR,
};
use es_model::Consent;
use es_protocol::agent::GrantSpan;
use es_store::Paths;
use futures::channel::oneshot;
use gpui::{App, AppContext as _, Entity, Task, TestAppContext};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use time::OffsetDateTime;

struct FakeClock(Rc<Cell<OffsetDateTime>>);

impl FakeClock {
    fn new() -> Self {
        FakeClock(Rc::new(Cell::new(OffsetDateTime::UNIX_EPOCH + HOUR * 24)))
    }

    fn clock(&self) -> Clock {
        let now = self.0.clone();
        Rc::new(move || now.get())
    }

    fn advance(&self, by: time::Duration) {
        self.0.set(self.0.get() + by);
    }
}

/// Answers each request through a channel the test holds, so it decides when the user replies.
#[derive(Default)]
struct HeldPrompt {
    asked: RefCell<Vec<(ConsentRequest, oneshot::Sender<Answer>)>>,
}

impl HeldPrompt {
    fn answer_all(&self, answer: Answer) {
        for (_, reply) in self.asked.borrow_mut().drain(..) {
            let _ = reply.send(answer);
        }
    }
}

impl ConsentPrompt for HeldPrompt {
    fn ask(&self, request: ConsentRequest, cx: &mut App) -> Task<Answer> {
        let (tx, rx) = oneshot::channel();
        self.asked.borrow_mut().push((request, tx));
        cx.spawn(async move |_| rx.await.unwrap_or(Answer::Deny))
    }

    fn confirm(&self, _: ChangeRequest, _: &mut App) -> Task<bool> {
        Task::ready(false)
    }
}

struct Rig {
    _dir: tempfile::TempDir,
    paths: Paths,
    clock: FakeClock,
    prompt: Rc<HeldPrompt>,
    access: Entity<AgentAccess>,
    events: Rc<RefCell<Vec<AccessEvent>>>,
    _sub: gpui::Subscription,
}

fn open(
    paths: &Paths,
    clock: &FakeClock,
    prompt: &Rc<HeldPrompt>,
    cx: &mut TestAppContext,
) -> Entity<AgentAccess> {
    let file = es_store::agent::open(paths);
    let prompt: Rc<dyn ConsentPrompt> = prompt.clone();
    cx.new(|_| {
        let mut access = AgentAccess::new(file, clock.clock());
        access.set_prompt(prompt);
        access
    })
}

fn rig(cx: &mut TestAppContext) -> Rig {
    let dir = tempfile::tempdir().expect("test setup");
    let paths = Paths::rooted(dir.path());
    std::fs::create_dir_all(dir.path().join("state")).expect("test setup");
    let clock = FakeClock::new();
    let prompt = Rc::new(HeldPrompt::default());
    let access = open(&paths, &clock, &prompt, cx);
    let events = Rc::new(RefCell::new(Vec::new()));
    let seen = events.clone();
    let sub = cx.update(|cx| {
        cx.subscribe(&access, move |_, e: &AccessEvent, _| {
            seen.borrow_mut().push(e.clone())
        })
    });
    Rig {
        _dir: dir,
        paths,
        clock,
        prompt,
        access,
        events,
        _sub: sub,
    }
}

fn request() -> ConsentRequest {
    ConsentRequest {
        route: "agent/list-outputs",
        target: None,
        pid: Some(4242),
        comm: Some("claude".into()),
        remember: true,
    }
}

fn ask(rig: &Rig, consent: Consent, cx: &mut TestAppContext) -> Task<bool> {
    cx.update(|cx| AgentAccess::consent(&rig.access, consent, request(), cx))
}

fn grant(rig: &Rig, cx: &mut TestAppContext) -> Option<Grant> {
    cx.read(|cx| rig.access.read(cx).grant())
}

#[gpui::test]
async fn requests_waiting_on_one_prompt_share_its_answer_and_it_is_asked_once(
    cx: &mut TestAppContext,
) {
    let rig = rig(cx);
    let first = ask(&rig, Consent::OncePerSession, cx);
    let second = ask(&rig, Consent::OncePerSession, cx);
    cx.run_until_parked();
    assert_eq!(rig.prompt.asked.borrow().len(), 1, "one prompt for both");
    assert!(rig.prompt.asked.borrow()[0].0.remember);
    rig.prompt.answer_all(Answer::Allow(GrantSpan::Session));
    assert!(first.await && second.await);
    assert_eq!(grant(&rig, cx), Some(Grant::Session));
    assert!(ask(&rig, Consent::OncePerSession, cx).await);
    assert!(
        rig.prompt.asked.borrow().is_empty(),
        "a grant is not asked again"
    );
}

#[gpui::test]
async fn deny_refuses_and_grants_nothing(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let asked = ask(&rig, Consent::OncePerSession, cx);
    cx.run_until_parked();
    rig.prompt.answer_all(Answer::Deny);
    assert!(!asked.await);
    assert_eq!(grant(&rig, cx), None);
}

#[gpui::test]
async fn without_a_prompt_every_request_is_denied(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().expect("test setup");
    let file = es_store::agent::open(&Paths::rooted(dir.path()));
    let clock = FakeClock::new();
    let access = cx.new(|_| AgentAccess::new(file, clock.clock()));
    let asked =
        cx.update(|cx| AgentAccess::consent(&access, Consent::OncePerSession, request(), cx));
    assert!(!asked.await);
}

#[gpui::test]
async fn an_hours_grant_ends_after_an_hour_and_says_so(cx: &mut TestAppContext) {
    let rig = rig(cx);
    rig.access.update(cx, |a, cx| a.allow(GrantSpan::Hour, cx));
    let until = OffsetDateTime::UNIX_EPOCH + HOUR * 25;
    assert_eq!(grant(&rig, cx), Some(Grant::Until(until)));
    rig.clock.advance(HOUR - time::Duration::SECOND);
    assert!(grant(&rig, cx).is_some());
    rig.clock.advance(time::Duration::SECOND);
    assert_eq!(
        grant(&rig, cx),
        None,
        "the grant is checked against the clock"
    );
    rig.events.borrow_mut().clear();
    cx.executor()
        .advance_clock(std::time::Duration::from_secs(3600));
    cx.run_until_parked();
    assert_eq!(
        *rig.events.borrow(),
        [AccessEvent::GrantChanged(None)],
        "the tray learns the grant ended"
    );
    let asked = ask(&rig, Consent::OncePerSession, cx);
    cx.run_until_parked();
    assert_eq!(
        rig.prompt.asked.borrow().len(),
        1,
        "asked again once it ended"
    );
    rig.prompt.answer_all(Answer::Deny);
    assert!(!asked.await);
}

#[gpui::test]
fn revoke_ends_every_grant_and_the_lasting_one_on_disk(cx: &mut TestAppContext) {
    let rig = rig(cx);
    rig.access.update(cx, |a, cx| {
        a.allow(GrantSpan::Session, cx);
        a.allow(GrantSpan::Always, cx);
    });
    assert!(matches!(grant(&rig, cx), Some(Grant::Always(_))));
    rig.events.borrow_mut().clear();
    rig.access.update(cx, |a, cx| a.revoke(cx));
    assert_eq!(grant(&rig, cx), None);
    assert_eq!(*rig.events.borrow(), [AccessEvent::GrantChanged(None)]);
    let reopened = open(&rig.paths, &rig.clock, &rig.prompt, cx);
    assert_eq!(cx.read(|cx| reopened.read(cx).grant()), None);
}

#[gpui::test]
fn always_outlives_the_daemon_and_an_hour_or_session_does_not(cx: &mut TestAppContext) {
    let rig = rig(cx);
    rig.access
        .update(cx, |a, cx| a.allow(GrantSpan::Session, cx));
    let restarted = open(&rig.paths, &rig.clock, &rig.prompt, cx);
    assert_eq!(cx.read(|cx| restarted.read(cx).grant()), None);

    rig.access
        .update(cx, |a, cx| a.allow(GrantSpan::Always, cx));
    let restarted = open(&rig.paths, &rig.clock, &rig.prompt, cx);
    assert_eq!(
        cx.read(|cx| restarted.read(cx).grant()),
        Some(Grant::Always(OffsetDateTime::UNIX_EPOCH + HOUR * 24))
    );
}

#[gpui::test]
async fn always_ask_asks_every_time_and_remembers_nothing(cx: &mut TestAppContext) {
    let rig = rig(cx);
    rig.access
        .update(cx, |a, cx| a.allow(GrantSpan::Session, cx));
    for _ in 0..2 {
        let asked = ask(&rig, Consent::AlwaysAsk, cx);
        cx.run_until_parked();
        assert_eq!(rig.prompt.asked.borrow().len(), 1);
        assert!(!rig.prompt.asked.borrow()[0].0.remember);
        rig.prompt.answer_all(Answer::Once);
        assert!(asked.await);
    }
    assert!(ask(&rig, Consent::AlwaysAllow, cx).await);
    assert!(rig.prompt.asked.borrow().is_empty());
}

#[gpui::test]
fn the_log_is_bounded_and_written_through(cx: &mut TestAppContext) {
    let rig = rig(cx);
    rig.access.update(cx, |a, cx| {
        for n in 0..4 {
            a.record(
                es_model::ActionRecord {
                    at: OffsetDateTime::UNIX_EPOCH,
                    route: "agent/list-outputs".into(),
                    target: None,
                    result: "ok".into(),
                    pid: Some(n),
                    comm: None,
                },
                3,
                cx,
            );
        }
    });
    let reopened = es_store::agent::open(&rig.paths);
    let pids: Vec<_> = reopened.value.log.iter().map(|e| e.pid).collect();
    assert_eq!(pids, [Some(3), Some(2), Some(1)]);
    assert!(rig.events.borrow().contains(&AccessEvent::Logged));
}

#[test]
fn a_change_loosens_access_only_when_it_gives_agents_more() {
    use es_app::agent_access::Loosening;
    use es_model::AgentCfg;
    let base = AgentCfg {
        enabled: true,
        allowed_outputs: vec!["DP-1".into()],
        ..AgentCfg::default()
    };
    let with = |f: fn(&mut AgentCfg)| {
        let mut c = base.clone();
        f(&mut c);
        c
    };
    let off = with(|c| c.enabled = false);
    assert_eq!(Loosening::between(&off, &base), Some(Loosening::TurnOn));
    assert_eq!(Loosening::between(&base, &off), None);
    let open = with(|c| c.consent = Consent::AlwaysAllow);
    assert_eq!(
        Loosening::between(&base, &open),
        Some(Loosening::AlwaysAllow)
    );
    assert_eq!(Loosening::between(&open, &base), None);
    let every = with(|c| c.allowed_outputs.clear());
    assert_eq!(
        Loosening::between(&base, &every),
        Some(Loosening::WiderScope)
    );
    assert_eq!(
        Loosening::between(&every, &base),
        None,
        "naming outputs narrows"
    );
    let more = with(|c| c.allowed_outputs.push("DP-2".into()));
    assert_eq!(
        Loosening::between(&base, &more),
        Some(Loosening::WiderScope)
    );
    assert_eq!(Loosening::between(&more, &base), None);
    assert_eq!(Loosening::between(&base, &base), None);
}

#[test]
fn hiding_secrets_less_or_the_log_shorter_is_the_users_call() {
    use es_app::agent_access::Loosening;
    use es_model::AgentCfg;
    let base = AgentCfg {
        redact_secrets: true,
        log_limit: 200,
        keep_captures: true,
        ..AgentCfg::default()
    };
    let with = |f: fn(&mut AgentCfg)| {
        let mut c = base.clone();
        f(&mut c);
        c
    };
    let plain = with(|c| c.redact_secrets = false);
    assert_eq!(
        Loosening::between(&base, &plain),
        Some(Loosening::ShowSecrets)
    );
    assert_eq!(Loosening::between(&plain, &base), None);
    let short = with(|c| c.log_limit = 10);
    assert_eq!(
        Loosening::between(&base, &short),
        Some(Loosening::ShorterLog)
    );
    assert_eq!(Loosening::between(&short, &base), None);
    let long = with(|c| c.log_limit = 500);
    assert_eq!(Loosening::between(&base, &long), None);
    let drop = with(|c| c.keep_captures = false);
    assert_eq!(Loosening::between(&base, &drop), None);
    assert_eq!(Loosening::between(&drop, &base), None);
}
