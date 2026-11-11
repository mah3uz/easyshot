#![allow(clippy::unwrap_used, clippy::expect_used)]

use es_app::history::History;
use es_app::platform_ui::ClipboardSink as ItemClipboard;
use es_app::settings_model::SettingsModel;
use es_app::share::{Caps, KeyPlace, Keyring, LiveTargets, ShareFailure, Shares, Target, Targets};
use es_backend::Caps as BackendCaps;
use es_desktop::SecretsError;
use es_model::platform::ClipItem;
use es_model::recording::InputAccess;
use es_model::{BucketKeys, ShareCfg, ShareMode};
use es_share::BucketFault;
use es_shell::settings::{
    ClipboardSink, ConnectionTest, Page, SavedKeys, SettingsDeps, SettingsWindow,
};
use es_store::{ConfigFile, Paths};
use es_testkit::Headless;
use es_ui::{assets::Assets, theme, tokens::ThemeMode};
use gpui::{AppContext, Entity, Task};
use gpui_component::Root;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

const SECRET: &str = "wJalrXUtnFEMI-never-shown";

struct NoClipboard;

impl ClipboardSink for NoClipboard {
    fn copy(&self, _: String, _: &mut gpui::App) {}
}

impl ItemClipboard for NoClipboard {
    fn copy(&self, _: Vec<ClipItem>, _: &mut gpui::App) -> Task<es_app::errors::Result<()>> {
        Task::ready(Ok(()))
    }
}

/// `None` answers as a machine with no Secret Service running.
struct Ring(Option<Mutex<Option<BucketKeys>>>);

impl Keyring for Ring {
    fn load(&self) -> Result<Option<BucketKeys>, SecretsError> {
        let held = self.0.as_ref().ok_or(SecretsError::Unavailable)?;
        Ok(held.lock().unwrap().clone())
    }

    fn store(&self, keys: &BucketKeys) -> Result<(), SecretsError> {
        let held = self.0.as_ref().ok_or(SecretsError::Unavailable)?;
        *held.lock().unwrap() = Some(keys.clone());
        Ok(())
    }

    fn clear(&self) -> Result<(), SecretsError> {
        let held = self.0.as_ref().ok_or(SecretsError::Unavailable)?;
        *held.lock().unwrap() = None;
        Ok(())
    }
}

/// The real key handling, with the bucket's answer to a connection test scripted.
struct Scripted {
    live: LiveTargets,
    answer: Mutex<Result<(), ShareFailure>>,
}

impl Targets for Scripted {
    fn open(&self, mode: ShareMode, cfg: &ShareCfg) -> Result<Box<dyn Target>, ShareFailure> {
        self.live.open(mode, cfg)
    }

    fn caps(&self, mode: ShareMode, cfg: &ShareCfg) -> Result<Caps, ShareFailure> {
        self.live.caps(mode, cfg)
    }

    fn test_bucket(&self, _: &ShareCfg) -> Result<(), ShareFailure> {
        *self.answer.lock().unwrap()
    }

    fn saved_keys(&self) -> Option<KeyPlace> {
        self.live.saved_keys()
    }

    fn save_keys(&self, keys: &BucketKeys) -> Result<KeyPlace, ShareFailure> {
        self.live.save_keys(keys)
    }

    fn remove_keys(&self) -> Result<(), ShareFailure> {
        self.live.remove_keys()
    }
}

struct Fixture {
    app: Headless<Root>,
    view: Entity<SettingsWindow>,
    settings: Entity<SettingsModel>,
    targets: Arc<Scripted>,
    ring: Arc<Ring>,
    paths: Paths,
    _dir: tempfile::TempDir,
}

fn fixture(ring: Ring, prepare: impl FnOnce(&Paths)) -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::rooted(dir.path());
    paths.ensure_all().unwrap();
    prepare(&paths);
    let file = ConfigFile::load(&paths.config_file()).unwrap();
    let ring = Arc::new(ring);
    let targets = Arc::new(Scripted {
        live: LiveTargets::new(paths.clone(), "1.0.0", ring.clone()),
        answer: Mutex::new(Ok(())),
    });
    let (mut view, mut model) = (None, None);
    let (w, h) = es_shell::settings::WINDOW_SIZE;
    let (deps_paths, deps_targets) = (paths.clone(), targets.clone());
    let app = Headless::open(Assets, es_ui::init, w, h * 1.4, |window, cx| {
        let settings = cx.new(|_| SettingsModel::new(file));
        model = Some(settings.clone());
        let history = cx.new(|_| History::open(&deps_paths, 10).unwrap());
        let (s, p) = (settings.clone(), deps_paths.clone());
        let shares =
            cx.new(|cx| Shares::new(&p, s, history, Rc::new(NoClipboard), deps_targets, cx));
        let deps = SettingsDeps {
            settings,
            history: None,
            caps: BackendCaps::empty(),
            binds: None,
            paths: deps_paths,
            clipboard: Rc::new(NoClipboard),
            outputs: Vec::new(),
            version: "v9.9.9".to_owned(),
            input_access: Rc::new(|| InputAccess::Granted),
            shares: Some(shares),
        };
        let v = cx.new(|cx| SettingsWindow::new(deps, window, cx));
        view = Some(v.clone());
        cx.new(|cx| Root::new(v, window, cx))
    });
    let mut fx = Fixture {
        app,
        view: view.unwrap(),
        settings: model.unwrap(),
        targets,
        ring,
        paths,
        _dir: dir,
    };
    fx.app
        .cx
        .update(|cx| es_ui::motion::set_reduce_motion(true, cx));
    fx.update(|v, cx| v.show(Page::Sharing, cx));
    fx
}

