use es_geom::{Point, Rect, Size};
use es_model::platform::{OutputId, OutputInfo};

/// Bottom of the bar above the bottom edge of the output, as for the capture bar.
pub const HOME_LIFT: f64 = 48.0;
const GAP: f64 = 8.0;
const EDGE: f64 = 4.0;
const CORNER_INSET: f64 = 16.0;

/// What a recording records, in global logical points.
#[derive(Debug, Clone, PartialEq)]
pub enum Recorded {
    Area(Rect),
    Display(OutputId),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Placement {
    On {
        output: OutputId,
        origin: Point,
    },
    /// No spot outside the video: the tray carries the controls.
    Tray,
}

/// Bottom-centre of the output, where the capture bar opens.
pub fn home(output: Rect, bar: Size) -> Point {
    Point::new(
        output.center().x - bar.width / 2.0,
        output.y1 - HOME_LIFT - bar.height,
    )
}

fn overlaps(a: Rect, b: Rect) -> bool {
    let i = a.intersect(b);
    i.width() > 0.0 && i.height() > 0.0
}

fn bar_at(origin: Point, bar: Size) -> Rect {
    Rect::new(
        origin.x,
        origin.y,
        origin.x + bar.width,
        origin.y + bar.height,
    )
}

/// Bottom-right of the output, lifted as far as the capture bar so a bottom panel stays clear.
fn corner(output: Rect, bar: Size) -> Point {
    Point::new(
        output.x1 - CORNER_INSET - bar.width,
        output.y1 - HOME_LIFT - bar.height,
    )
}

/// Keeps the control bar out of the video: home if that misses the area, else just below it, else
/// just above it, else home on another output, else inside the video in the bottom-right corner;
/// the bar stays horizontally centred on its output.
/// A whole-screen recording puts the bar on another output, or with none free in the bottom-right
/// corner of the recorded one (where the video shows it); with `whole_screen_bar` off it has none.
pub fn control_bar(
    recorded: &Recorded,
    outputs: &[OutputInfo],
    bar: Size,
    whole_screen_bar: bool,
) -> Placement {
    let area = match recorded {
        Recorded::Area(area) => *area,
        Recorded::Display(id) => {
            let Some(recorded_out) = outputs.iter().find(|o| o.id == *id) else {
                return Placement::Tray;
            };
            if !whole_screen_bar {
                return Placement::Tray;
            }
            return match outputs.iter().find(|o| o.id != *id) {
                Some(other) => Placement::On {
                    output: other.id.clone(),
                    origin: home(other.logical, bar),
                },
                None => Placement::On {
                    output: id.clone(),
                    origin: corner(recorded_out.logical, bar),
                },
            };
        }
    };
    let clear = |origin: Point| !overlaps(bar_at(origin, bar), area);
    let main = outputs.iter().max_by(|a, b| {
        let (ia, ib) = (a.logical.intersect(area), b.logical.intersect(area));
        (ia.width().max(0.0) * ia.height().max(0.0))
            .total_cmp(&(ib.width().max(0.0) * ib.height().max(0.0)))
    });
    if let Some(main) = main.filter(|o| overlaps(o.logical, area)) {
        let out = main.logical;
        let x = out.center().x - bar.width / 2.0;
        let candidates = [
            home(out, bar),
            Point::new(x, area.y1 + GAP),
            Point::new(x, area.y0 - GAP - bar.height),
        ];
        let inside = |p: Point| p.y >= out.y0 + EDGE && p.y + bar.height <= out.y1 - EDGE;
        if let Some(origin) = candidates.into_iter().find(|p| inside(*p) && clear(*p)) {
            return Placement::On {
                output: main.id.clone(),
                origin,
            };
        }
    }
    let elsewhere = outputs
        .iter()
        .map(|o| (o, home(o.logical, bar)))
        .find(|(o, p)| Some(&o.id) != main.map(|m| &m.id) && clear(*p));
    match (elsewhere, main) {
        (Some((o, origin)), _) => Placement::On {
            output: o.id.clone(),
            origin,
        },
        (None, Some(main)) => Placement::On {
            output: main.id.clone(),
            origin: corner(main.logical, bar),
        },
        (None, None) => Placement::Tray,
    }
}

/// `origin` moved the least so the whole bar stays on the output.
pub fn clamp(origin: Point, bar: Size, output: Rect) -> Point {
    Point::new(
        origin.x.min(output.x1 - bar.width).max(output.x0),
        origin.y.min(output.y1 - bar.height).max(output.y0),
    )
}

/// Where the user last left the bar on `output`, as an offset from its top-left corner. A spot
/// that covers the recorded `video` is passed over when the placement itself keeps clear of it.
pub fn remembered(
    placement: Placement,
    outputs: &[OutputInfo],
    bar: Size,
    video: Option<Rect>,
    offset: impl Fn(&OutputId) -> Option<Point>,
) -> Placement {
    let Placement::On { output, origin } = placement else {
        return placement;
    };
    let spot = outputs.iter().find(|o| o.id == output).and_then(|o| {
        offset(&o.id).map(|d| {
            clamp(
                Point::new(o.logical.x0 + d.x, o.logical.y0 + d.y),
                bar,
                o.logical,
            )
        })
    });
    let covers = |p: Point| video.is_some_and(|v| overlaps(bar_at(p, bar), v));
    let spot = spot.filter(|s| covers(origin) || !covers(*s));
    Placement::On {
        origin: spot.unwrap_or(origin),
        output,
    }
}

/// Four strips covering the output outside the area; empty ones are dropped.
pub fn highlight_strips(area: Rect, output: Rect) -> Vec<Rect> {
    let a = area.intersect(output);
    if a.width() <= 0.0 || a.height() <= 0.0 {
        return vec![output];
    }
    [
        Rect::new(output.x0, output.y0, output.x1, a.y0),
        Rect::new(output.x0, a.y1, output.x1, output.y1),
        Rect::new(output.x0, a.y0, a.x0, a.y1),
        Rect::new(a.x1, a.y0, output.x1, a.y1),
    ]
    .into_iter()
    .filter(|r| r.width() > 0.0 && r.height() > 0.0)
    .collect()
}
