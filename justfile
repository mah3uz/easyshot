set shell := ["bash", "-euo", "pipefail", "-c"]

# List recipes
default:
    @just --list

# Build the app (release)
build:
    cargo build --release -p easyshot

# Build the Arch package from the working tree into dist/, as a release's would be built from its tarball
package profile="release":
    #!/usr/bin/env bash
    set -euo pipefail
    cargo build --profile {{ profile }} --locked -p easyshot --features sound,player,portal-recorder
    src=packaging/aur/easyshot-bin
    name=easyshot-$(sed -n 's/^pkgver=//p' $src/PKGBUILD)-x86_64-unknown-linux-gnu
    rm -rf dist/build && mkdir -p dist/build
    packaging/stage.sh target/{{ profile }} . "dist/build/$name"
    tar -C dist/build -czf "dist/build/$name.tar.gz" "$name"
    cp $src/PKGBUILD $src/easyshot.install dist/build/
    # The package a release serves, made from this tarball in place of the release's.
    (cd dist/build && PKGDEST=$PWD makepkg -f --skipchecksums)
    mv dist/build/easyshot-bin-*.pkg.tar.zst dist/
    rm -rf dist/build

# One release's section of CHANGELOG.md, as its release notes; `Unreleased` shows what is coming
release-notes v:
    scripts/release-notes.sh {{ v }}

# Build the tag v<version> into dist/: tarballs, checksums, signature and the AUR packages' checksums
release:
    scripts/release.sh

# On Arch Linux: build the package from the working tree and install it with pacman
install: (package "fast")
    #!/usr/bin/env bash
    set -euo pipefail
    # Built without link-time optimisation, as the benches build: a change is ready in minutes, not many.
    src=packaging/aur/easyshot-bin/PKGBUILD
    sudo pacman -U "dist/easyshot-bin-$(sed -n 's/^pkgver=//p' $src)-$(sed -n 's/^pkgrel=//p' $src)-x86_64.pkg.tar.zst"
    easyshot --version

# Remove the installed package. Settings and captures are left alone
uninstall:
    sudo pacman -R easyshot-bin

# Run the app from this checkout (release); arguments go to easyshot
run *ARGS: build
    ./target/release/easyshot {{ ARGS }}

# Run the app (debug build)
dev *ARGS:
    cargo run -p easyshot -- {{ ARGS }}

# Run every test in the workspace
test:
    cargo nextest run --workspace
    cargo test --workspace --doc

# Run the tests of the packages changed against main
test-changed:
    scripts/test-changed.sh

# Check formatting and run clippy with warnings as errors
lint:
    cargo fmt --check
    cargo clippy --workspace --all-targets -- -D warnings

# Fail if a crate depends upward across the layers, or on a forbidden crate
layers:
    scripts/layers.sh

# Fail if compositor-specific names appear outside the es-backend compositor modules, or unsafe outside es-wayland
grep-guard:
    scripts/grep-guard.sh

# Regenerate LICENSES/THIRD-PARTY.md from the release dependency graph
third-party:
    cargo about generate --offline --locked -m crates/easyshot/Cargo.toml about.hbs -o LICENSES/THIRD-PARTY.md

# Fail on duplicate or missing Fluent ids
i18n-check:
    scripts/i18n-check.sh

# Fail if two versions of any gpui* or zbus crate end up in the graph
gpui-guard:
    scripts/gpui-guard.sh

# Fail if the release build can contain test hooks (build it first)
release-check:
    scripts/release-check.sh

# The gate: everything that must pass before a merge
check: lint layers grep-guard i18n-check gpui-guard test
    tests/harness/sway.sh platform
    tests/harness/niri.sh platform
    tests/harness/hypr.sh platform
    cargo build -p easyshot --release
    just release-check

# Regenerate every icon from the logo masters in branding/
logo:
    scripts/logo.sh

# Benchmarks (GPUI BenchContext)
bench:
    cargo bench --workspace
