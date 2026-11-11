use super::widgets::{FIELD_WIDTH, caption, group, note_row, row, status_mark, status_row};
use super::{SettingsWindow, SettingsWindowEvent};
use crate::strings::fl;
use es_app::share::{KeyPlace, ShareFailure, Shares};
use es_model::config::SHARE_DAYS;
use es_model::{BucketKeys, ShareCfg, ShareMode};
use es_share::BucketFault;
use es_store::paths::Paths;
use es_ui::components::{ButtonKind, EditorButton};
use es_ui::theme::tokens;
use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, IntoElement, ParentElement, SharedString,
    Styled, Subscription, Task, Window, div, px,
};
use gpui_component::{
    h_flex,
    input::{Input, InputEvent, InputState},
};
use std::path::PathBuf;

pub use es_app::share::TERMS_URL;
const FIELD_COUNT: usize = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SavedKeys {
    Unknown,
    Checking,
    Missing,
    Saved(KeyPlace),
    Replacing(KeyPlace),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionTest {
    Idle,
    Running,
    Done(Result<(), ShareFailure>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Problem {
    KeysIncomplete,
    Save(ShareFailure),
    Remove(ShareFailure),
}

pub(super) struct Sharing {
    shares: Option<Entity<Shares>>,
    key_folder: String,
    fields: [Entity<InputState>; FIELD_COUNT],
    key_id: Entity<InputState>,
    secret: Entity<InputState>,
    keys: SavedKeys,
    test: ConnectionTest,
    problem: Option<Problem>,
    _subscriptions: Vec<Subscription>,
}

fn field(c: &mut ShareCfg, i: usize) -> &mut String {
    match i {
        0 => &mut c.bucket_endpoint,
        1 => &mut c.bucket_name,
        2 => &mut c.bucket_region,
        3 => &mut c.bucket_public_base_url,
        _ => &mut c.bucket_prefix,
    }
}

fn field_title(i: usize) -> String {
    match i {
        0 => fl!("sharing-endpoint"),
        1 => fl!("sharing-bucket"),
        2 => fl!("sharing-region"),
        3 => fl!("sharing-public-url"),
        _ => fl!("sharing-prefix"),
    }
}

fn field_prompt(i: usize) -> String {
    match i {
        0 => fl!("sharing-endpoint-prompt"),
        1 => fl!("sharing-bucket-prompt"),
        2 => fl!("sharing-region-prompt"),
        3 => fl!("sharing-public-url-prompt"),
        _ => fl!("sharing-prefix-prompt"),
    }
}

fn place_text(place: KeyPlace, folder: &str) -> String {
    match place {
        KeyPlace::Keyring => fl!("sharing-keys-in-keyring"),
        KeyPlace::File => fl!("sharing-keys-in-file", folder = folder),
    }
}

/// The folder that holds the key file, with the home directory written as `~`.
pub fn key_folder(paths: &Paths) -> String {
    let file = paths.bucket_keys_file();
    let folder = file.parent().unwrap_or(&file);
    let home = std::env::var_os("HOME").map(PathBuf::from);
    match home.as_deref().and_then(|h| folder.strip_prefix(h).ok()) {
        Some(rest) if rest.as_os_str().is_empty() => "~".to_owned(),
        Some(rest) => format!("~/{}", rest.display()),
        None => folder.display().to_string(),
    }
}

fn test_text(result: Result<(), ShareFailure>) -> String {
    match result {
        Ok(()) => fl!("sharing-test-works"),
        Err(ShareFailure::Bucket(BucketFault::Credentials)) => fl!("sharing-test-credentials"),
        Err(ShareFailure::Bucket(BucketFault::Bucket)) => fl!("sharing-test-bucket"),
        Err(ShareFailure::Bucket(BucketFault::PublicUrl)) => fl!("sharing-test-public-url"),
        Err(ShareFailure::Bucket(BucketFault::Network) | ShareFailure::Network) => {
            fl!("sharing-test-network")
        }
        Err(ShareFailure::NotConfigured) => fl!("sharing-test-not-configured"),
        Err(ShareFailure::Keyring) => fl!("sharing-test-keyring"),
        Err(_) => fl!("sharing-test-failed"),
    }
}

fn problem_text(problem: Problem) -> String {
    match problem {
        Problem::KeysIncomplete => fl!("sharing-keys-incomplete"),
        Problem::Save(ShareFailure::Keyring) => fl!("sharing-keys-keyring-failed"),
        Problem::Save(_) => fl!("sharing-keys-not-saved"),
        Problem::Remove(_) => fl!("sharing-keys-not-removed"),
    }
}

fn input(
    window: &mut Window,
    cx: &mut Context<SettingsWindow>,
    prompt: Option<String>,
    value: &str,
) -> Entity<InputState> {
    let value = value.to_owned();
    cx.new(|cx| {
        let state = InputState::new(window, cx).default_value(value);
        match prompt {
            Some(prompt) => state.placeholder(prompt),
            None => state,
        }
    })
}

impl Sharing {
    pub(super) fn new(
        cfg: &ShareCfg,
        shares: Option<Entity<Shares>>,
        key_folder: String,
        window: &mut Window,
        cx: &mut Context<SettingsWindow>,
    ) -> Self {
        let mut c = cfg.clone();
        let fields = std::array::from_fn(|i| {
            let value = field(&mut c, i).clone();
            input(window, cx, Some(field_prompt(i)), &value)
        });
        let key_id = input(window, cx, None, "");
        let secret = cx.new(|cx| InputState::new(window, cx).masked(true));
        let subscriptions = fields
            .iter()
            .chain([&key_id, &secret])
            .map(|f| {
                cx.subscribe(f, |_, _, event: &InputEvent, cx| {
                    if let InputEvent::Change = event {
                        cx.notify();
                    }
                })
            })
            .collect();
        Self {
            shares,
            key_folder,
            fields,
            key_id,
            secret,
            keys: SavedKeys::Unknown,
            test: ConnectionTest::Idle,
            problem: None,
            _subscriptions: subscriptions,
        }
    }
}

impl SettingsWindow {
    /// The bucket fields in page order, then the access key ID and the secret.
    pub fn sharing_inputs(&self) -> Vec<Entity<InputState>> {
        let s = &self.sharing;
        let mut all = s.fields.to_vec();
        all.extend([s.key_id.clone(), s.secret.clone()]);
        all
    }

    pub fn saved_keys(&self) -> SavedKeys {
        self.sharing.keys
    }

    pub fn connection_test(&self) -> ConnectionTest {
        self.sharing.test
    }

    /// What the page says about the keys and the last save, test or removal.
    pub fn sharing_status(&self) -> Vec<String> {
        let s = &self.sharing;
        let mut out = Vec::new();
        if let SavedKeys::Saved(place) | SavedKeys::Replacing(place) = s.keys {
            out.push(place_text(place, &s.key_folder));
        }
        if let Some(problem) = s.problem {
            out.push(problem_text(problem));
        }
        if let ConnectionTest::Done(result) = s.test {
            out.push(test_text(result));
        }
        out
    }

    /// Choosing here counts as agreeing, as the first-share step does.
    pub fn set_share_target(&mut self, mode: ShareMode, cx: &mut Context<Self>) {
        if let Some(shares) = &self.sharing.shares {
            shares.update(cx, |s, cx| s.consent(mode, cx));
        }
        if mode == ShareMode::Bucket {
            self.check_saved_keys(cx);
        }
    }

    pub fn set_share_days(&mut self, days: u8, cx: &mut Context<Self>) {
        if let Some(shares) = &self.sharing.shares {
            shares.update(cx, |s, cx| s.set_days(days, cx));
        }
    }

    /// The keyring is asked only once the bucket section shows, so Free users never meet an unlock prompt.
    pub(super) fn check_saved_keys(&mut self, cx: &mut Context<Self>) {
        let bucket = self.config(cx).share.target == ShareMode::Bucket;
        let Some(shares) = self.sharing.shares.clone().filter(|_| bucket) else {
            return;
        };
        if self.sharing.keys != SavedKeys::Unknown {
            return;
        }
        self.sharing.keys = SavedKeys::Checking;
        let task = shares.read(cx).saved_keys(cx);
        self.when_done(task, cx, |this, result| {
            this.sharing.keys = match result {
                Ok(Some(place)) => SavedKeys::Saved(place),
                _ => SavedKeys::Missing,
            };
        });
    }

    /// Writes the bucket fields to config, and the keys when both are filled in.
    pub fn save_bucket(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let values: Vec<String> = self
            .sharing
            .fields
            .iter()
            .map(|f| f.read(cx).value().trim().to_owned())
            .collect();
        self.edit(
            move |c| {
                for (i, v) in values.into_iter().enumerate() {
                    *field(&mut c.share, i) = v;
                }
            },
            cx,
        );
        self.sharing.problem = None;
        self.sharing.test = ConnectionTest::Idle;
        let id = self.sharing.key_id.read(cx).value().trim().to_owned();
        let secret = self.sharing.secret.read(cx).value().trim().to_owned();
        match (id.is_empty(), secret.is_empty()) {
            (true, true) => {
                if let SavedKeys::Replacing(place) = self.sharing.keys {
                    self.sharing.keys = SavedKeys::Saved(place);
                }
            }
            (false, false) => {
                for input in [&self.sharing.key_id, &self.sharing.secret] {
                    input.update(cx, |s, cx| s.set_value("", window, cx));
                }
                if let Some(shares) = &self.sharing.shares {
                    let keys = BucketKeys {
                        access_key_id: id,
                        secret_access_key: secret,
                    };
                    let task = shares.read(cx).save_keys(keys, cx);
                    self.when_done(task, cx, |this, result| match result {
                        Ok(place) => this.sharing.keys = SavedKeys::Saved(place),
                        Err(e) => this.sharing.problem = Some(Problem::Save(e)),
                    });
                }
            }
            _ => self.sharing.problem = Some(Problem::KeysIncomplete),
        }
        cx.notify();
    }

    pub fn test_connection(&mut self, cx: &mut Context<Self>) {
        let Some(shares) = &self.sharing.shares else {
            return;
        };
        self.sharing.test = ConnectionTest::Running;
        let task = shares.read(cx).test_bucket(cx);
        self.when_done(task, cx, |this, result| {
            this.sharing.test = ConnectionTest::Done(result)
        });
        cx.notify();
    }

    pub fn remove_keys(&mut self, cx: &mut Context<Self>) {
        let Some(shares) = &self.sharing.shares else {
            return;
        };
        self.sharing.problem = None;
        self.sharing.test = ConnectionTest::Idle;
        let task = shares.read(cx).remove_keys(cx);
        self.when_done(task, cx, |this, result| match result {
            Ok(()) => this.sharing.keys = SavedKeys::Missing,
            Err(e) => this.sharing.problem = Some(Problem::Remove(e)),
        });
    }

    fn when_done<T: 'static>(
        &mut self,
        task: Task<T>,
        cx: &mut Context<Self>,
        done: impl FnOnce(&mut Self, T) + 'static,
    ) {
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                done(this, result);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn bucket_dirty(&self, cfg: &ShareCfg, cx: &Context<Self>) -> bool {
        let mut c = cfg.clone();
        let s = &self.sharing;
        s.fields
            .iter()
            .enumerate()
            .any(|(i, f)| f.read(cx).value().trim() != field(&mut c, i).as_str())
            || [&s.key_id, &s.secret]
                .iter()
                .any(|f| !f.read(cx).value().trim().is_empty())
    }

    /// A bucket's own rules decide how long its shares stay.
    pub fn shows_share_days(&self, cx: &App) -> bool {
        self.config(cx).share.target != ShareMode::Bucket
    }

    pub(super) fn render_sharing(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let c = self.config(cx);
        let share = &c.share;
        let bucket = !self.shows_share_days(cx);
        let help = match (share.consented, bucket) {
            (false, _) => fl!("sharing-target-unchosen-help"),
            (true, false) => fl!("sharing-target-free-help"),
            (true, true) => fl!("sharing-target-bucket-help"),
        };
        let mut rows = vec![self.pick_row(
            "share-target",
            fl!("sharing-target"),
            Some(help),
            vec![
                (ShareMode::Free, fl!("sharing-target-free")),
                (ShareMode::Bucket, fl!("sharing-target-bucket")),
            ],
            &share.target,
            |this, mode, cx| this.set_share_target(mode, cx),
            cx,
        )];
        if !bucket {
            let days = SHARE_DAYS
                .iter()
                .map(|&d| {
                    let label = match d {
                        1 => fl!("sharing-days-1"),
                        3 => fl!("sharing-days-3"),
                        _ => fl!("sharing-days-7"),
                    };
                    (d, label)
                })
                .collect();
            rows.push(self.pick_row(
                "share-days",
                fl!("sharing-days"),
                None,
                days,
                &share.days(),
                |this, days, cx| this.set_share_days(days, cx),
                cx,
            ));
        }
        let mut out =
            vec![group(Some(fl!("sharing-where").into()), rows, None, cx).into_any_element()];
        if bucket {
            out.push(self.render_bucket(share, cx));
        }
        let terms = EditorButton::new("sharing-terms")
            .label(fl!("sharing-terms"))
            .accessible_label(fl!("sharing-terms-a11y"))
            .kind(ButtonKind::Bordered)
            .on_click(cx.listener(|_, _, _, cx| cx.emit(SettingsWindowEvent::OpenUrl(TERMS_URL))));
        out.push(
            group(
                None,
                vec![note_row(fl!("sharing-unlisted"), terms, cx)],
                None,
                cx,
            )
            .into_any_element(),
        );
        out
    }

    fn render_bucket(&mut self, share: &ShareCfg, cx: &mut Context<Self>) -> AnyElement {
        let s = &self.sharing;
        let ready = s.shares.is_some();
        let text_field = |input: &Entity<InputState>, label: String| {
            Input::new(input).aria_label(label).w(px(FIELD_WIDTH))
        };
        let mut rows: Vec<AnyElement> = s
            .fields
            .iter()
            .enumerate()
            .map(|(i, f)| row(field_title(i), None, text_field(f, field_title(i)), cx))
            .collect();
        match s.keys {
            SavedKeys::Saved(place) => {
                let controls = h_flex()
                    .gap(px(8.))
                    .child(
                        EditorButton::new("sharing-keys-remove")
                            .label(fl!("sharing-keys-remove"))
                            .kind(ButtonKind::Bordered)
                            .on_click(cx.listener(|this, _, _, cx| this.remove_keys(cx))),
                    )
                    .child(
                        EditorButton::new("sharing-keys-replace")
                            .label(fl!("sharing-keys-replace"))
                            .kind(ButtonKind::Bordered)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.sharing.keys = SavedKeys::Replacing(place);
                                cx.notify();
                            })),
                    );
                rows.push(row(
                    fl!("sharing-keys"),
                    Some(place_text(place, &s.key_folder).into()),
                    controls,
                    cx,
                ));
            }
            SavedKeys::Unknown | SavedKeys::Checking => {
                rows.push(row(
                    fl!("sharing-keys"),
                    Some(fl!("sharing-keys-checking").into()),
                    div(),
                    cx,
                ));
            }
            SavedKeys::Missing | SavedKeys::Replacing(_) => {
                for (input, title) in [
                    (&s.key_id, fl!("sharing-access-key-id")),
                    (&s.secret, fl!("sharing-secret-key")),
                ] {
                    rows.push(row(title.clone(), None, text_field(input, title), cx));
                }
            }
        }
        let t = tokens(cx);
        let status: AnyElement = match (s.problem, s.test) {
            (Some(problem), _) => caption(problem_text(problem), cx)
                .text_color(t.destructive)
                .into_any_element(),
            (None, ConnectionTest::Done(Ok(()))) => {
                status_mark(true, test_text(Ok(())), cx).into_any_element()
            }
            (None, ConnectionTest::Done(result)) => caption(test_text(result), cx)
                .text_color(t.destructive)
                .into_any_element(),
            (None, ConnectionTest::Running) => {
                caption(fl!("sharing-test-running"), cx).into_any_element()
            }
            (None, ConnectionTest::Idle) => {
                caption(fl!("sharing-test-help"), cx).into_any_element()
            }
        };
        let dirty = self.bucket_dirty(share, cx);
        let can_test = ready
            && !dirty
            && matches!(s.keys, SavedKeys::Saved(_))
            && s.test != ConnectionTest::Running;
        let actions = h_flex()
            .gap(px(8.))
            .child(
                EditorButton::new("sharing-test")
                    .label(fl!("sharing-test"))
                    .kind(ButtonKind::Bordered)
                    .disabled(!can_test)
                    .on_click(cx.listener(|this, _, _, cx| this.test_connection(cx))),
            )
            .child(
                EditorButton::new("sharing-save")
                    .label(fl!("sharing-save"))
                    .kind(ButtonKind::Prominent)
                    .disabled(!ready || !dirty)
                    .on_click(cx.listener(|this, _, window, cx| this.save_bucket(window, cx))),
            );
        rows.push(status_row(status, actions));
        group(
            Some(SharedString::from(fl!("sharing-bucket-group"))),
            rows,
            Some(fl!("sharing-keys-footer", folder = s.key_folder.clone()).into()),
            cx,
        )
        .into_any_element()
    }
}
