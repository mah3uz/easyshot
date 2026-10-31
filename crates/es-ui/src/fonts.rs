use gpui::{App, AssetSource};

use crate::assets::Assets;

pub fn register(cx: &mut App) {
    let faces = match Assets.list("fonts/") {
        Ok(paths) => paths,
        Err(error) => {
            tracing::error!(%error, "cannot list bundled fonts");
            return;
        }
    };
    let mut data = Vec::new();
    for path in faces.iter().filter(|p| p.ends_with(".ttf") || p.ends_with(".otf")) {
        match Assets.load(path) {
            Ok(Some(bytes)) => data.push(bytes),
            Ok(None) => tracing::error!(%path, "bundled font missing"),
            Err(error) => tracing::error!(%path, %error, "cannot read bundled font"),
        }
    }
    if let Err(error) = cx.text_system().add_fonts(data) {
        tracing::error!(%error, "cannot register bundled fonts");
    }
}
