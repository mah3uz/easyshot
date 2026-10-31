use std::borrow::Cow;

use gpui::{AssetSource, Result, SharedString};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "../../assets"]
#[include = "icons/**"]
#[include = "fonts/**"]
#[exclude = "icons/app/hicolor/**"]
struct Embedded;

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        match Embedded::get(path) {
            Some(file) => Ok(Some(file.data)),
            None => gpui_kit_assets::Assets.load(path),
        }
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut paths: Vec<SharedString> =
            Embedded::iter().filter(|name| name.starts_with(path)).map(|name| name.to_string().into()).collect();
        paths.extend(gpui_kit_assets::Assets.list(path)?);
        Ok(paths)
    }
}
