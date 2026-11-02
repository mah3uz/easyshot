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
    if let Err(error) = loader.load_languages(&Localizations, &[loader.fallback_language().clone()])
    {
        tracing::error!(%error, "cannot load es-overlays strings");
    }
    // Only bundles that exist when it is called take the setting.
    loader.set_use_isolating(false);
    loader
});

pub fn loader() -> &'static FluentLanguageLoader {
    &LOADER
}

macro_rules! fl {
    ($message_id:literal) => {
        i18n_embed_fl::fl!($crate::capture::strings::loader(), $message_id)
    };
    ($message_id:literal, $($args:tt)*) => {
        i18n_embed_fl::fl!($crate::capture::strings::loader(), $message_id, $($args)*)
    };
}

pub(crate) use fl;

#[cfg(test)]
mod tests {
    #[test]
    fn arguments_are_not_wrapped_in_isolation_marks() {
        let text = i18n_embed_fl::fl!(super::loader(), "region-size", w = 3, h = 4);
        assert_eq!(text, "3 × 4");
    }
}
