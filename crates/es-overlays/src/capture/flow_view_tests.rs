use super::*;

#[test]
fn a_scroll_capture_starts_on_a_region_and_the_window_start_is_the_alternate() {
    assert_eq!(view_for(SelectMode::Scroll), Some(View::Scroll));
    assert_eq!(view_for(SelectMode::ScrollWindow), Some(View::ScrollWindow));
    assert_eq!(view_for(SelectMode::Window), Some(View::Window));
}

#[test]
fn a_window_pick_with_no_windows_shows_the_region_when_a_switch_is_queued() {
    assert!(matches!(
        first_view(View::Window, true, false),
        Err(Error::Unsupported)
    ));
    assert_eq!(
        first_view(View::Window, true, true).ok(),
        Some(View::Region)
    );
    assert_eq!(
        first_view(View::Window, false, false).ok(),
        Some(View::Window)
    );
    assert_eq!(
        first_view(View::Region, true, false).ok(),
        Some(View::Region)
    );
}

#[test]
fn space_swaps_window_and_region_without_leaving_the_kind_of_capture() {
    assert_eq!(toggled(View::ScrollWindow, false), Some(View::Scroll));
    assert_eq!(toggled(View::Scroll, true), Some(View::ScrollWindow));
    assert_eq!(toggled(View::Window, false), Some(View::Region));
    assert_eq!(toggled(View::Region, true), Some(View::Window));
    assert_eq!(toggled(View::Color, true), None);
    assert_eq!(toggled(View::Text, false), None);
}

#[test]
fn with_no_window_to_pick_a_window_scroll_starts_on_a_region() {
    let mut setup = Setup {
        options: FlowOptions::default(),
        last: None,
        windows: Vec::new(),
        geometry: false,
    };
    assert_eq!(usable(View::ScrollWindow, &setup), View::Scroll);
    assert_eq!(usable(View::Window, &setup), View::Window);
    setup.geometry = true;
    assert_eq!(usable(View::ScrollWindow, &setup), View::ScrollWindow);
}

// Space and a click in one input batch: the click reaches the window view before the region
// view replaces it, so it must still start a scroll capture of that window.
#[test]
fn a_pick_keeps_the_view_it_was_made_in_after_space_leaves_it() {
    let click = PickEvent::Window {
        id: WindowId("page".into()),
        local: None,
    };
    assert_eq!(toggled(View::ScrollWindow, false), Some(View::Scroll));
    assert!(matches!(
        pick(&click, View::ScrollWindow),
        Pick::Window {
            made: View::ScrollWindow,
            ..
        }
    ));
    assert!(matches!(
        pick(&click, View::Window),
        Pick::Window {
            made: View::Window,
            ..
        }
    ));
}
