set shell := ["bash", "-euo", "pipefail", "-c"]

# List recipes
default:
    @just --list

# Build the app (release)
build:
    cargo build --release -p easyshot

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

# Fail if compositor-specific names appear outside the es-backend compositor modules
grep-guard:
    scripts/grep-guard.sh

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
