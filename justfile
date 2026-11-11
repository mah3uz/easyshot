set shell := ["bash", "-euo", "pipefail", "-c"]

# The features the package is built with
shipped_features := "easyshot/sound,easyshot/player,easyshot/evdev-input"

# The released version: the easyshot binary crate's, as the internal crates keep their own
version := `sed -n 's/^version = "\(.*\)"/\1/p' crates/easyshot/Cargo.toml | head -1`

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
    cargo build --profile {{ profile }} --locked -p easyshot --features {{ shipped_features }}
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

# The whole release, e.g. `just release 0.2.0`: checks, the gate, version bump and stamps, the tag built locally, then on your yes the push and `just publish`
release ver:
    #!/usr/bin/env bash
    set -euo pipefail
    v={{ ver }} tag=v{{ ver }} current={{ version }}
    fail() { echo "release: $*" >&2; exit 1; }
    key=${MINISIGN_KEY:-$HOME/.minisign/easyshot.key}
    last=$(git describe --tags --abbrev=0 --match 'v*' --exclude 'v*-*' 2>/dev/null || true)
    last=${last#v}
    last=${last:-0.0.0}
    unreleased=$(scripts/release-notes.sh Unreleased 2>/dev/null || true)

    [[ $v =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || fail "give the version as x.y.z, e.g. just release 0.2.0"
    [[ $v != "$last" && $(printf '%s\n%s\n' "$last" "$v" | sort -V | tail -1) == "$v" ]] || fail "$v is not newer than the last release ($last)"
    grep -q '^- ' <<<"$unreleased" || fail "CHANGELOG.md has nothing under ## Unreleased"
    ! grep -q "^## $v\( \|$\)" CHANGELOG.md || fail "CHANGELOG.md already has a ## $v section"
    [[ $(git branch --show-current) == main ]] || fail "switch to main first"
    [[ -z $(git status --porcelain) ]] || fail "commit or stash your changes first"
    git fetch -q origin
    [[ $(git rev-list --count HEAD..origin/main) == 0 ]] || fail "main is behind origin/main; pull first"
    ! git rev-parse -q --verify "refs/tags/$tag" >/dev/null || fail "tag $tag already exists"
    ! git ls-remote --exit-code --tags origin "$tag" >/dev/null || fail "tag $tag already exists on origin"
    gh auth status >/dev/null 2>&1 || fail "gh is not logged in; run gh auth login"
    ssh -o BatchMode=yes aur@aur.archlinux.org help >/dev/null 2>&1 || fail "can't reach the AUR over SSH; add your key at https://aur.archlinux.org/account"
    (cd docs/site && { [ -d node_modules ] || npm ci --silent; } && [[ $(npx wrangler whoami 2>&1) == *"You are logged in"* ]]) ||
        fail "wrangler is not logged in, and the website is deployed at the end; run: cd docs/site && npx wrangler login"
    [[ -f $key ]] || fail "no signing key at $key; create one with: mkdir -p ~/.minisign && minisign -G -p ~/.minisign/easyshot.pub -s ~/.minisign/easyshot.key"
    cmp -s packaging/aur/easyshot/easyshot.install packaging/aur/easyshot-bin/easyshot.install || fail "the two easyshot.install files differ"

    echo "==> lint and tests"
    just check

    echo "==> version $v (last release: $last)"
    if [[ $v != "$current" ]]; then
        sed -i "0,/^version = \".*\"/s//version = \"$v\"/" crates/easyshot/Cargo.toml
        cargo update -w --offline -q
    fi
    for p in easyshot easyshot-bin; do
        sed -i -e "s/^pkgver=.*/pkgver=$v/" -e "s/^pkgrel=.*/pkgrel=1/" -e "s/^sha256sums=.*/sha256sums=('SKIP')/" "packaging/aur/$p/PKGBUILD"
    done
    # To the second, with the UTC offset, since one day can hold several releases.
    now=$(date +%Y-%m-%dT%H:%M:%S%:z)
    sed -i "s/^## Unreleased$/## Unreleased\n\n## $v - $now/" CHANGELOG.md
    meta=packaging/data/metainfo.xml.in
    if grep -q "<release version=\"$v\"" "$meta"; then
        sed -i "s|<release version=\"$v\"[^/]*/>|<release version=\"$v\" date=\"$now\"/>|" "$meta"
    else
        sed -i "s|<releases>|<releases>\n    <release version=\"$v\" date=\"$now\"/>|" "$meta"
    fi
    git commit -qam "easyshot $v"

    # The tag stays local until the yes, and the release is built from it now, so a build that fails stops before anything is public.
    git tag -a "$tag" -m "easyshot $v"
    trap 'git tag -d "$tag" >/dev/null; echo "release: removed the local tag $tag. To undo the version commit: git reset --hard HEAD~1" >&2' EXIT
    echo "==> build $tag"
    scripts/release-build.sh "$v"

    # Everything after this is public and can't be taken back.
    read -rp "Push $tag, publish the GitHub release and AUR packages, and deploy the website? [y/N] " answer
    [[ $answer == [yY] ]] || exit 1

    echo "==> push"
    git push -q --atomic origin main "$tag"
    trap - EXIT
    just publish "$v"

# After v<VER> is pushed: build the tag into dist/, publish the GitHub release with its notes, push the AUR packages and deploy the website; safe to run again
publish ver:
    #!/usr/bin/env bash
    set -euo pipefail
    v={{ ver }} tag=v{{ ver }}
    fail() { echo "publish: $*" >&2; exit 1; }
    trap 'echo "publish: stopped; fix the cause and run just publish $v again" >&2' ERR
    [[ $(git branch --show-current) == main ]] || fail "switch to main first"
    [[ -z $(git status --porcelain) ]] || fail "commit or stash your changes first"
    git ls-remote --exit-code --tags origin "$tag" >/dev/null || fail "push $tag first"
    for p in easyshot easyshot-bin; do
        grep -q "^pkgver=$v$" "packaging/aur/$p/PKGBUILD" || fail "packaging/aur/$p is not at $v"
    done
    notes=$(scripts/release-notes.sh "$v")

    if [[ $(cat dist/.built 2>/dev/null) == "$(git rev-parse "$tag^{commit}")" ]]; then
        echo "==> $tag: dist/ is already built"
    else
        echo "==> build $tag"
        scripts/release-build.sh "$v"
    fi

    echo "==> GitHub release"
    if gh release view "$tag" >/dev/null 2>&1; then
        gh release edit "$tag" --notes "$notes" >/dev/null
        gh release upload "$tag" --clobber dist/*
    else
        gh release create "$tag" --verify-tag --title "easyshot $v" --notes "$notes" --latest=false dist/* ||
            gh release upload "$tag" --clobber dist/*
    fi
    # install.sh downloads releases/latest, so a release becomes latest only once all its assets are up, and never over a newer one.
    if [[ $(git describe --tags --abbrev=0 --match 'v*' --exclude 'v*-*') == "$tag" ]]; then
        gh release edit "$tag" --latest >/dev/null
    fi

    echo "==> AUR checksums"
    for p in easyshot easyshot-bin; do
        file=$(cd "packaging/aur/$p" && makepkg --printsrcinfo | sed -n 's|^[[:space:]]*source = .*/||p')
        [[ -f dist/$file ]] || fail "dist/ has no $file, which $p downloads"
        sed -i "s/^sha256sums=.*/sha256sums=('$(sha256sum "dist/$file" | cut -d' ' -f1)')/" "packaging/aur/$p/PKGBUILD"
    done
    if [[ -n $(git status --porcelain packaging/aur) ]]; then
        git commit -qm "Release $v: AUR checksums" packaging/aur
        git push -q origin main
    fi

    echo "==> AUR: easyshot and easyshot-bin"
    just aur

    echo "==> the website"
    just site-deploy

    echo
    echo "Published $v: $(gh release view "$tag" --json url --jq .url)"

# Publish packaging/aur (easyshot, easyshot-bin) to the AUR through throwaway clones in target/aur; bump pkgrel and commit first for a packaging-only fix
aur:
    #!/usr/bin/env bash
    set -euo pipefail
    ver={{ version }}
    fail() { echo "aur: $*" >&2; exit 1; }
    [[ -z $(git status --porcelain packaging/aur) ]] || fail "commit packaging/aur first"
    cmp -s packaging/aur/easyshot/easyshot.install packaging/aur/easyshot-bin/easyshot.install || fail "the two easyshot.install files differ"
    assets=$(gh release view "v$ver" --json assets --jq '.assets[].name') || fail "there is no v$ver GitHub release; run just publish $ver"
    # Every package is checked before any is pushed, so the AUR never holds one at $ver and the other not.
    for p in easyshot easyshot-bin; do
        src=packaging/aur/$p
        grep -q "^pkgver=$ver$" "$src/PKGBUILD" || fail "$p: pkgver is not $ver"
        ! grep -q "^sha256sums=.*SKIP" "$src/PKGBUILD" || fail "$p: no checksum yet; run just publish $ver"
        file=$(cd "$src" && makepkg --printsrcinfo | sed -n 's|^[[:space:]]*source = .*/||p')
        grep -qxF "$file" <<<"$assets" || fail "$p: the v$ver GitHub release has no $file; run just publish $ver"
    done
    mkdir -p target/aur
    for p in easyshot easyshot-bin; do
        src=packaging/aur/$p
        dir=target/aur/$p
        rm -rf "$dir"
        # a new package's AUR repo is empty, which git warns about
        git clone -q "ssh://aur@aur.archlinux.org/$p.git" "$dir" 2>/dev/null || fail "$p: can't clone ssh://aur@aur.archlinux.org/$p.git"
        cp "$src/PKGBUILD" "$src/easyshot.install" "$dir/"
        (cd "$dir" && makepkg --printsrcinfo > .SRCINFO)
        git -C "$dir" add PKGBUILD .SRCINFO easyshot.install
        if git -C "$dir" diff --cached --quiet; then
            echo "$p: already up to date"
        else
            rel=$(sed -n 's/^pkgrel=//p' "$src/PKGBUILD")
            git -C "$dir" commit -qm "Update to $ver-$rel"
            # the AUR only accepts master, whatever init.defaultBranch named the clone's branch
            git -C "$dir" push -q origin HEAD:master
            echo "$p: pushed $ver-$rel"
        fi
        rm -rf "$dir"
    done

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

# Run every test in the workspace, built once with the shipped features and the test hooks
test:
    scripts/test.sh
    scripts/test.sh --doc

# Run the tests of the packages changed against main
test-changed:
    scripts/test-changed.sh

# Delete the dev build output when it is over MAX_GB; cargo never removes stale artifacts itself
sweep MAX_GB="80":
    #!/usr/bin/env bash
    set -euo pipefail
    [ -d target/debug ] || exit 0
    gb=$(( $(du -s --block-size=1G target/debug | cut -f1) ))
    if [ "$gb" -gt "{{ MAX_GB }}" ]; then
        echo "target/debug is ${gb} GB (over {{ MAX_GB }}): removing it"
        cargo clean --profile dev
    else
        echo "target/debug is ${gb} GB (limit {{ MAX_GB }})"
    fi

# Check formatting and run clippy with warnings as errors
lint:
    cargo fmt --check
    cargo clippy --workspace --all-targets --features {{ shipped_features }} -- -D warnings

# Fail if a crate depends upward across the layers, or on a forbidden crate
layers:
    scripts/layers.sh

# Fail if compositor-specific names appear outside the es-backend compositor modules, or unsafe outside es-wayland
grep-guard:
    scripts/grep-guard.sh

# Fail if any Rust file outside vendor/ is over 600 lines
file-size:
    scripts/file-size.sh

# Regenerate LICENSES/THIRD-PARTY.md from the release dependency graph
third-party:
    cargo about generate --offline --locked -m crates/easyshot/Cargo.toml about.hbs -o LICENSES/THIRD-PARTY.md

# Fail on init/install/register/bind functions nothing calls and events nothing subscribes to
wiring-check:
    scripts/wiring-check.py

# Fail on duplicate, missing or unreferenced Fluent ids and unused icons
i18n-check:
    scripts/i18n-check.sh

# Fail if two versions of any gpui* or zbus crate end up in the graph
gpui-guard:
    scripts/gpui-guard.sh

# Fail if the release build can contain test hooks (build it first)
release-check:
    scripts/release-check.sh

# Run install.sh from a staged release, and the one-line installer against a local fake release, into temporary directories
install-test:
    scripts/install-test.sh
    scripts/install-bootstrap-test.sh

# The gate: everything that must pass before a merge
check: lint layers grep-guard file-size i18n-check wiring-check gpui-guard install-test test
    tests/harness/sway.sh platform
    tests/harness/niri.sh platform
    tests/harness/hypr.sh platform

# Regenerate every icon from the logo masters in branding/
logo:
    scripts/logo.sh

# Benchmarks (GPUI BenchContext)
bench:
    cargo bench --workspace

# Run the website's dev server (docs/site); arguments go to nuxt dev, e.g. `just site-dev --port 4000`
[working-directory: 'docs/site']
site-dev *ARGS:
    [ -d node_modules ] || npm ci
    npx nuxt dev {{ ARGS }}

# Build the website into docs/site/.output/public
[working-directory: 'docs/site']
site-build:
    [ -d node_modules ] || npm ci
    npm run build

# Build the website and deploy it to Cloudflare Workers (needs `npx wrangler login` once)
[working-directory: 'docs/site']
site-deploy:
    [ -d node_modules ] || npm ci
    npm run deploy
