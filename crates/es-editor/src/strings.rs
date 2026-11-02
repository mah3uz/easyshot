use i18n_embed::{
    LanguageLoader,
    fluent::{FluentLanguageLoader, fluent_language_loader},
};
use rust_embed::RustEmbed;
use std::sync::LazyLock;

#[derive(RustEmbed)]
#[folder = "../../i18n"]
struct Localizations;

static LOADER: LazyLock<FluentLanguageLoader> = LazyLock::new(|| {
    let loader = fluent_language_loader!();
    loader.set_use_isolating(false);
    if let Err(error) = loader.load_languages(&Localizations, &[loader.fallback_language().clone()])
    {
        tracing::error!(%error, "cannot load es-editor strings");
    }
    loader
});

pub fn loader() -> &'static FluentLanguageLoader {
    &LOADER
}

#[expect(unused_macros)]
macro_rules! fl {
    ($message_id:literal) => {
        i18n_embed_fl::fl!($crate::strings::loader(), $message_id)
    };
    ($message_id:literal, $($args:tt)*) => {
        i18n_embed_fl::fl!($crate::strings::loader(), $message_id, $($args)*)
    };
}

#[expect(unused_imports)]
pub(crate) use fl;
