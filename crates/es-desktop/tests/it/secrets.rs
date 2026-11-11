use crate::dbus_support::PrivateBus;
use es_desktop::{SecretsError, available, clear_bucket_keys, load_bucket_keys, store_bucket_keys};
use es_model::BucketKeys;
use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const MODE: &str = "ES_SECRETS_CHILD";

fn keys() -> BucketKeys {
    BucketKeys {
        access_key_id: "AKIA-test".into(),
        secret_access_key: "s3cr3t-value".into(),
    }
}

/// The oo7 client reads the session bus from the environment, so each scenario runs in a
/// re-executed copy of this test binary whose environment points at a private bus.
fn in_child(mode: &str, bus: &PrivateBus, home: Option<&std::path::Path>) {
    let mut cmd = Command::new(std::env::current_exe().expect("test binary"));
    cmd.args(["--exact", "secrets::child", "--nocapture"])
        .env(MODE, mode)
        .env("DBUS_SESSION_BUS_ADDRESS", &bus.address)
        .env_remove("XDG_RUNTIME_DIR");
    if let Some(home) = home {
        cmd.env("HOME", home);
    }
    let out = cmd.output().expect("child runs");
    assert!(
        out.status.success(),
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn no_secret_service_is_unavailable_and_fast() {
    let bus = PrivateBus::start();
    let started = Instant::now();
    in_child("unavailable", &bus, None);
    assert!(started.elapsed() < Duration::from_secs(20));
}

#[test]
fn keys_round_trip_through_a_keyring() {
    if Command::new("gnome-keyring-daemon")
        .arg("--version")
        .stdout(Stdio::null())
        .status()
        .is_err()
    {
        #[allow(clippy::print_stdout)]
        {
            println!("skipped: gnome-keyring-daemon is not installed");
        }
        return;
    }
    let bus = PrivateBus::start();
    let home = tempfile::tempdir().expect("home");
    let mut daemon = Command::new("gnome-keyring-daemon")
        .args(["--foreground", "--components=secrets", "--unlock"])
        .env("DBUS_SESSION_BUS_ADDRESS", &bus.address)
        .env("HOME", home.path())
        .env("XDG_DATA_HOME", home.path().join("data"))
        .env_remove("XDG_RUNTIME_DIR")
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("daemon starts");
    daemon
        .stdin
        .take()
        .expect("piped")
        .write_all(b"test\n")
        .expect("password");
    in_child("roundtrip", &bus, Some(home.path()));
    let _ = daemon.kill();
    let _ = daemon.wait();
}

#[test]
fn child() {
    let Ok(mode) = std::env::var(MODE) else {
        return;
    };
    crate::dbus_support::run(async move {
        if mode == "unavailable" {
            assert!(!available().await);
            assert!(matches!(
                load_bucket_keys().await,
                Err(SecretsError::Unavailable)
            ));
            assert!(matches!(
                store_bucket_keys(&keys()).await,
                Err(SecretsError::Unavailable)
            ));
            return;
        }
        for _ in 0..50 {
            if available().await {
                break;
            }
            async_io::Timer::after(Duration::from_millis(100)).await;
        }
        assert!(available().await);
        clear_bucket_keys().await.expect("clear");
        assert_eq!(load_bucket_keys().await.expect("load"), None);
        store_bucket_keys(&keys()).await.expect("store");
        assert_eq!(load_bucket_keys().await.expect("load"), Some(keys()));
        let renewed = BucketKeys {
            access_key_id: "AKIA-new".into(),
            ..keys()
        };
        store_bucket_keys(&renewed).await.expect("replace");
        assert_eq!(load_bucket_keys().await.expect("load"), Some(renewed));
        clear_bucket_keys().await.expect("clear");
        assert_eq!(load_bucket_keys().await.expect("load"), None);
    });
}
