use std::path::PathBuf;
use std::process::Command;

fn git(args: &[&str]) -> Option<String> {
    let dir = std::env::var_os("CARGO_MANIFEST_DIR")?;
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

fn mark() -> String {
    if std::env::var_os("EASYSHOT_RELEASE").is_some_and(|set| !set.is_empty()) {
        return String::new();
    }
    let Some(hash) = git(&["rev-parse", "--short=8", "HEAD"]) else {
        return String::new();
    };
    let dirty = git(&["status", "--porcelain", "--untracked-files=no"])
        .is_some_and(|changes| !changes.is_empty());
    format!("+{hash}{}", if dirty { "-dirty" } else { "" })
}

fn main() {
    println!("cargo:rerun-if-env-changed=EASYSHOT_RELEASE");
    if let Some(dir) = git(&["rev-parse", "--absolute-git-dir"]).map(PathBuf::from) {
        let common = git(&["rev-parse", "--path-format=absolute", "--git-common-dir"])
            .map(PathBuf::from)
            .unwrap_or_else(|| dir.clone());
        println!("cargo:rerun-if-changed={}", dir.join("HEAD").display());
        println!("cargo:rerun-if-changed={}", dir.join("index").display());
        println!(
            "cargo:rerun-if-changed={}",
            common.join("packed-refs").display()
        );
        if let Some(head) = git(&["symbolic-ref", "-q", "HEAD"]) {
            println!("cargo:rerun-if-changed={}", common.join(head).display());
        }
    }
    let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_default();
    println!("cargo:rustc-env=EASYSHOT_VERSION=v{version}{}", mark());
}
