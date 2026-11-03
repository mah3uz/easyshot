#[test]
fn editor_strings_resolve_from_the_catalogue() {
    assert!(es_editor::strings::loader().has("editor-tool-arrow"));
    assert_eq!(
        i18n_embed_fl::fl!(es_editor::strings::loader(), "editor-tool-arrow"),
        "Arrow"
    );
}