impl Fixture {
    fn update(&mut self, f: impl FnOnce(&mut SettingsWindow, &mut gpui::Context<SettingsWindow>)) {
        let view = self.view.clone();
        self.app.cx.update(|cx| view.update(cx, f));
        self.app.settle();
    }

    fn read<R>(&mut self, f: impl FnOnce(&SettingsWindow, &gpui::App) -> R) -> R {
        let view = self.view.clone();
        self.app.cx.update(|cx| f(view.read(cx), cx))
    }

    fn share(&mut self) -> ShareCfg {
        let settings = self.settings.clone();
        self.app
            .cx
            .update(|cx| settings.read(cx).config().share.clone())
    }

    fn until_keys(&mut self, want: SavedKeys) {
        let view = self.view.clone();
        self.app.until(&format!("keys {want:?}"), |cx| {
            view.read(cx).saved_keys() == want
        });
    }

    /// Endpoint, bucket, region, public URL, prefix, access key ID, secret.
    fn fill(&mut self, values: [&str; 7]) {
        let inputs = self.read(|v, _| v.sharing_inputs());
        let handle = self.app.window.into();
        self.app
            .cx
            .update_window(handle, |_, window, cx| {
                for (input, value) in inputs.iter().zip(values) {
                    input.update(cx, |s, cx| s.set_value(value, window, cx));
                }
            })
            .unwrap();
        self.app.settle();
    }

    fn save(&mut self) {
        let view = self.view.clone();
        let handle = self.app.window.into();
        self.app
            .cx
            .update_window(handle, |_, window, cx| {
                view.update(cx, |v, cx| v.save_bucket(window, cx))
            })
            .unwrap();
        self.app.settle();
    }

    fn shots(&mut self, name: &str) {
        for (mode, suffix) in [(ThemeMode::Light, "light"), (ThemeMode::Dark, "dark")] {
            let view = self.view.clone();
            self.app.cx.update(|cx| {
                theme::set_mode(mode, cx);
                view.update(cx, |_, cx| cx.notify());
            });
            self.app
                .screenshot(&format!("settings-sharing-{name}-{suffix}"));
        }
    }
}

fn keys() -> BucketKeys {
    BucketKeys {
        access_key_id: "AKIAEXAMPLE".into(),
        secret_access_key: SECRET.into(),
    }
}

fn text(id: &str) -> String {
    es_shell::strings::loader().get(id)
}

const BUCKET: [&str; 7] = [
    "https://abc.r2.cloudflarestorage.com",
    "shots",
    "auto",
    "https://shots.example.com",
    "",
    "AKIAEXAMPLE",
    SECRET,
];

#[test]
fn the_page_says_which_destination_shares_use_and_the_days_show_only_for_free() {
    let mut fx = fixture(Ring(None), |_| {});
    assert_eq!(
        fx.read(|v, cx| v.share_destination_line(cx)),
        text("sharing-using-free"),
        "nothing configured shares to Free, with no choice to make"
    );
    assert!(fx.read(|v, cx| v.shows_share_days(cx)));
    fx.shots("free");
    fx.update(|v, cx| v.set_share_days(3, cx));
    assert_eq!(fx.share().days, 3);

    fx.fill(BUCKET);
    fx.save();
    fx.until_keys(SavedKeys::Saved(KeyPlace::File));
    let line = fx.read(|v, cx| v.share_destination_line(cx));
    assert!(
        line.starts_with("Shares use your bucket") && line.contains("shots"),
        "{line}"
    );
    assert!(
        !fx.read(|v, cx| v.shows_share_days(cx)),
        "a bucket keeps shares by its own rules, so the lifetime is not offered"
    );

    let mut cleared = BUCKET;
    cleared[0] = "";
    fx.fill(cleared);
    fx.save();
    assert_eq!(
        fx.read(|v, cx| v.share_destination_line(cx)),
        text("sharing-using-free"),
        "removing the bucket details returns to Free"
    );
    assert!(fx.read(|v, cx| v.shows_share_days(cx)));
    assert_eq!(fx.share().days, 3);
}

