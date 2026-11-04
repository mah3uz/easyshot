use std::rc::Rc;
use std::time::Duration;

use es_app::windows::{WindowKey, WindowKind};
use es_desktop::dbus::Endpoint;
use gpui::{App, Task};

use super::windows::Wiring;

pub const HOST_WAIT: Duration = Duration::from_secs(10);

pub fn open_now(show_at_launch: bool, has_tray: bool) -> bool {
    show_at_launch || !has_tray
}

fn open_bar(wiring: &Wiring, cx: &mut App) {
    if let Err(e) = wiring
        .windows
        .open(WindowKind::CaptureBar, WindowKey::Single, cx)
    {
        tracing::warn!(error = %e, "capture bar at launch");
    }
}

/// The capture bar opens when the setting asks, or when no tray host shows up within `HOST_WAIT`.
pub fn at_launch(
    show_at_launch: bool,
    has_tray: bool,
    wiring: &Rc<Wiring>,
    tasks: &mut Vec<Task<()>>,
    cx: &mut App,
) {
    if open_now(show_at_launch, has_tray) {
        open_bar(wiring, cx);
        return;
    }
    let wiring = wiring.clone();
    tasks.push(cx.spawn(async move |cx| {
        let timer = cx.background_executor().timer(HOST_WAIT);
        let found = cx
            .background_executor()
            .spawn(async move { es_desktop::host_within(&Endpoint::session(), timer).await })
            .await;
        if !found {
            cx.update(|cx| open_bar(&wiring, cx));
        }
    }));
}

#[cfg(test)]
mod tests {
    use super::open_now;

    #[test]
    fn a_present_tray_leaves_the_bar_to_the_setting_and_a_missing_one_opens_it() {
        assert!(open_now(true, true));
        assert!(open_now(false, false));
        assert!(!open_now(false, true));
    }
}
