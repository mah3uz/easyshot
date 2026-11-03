use super::*;

const BARSIZE: Size = CONTROL;

fn on(placement: &Placement) -> (String, Rect) {
    match placement {
        Placement::On { output, origin } => (
            output.0.clone(),
            Rect::new(
                origin.x,
                origin.y,
                origin.x + BARSIZE.width,
                origin.y + BARSIZE.height,
            ),
        ),
        Placement::Tray => panic!("expected a bar on screen"),
    }
}

fn clear_of(bar: Rect, area: Rect) -> bool {
    let i = bar.intersect(area);
    i.width() <= 0. || i.height() <= 0.
}

#[test]
fn the_control_bar_stays_out_of_an_area_near_each_edge() {
    let one = vec![output("A", 0., 1280., 720.)];
    let cases = [
        ("top", Rect::new(100., 0., 1000., 200.)),
        ("bottom", Rect::new(100., 400., 1000., 720.)),
        ("left", Rect::new(0., 100., 500., 600.)),
        ("right", Rect::new(900., 100., 1280., 700.)),
        ("middle", Rect::new(300., 100., 900., 600.)),
    ];
    for (name, area) in cases {
        let placed = control_bar(&Recorded::Area(area), &one, BARSIZE, true);
        let (out, bar) = on(&placed);
        assert_eq!(out, "A", "{name}");
        assert!(clear_of(bar, area), "{name}: {bar:?} covers {area:?}");
        assert!(
            bar.y0 >= 4. && bar.y1 <= 716.,
            "{name}: {bar:?} leaves the output"
        );
    }
    let low = Rect::new(100., 400., 1000., 720.);
    let (_, bar) = on(&control_bar(&Recorded::Area(low), &one, BARSIZE, true));
    assert_eq!(bar.y1, 392., "no room below, so just above");
    let mid = Rect::new(300., 100., 900., 650.);
    let (_, bar) = on(&control_bar(&Recorded::Area(mid), &one, BARSIZE, true));
    assert_eq!(bar.y0, 658., "home would cover it, so just below");
    let small = Rect::new(100., 0., 1000., 200.);
    let (_, bar) = on(&control_bar(&Recorded::Area(small), &one, BARSIZE, true));
    assert_eq!(
        bar.origin(),
        Point::new(508., 634.),
        "home, as the capture bar"
    );
}

#[test]
fn an_area_with_no_room_around_it_sends_the_bar_elsewhere() {
    let tall = Rect::new(0., 30., 700., 700.);
    let one = vec![output("A", 0., 1280., 720.)];
    assert_eq!(
        control_bar(&Recorded::Area(tall), &one, BARSIZE, true),
        Placement::Tray
    );
    let two = vec![
        output("A", 0., 1280., 720.),
        output("B", 1280., 1920., 1080.),
    ];
    let (out, bar) = on(&control_bar(&Recorded::Area(tall), &two, BARSIZE, true));
    assert_eq!(out, "B");
    assert!(bar.x0 >= 1280.);
}

#[test]
fn a_whole_screen_bar_goes_to_the_other_monitor() {
    let two = vec![
        output("A", 0., 1280., 720.),
        output("B", 1280., 1920., 1080.),
    ];
    let whole = Recorded::Display(OutputId("A".into()));
    let (out, bar) = on(&control_bar(&whole, &two, BARSIZE, true));
    assert_eq!(out, "B", "nothing of the bar is in the video");
    assert_eq!(
        bar.origin(),
        Point::new(1280. + 960. - 132., 1080. - 48. - 38.)
    );
    let (out, _) = on(&control_bar(
        &Recorded::Display(OutputId("B".into())),
        &two,
        BARSIZE,
        true,
    ));
    assert_eq!(out, "A");
}

#[test]
fn a_whole_screen_bar_on_the_only_monitor_sits_in_a_corner_of_it() {
    let one = vec![output("A", 0., 1280., 720.)];
    let whole = Recorded::Display(OutputId("A".into()));
    let (out, bar) = on(&control_bar(&whole, &one, BARSIZE, true));
    assert_eq!(out, "A");
    assert!(
        bar.x0 >= 0. && bar.x1 <= 1280. && bar.y0 >= 0. && bar.y1 <= 720.,
        "{bar:?} leaves the output"
    );
    assert!(bar.x0 > 640. && bar.y0 > 360., "bottom-right: {bar:?}");
}

#[test]
fn a_whole_screen_bar_that_is_switched_off_leaves_the_tray() {
    let one = vec![output("A", 0., 1280., 720.)];
    let two = vec![
        output("A", 0., 1280., 720.),
        output("B", 1280., 1920., 1080.),
    ];
    let whole = Recorded::Display(OutputId("A".into()));
    assert_eq!(control_bar(&whole, &one, BARSIZE, false), Placement::Tray);
    assert_eq!(control_bar(&whole, &two, BARSIZE, false), Placement::Tray);
}

#[test]
fn the_switch_does_not_move_a_region_bar() {
    let one = vec![output("A", 0., 1280., 720.)];
    let area = Recorded::Area(Rect::new(100., 0., 1000., 200.));
    assert_eq!(
        control_bar(&area, &one, BARSIZE, false),
        control_bar(&area, &one, BARSIZE, true)
    );
}

#[test]
fn the_highlight_dims_everything_but_the_area() {
    let out = Rect::new(0., 0., 1280., 720.);
    let area = Rect::new(100., 50., 600., 400.);
    let strips = highlight_strips(area, out);
    assert_eq!(strips.len(), 4);
    let covered: f64 = strips.iter().map(|r| r.width() * r.height()).sum();
    assert_eq!(
        covered,
        out.width() * out.height() - area.width() * area.height()
    );
    assert!(strips.iter().all(|r| clear_of(*r, area)));
    assert_eq!(
        highlight_strips(Rect::new(0., 0., 600., 720.), out).len(),
        1
    );
}

// Timer menu: 248 high, 8 above the 64-high bar; 4 padding and a 24-high header above the rows.