#[test]
fn keys_saved_without_a_keyring_go_to_the_key_file_and_the_secret_is_never_shown_again() {
    let mut fx = fixture(Ring(None), |_| {});
    fx.fill(BUCKET);
    fx.shots("bucket-filled");
    fx.save();
    fx.until_keys(SavedKeys::Saved(KeyPlace::File));

    let share = fx.share();
    assert_eq!(share.bucket_endpoint, BUCKET[0]);
    assert_eq!(share.bucket_name, "shots");
    assert_eq!(share.bucket_region, "auto");
    assert_eq!(share.bucket_public_base_url, BUCKET[3]);
    let config = std::fs::read_to_string(fx.paths.config_file()).unwrap_or_default();
    assert!(!config.contains(SECRET), "a key reached config.toml");
    assert_eq!(
        es_store::shares::read_bucket_keys(&fx.paths).unwrap(),
        Some(keys())
    );

    let status = fx.read(|v, _| v.sharing_status());
    let folder = fx
        .paths
        .bucket_keys_file()
        .parent()
        .unwrap()
        .display()
        .to_string();
    assert!(
        status.iter().any(|s| s.contains(&format!("in {folder},"))),
        "the page must name the folder the key file is really in: {status:?}"
    );
    let shown: Vec<String> = fx.read(|v, cx| {
        v.sharing_inputs()
            .iter()
            .map(|i| i.read(cx).value().to_string())
            .collect()
    });
    assert!(
        shown.iter().all(|s| !s.contains(SECRET)) && status.iter().all(|s| !s.contains(SECRET)),
        "the secret is still on the page after saving: {shown:?}"
    );
    fx.shots("bucket-saved");
}

#[test]
fn the_connection_test_names_the_step_that_failed() {
    let mut fx = fixture(Ring(None), |p| {
        es_store::shares::write_bucket_keys(p, keys()).unwrap();
    });
    fx.fill(BUCKET);
    fx.save();
    for (answer, id) in [
        (
            Err(ShareFailure::Bucket(BucketFault::Credentials)),
            "sharing-test-credentials",
        ),
        (
            Err(ShareFailure::Bucket(BucketFault::Bucket)),
            "sharing-test-bucket",
        ),
        (
            Err(ShareFailure::Bucket(BucketFault::PublicUrl)),
            "sharing-test-public-url",
        ),
        (
            Err(ShareFailure::Bucket(BucketFault::Network)),
            "sharing-test-network",
        ),
        (Err(ShareFailure::Network), "sharing-test-network"),
        (Ok(()), "sharing-test-works"),
    ] {
        *fx.targets.answer.lock().unwrap() = answer;
        fx.update(|v, cx| v.test_connection(cx));
        let view = fx.view.clone();
        fx.app.until("the test", |cx| {
            matches!(view.read(cx).connection_test(), ConnectionTest::Done(_))
        });
        assert_eq!(
            fx.read(|v, _| v.connection_test()),
            ConnectionTest::Done(answer)
        );
        let status = fx.read(|v, _| v.sharing_status());
        assert!(status.contains(&text(id)), "{answer:?}: {status:?}");
        if id == "sharing-test-public-url" {
            fx.shots("test-failed");
        }
    }
    fx.shots("test-works");
}

#[test]
fn remove_keys_clears_the_keyring_and_the_key_file() {
    let ring = Ring(Some(Mutex::new(Some(keys()))));
    let mut fx = fixture(ring, |p| {
        es_store::shares::write_bucket_keys(p, keys()).unwrap();
        std::fs::write(
            p.config_file(),
            "[share]\nbucket_endpoint = \"https://s3.example\"\nbucket_name = \"shots\"\nbucket_public_base_url = \"https://cdn.example\"\n",
        )
        .unwrap();
    });
    fx.until_keys(SavedKeys::Saved(KeyPlace::Keyring));
    assert!(
        fx.read(|v, _| v.sharing_status())
            .contains(&text("sharing-keys-in-keyring"))
    );
    fx.update(|v, cx| v.remove_keys(cx));
    fx.until_keys(SavedKeys::Missing);
    assert_eq!(fx.ring.load().unwrap(), None);
    assert_eq!(es_store::shares::read_bucket_keys(&fx.paths).unwrap(), None);
}

#[test]
fn the_key_folder_is_written_from_home_as_a_tilde() {
    let home = std::path::PathBuf::from(std::env::var_os("HOME").unwrap());
    let under_home = Paths::rooted(&home.join("xdg"));
    assert_eq!(es_shell::settings::key_folder(&under_home), "~/xdg/data");
    let elsewhere = Paths::rooted(std::path::Path::new("/srv/es"));
    assert_eq!(es_shell::settings::key_folder(&elsewhere), "/srv/es/data");
}
