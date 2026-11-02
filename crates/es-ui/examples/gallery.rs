fn main() {
    gpui_platform::application()
        .with_assets(es_ui::assets::Assets)
        .run(es_ui::components::gallery::open);
}
