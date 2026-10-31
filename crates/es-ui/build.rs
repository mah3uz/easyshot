use std::{fs, path::Path, process::exit};

fn main() {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let list = Path::new(&manifest).join("../../assets/icons/lucide.txt");
    let dir = Path::new(&manifest).join("../../assets/icons/lucide");
    println!("cargo:rerun-if-changed={}", list.display());
    println!("cargo:rerun-if-changed={}", dir.display());

    let names = fs::read_to_string(&list).unwrap_or_else(|e| {
        eprintln!("error: cannot read {}: {e}", list.display());
        exit(1);
    });
    let missing: Vec<&str> = names
        .lines()
        .map(str::trim)
        .filter(|n| !n.is_empty() && !dir.join(format!("{n}.svg")).is_file())
        .collect();
    if !missing.is_empty() {
        eprintln!(
            "error: lucide.txt lists icons with no SVG in assets/icons/lucide/: {}",
            missing.join(", ")
        );
        exit(1);
    }
}
