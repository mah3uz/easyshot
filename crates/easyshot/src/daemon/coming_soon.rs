use es_app::platform_ui::{ComingSoonSurface, LayerShellSurfaces};
pub(super) use es_ui::components::Feature;
use gpui::{App, Global};
use std::rc::Rc;

type Show = Rc<dyn Fn(Feature, &mut App)>;

struct Panels(Show);

impl Global for Panels {}

pub(super) fn install(surfaces: Rc<LayerShellSurfaces>, cx: &mut App) {
    let surface = ComingSoonSurface::new(surfaces);
    cx.set_global(Panels(Rc::new(move |feature, cx| {
        if let Err(error) = surface.show(feature, cx) {
            tracing::warn!(%error, ?feature, "coming soon panel not shown");
        }
    })));
}

/// Every feature that is not built yet answers through here.
pub(super) fn coming_soon(feature: Feature, cx: &mut App) {
    let Some(show) = cx.try_global::<Panels>().map(|p| p.0.clone()) else {
        tracing::warn!(?feature, "coming soon panel dropped: none installed");
        return;
    };
    show(feature, cx);
}

/// Records each feature asked for instead of opening a surface.
#[cfg(test)]
pub(super) fn record(cx: &mut App) -> Rc<std::cell::RefCell<Vec<Feature>>> {
    let asked: Rc<std::cell::RefCell<Vec<Feature>>> = Rc::default();
    let into = asked.clone();
    cx.set_global(Panels(Rc::new(move |feature, _| {
        into.borrow_mut().push(feature)
    })));
    asked
}

#[cfg(test)]
#[path = "coming_soon_tests.rs"]
mod tests;
