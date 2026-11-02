# Licences and notices

easyshot itself is MIT (`../LICENSE`). Only what easyshot ships or links is listed. Their code and assets keep their own terms, and the declared licence of a distributed package lists them.

| File | What it covers | Terms |
|---|---|---|
| `THIRD-PARTY.md` | Every Rust crate linked into the `easyshot` binary, with its licence text; regenerate with `just third-party` | MIT, Apache-2.0, Unicode-3.0, BSD-3-Clause, BSD-2-Clause, ISC, Zlib, 0BSD, CC0-1.0, MPL-2.0, Unlicense |
| `Inter-OFL-1.1.txt`, `InterTight-OFL-1.1.txt`, `Nunito-OFL-1.1.txt` | Bundled fonts in `assets/fonts` (see its SOURCES.md) | OFL-1.1 |
| `Lucide-ISC.txt` | Lucide icons in `assets/icons/lucide` and in the gpui-kit-assets crate (includes the Feather MIT notice) | ISC, MIT |
| `tiny-skia-BSD-3-Clause.txt` | tiny-skia (Google Inc., Yevhenii Reizner) | BSD-3-Clause |
| `Apache-2.0.txt` | gpui-pre (Zed Industries, Inc.) and gpui-kit / gpui-kit-assets (longbridge) | Apache-2.0 |

`vendor/gpui-pre/` and `vendor/gpui-pre-linux/` are modified copies of the crates.io releases: easyshot's changes are the git commits named `gpui patch` on top of the original copy.

easyshot's own assets are not third-party: the app icon (`assets/icons/app`, `assets/icons/easyshot.svg`), the tool icons `assets/icons/{counter,highlight,pixelate}.svg` and `assets/banner` are MIT with the code; `assets/wallpapers/*.jpg` (`assets/wallpapers/SOURCES.md`), `assets/practice.png` (`assets/SOURCES.md`) and `assets/sounds/shutter.ogg` (`assets/sounds/SOURCES.md`) are dedicated CC0-1.0.

A binary release is therefore `MIT AND Apache-2.0 AND Unicode-3.0 AND BSD-3-Clause AND BSD-2-Clause AND ISC AND Zlib AND 0BSD AND CC0-1.0 AND MPL-2.0 AND Unlicense AND OFL-1.1`.

Texts are fetched from the upstream repositories; each file that adds a preface says so and gives the source URL.
