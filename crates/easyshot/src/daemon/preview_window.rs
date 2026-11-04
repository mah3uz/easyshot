use super::windows::{handle, opened};
use es_app::errors::{Error, Result};
use es_app::windows::{WindowHandle, WindowKey};
use gpui::App;
use std::rc::Rc;

pub fn open(key: &WindowKey, cx: &mut App) -> Result<Rc<dyn WindowHandle>> {
    let WindowKey::Path(file) = key else {
        return Err(Error::Unsupported);
    };
    let window = es_shell::preview::open(file, cx)
        .ok_or_else(|| Error::NotFound(file.display().to_string()))?;
    Ok(handle(opened(window)?))
}
