use es_app::deck::DeckAction;
use es_geom::{Point, Rect, Size};
use es_model::config::CardActions;
use std::collections::BTreeMap;

pub const SPACING: f64 = 10.;
pub const HEADER: f64 = 26.;
/// Room on the far sides of the cards for their shadow.
pub const SHADOW_ROOM: f64 = 24.;
pub const DEFAULT_MARGIN: f64 = 20.;
pub const MAX_MARGIN: f64 = 48.;
/// How far each card folded behind the newest shows above it.
pub const PEEK: f64 = 7.;
/// How much smaller each card folded behind is than the one in front of it.
pub const PEEK_SHRINK: f64 = 0.05;
pub const PEEKING: usize = 2;
pub const BADGE: f64 = 20.;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CardSize {
    #[default]
    Small,
    Medium,
    Large,
}

impl CardSize {
    pub fn parse(s: &str) -> Self {
        match s {
            "medium" => CardSize::Medium,
            "large" => CardSize::Large,
            _ => CardSize::Small,
        }
    }

    pub fn size(self) -> Size {
        match self {
            CardSize::Small => Size::new(180., 130.),
            CardSize::Medium => Size::new(240., 172.),
            CardSize::Large => Size::new(300., 216.),
        }
    }

    /// Hover controls grow with the card.
    pub fn control_scale(self) -> f32 {
        match self {
            CardSize::Small => 1.15,
            CardSize::Medium => 1.35,
            CardSize::Large => 1.6,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Corner {
    #[default]
    BottomRight,
    BottomLeft,
}

impl Corner {
    pub fn parse(s: &str) -> Self {
        match s {
            "bottom-left" => Corner::BottomLeft,
            _ => Corner::BottomRight,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Slot {
    TopLeft,
    TopRight,
    CenterLeft,
    CenterRight,
    BottomLeft,
    BottomRight,
    TopCenter,
    BottomCenter,
}

impl Slot {
    pub const ALL: [Slot; 8] = [
        Slot::TopLeft,
        Slot::TopRight,
        Slot::CenterLeft,
        Slot::CenterRight,
        Slot::BottomLeft,
        Slot::BottomRight,
        Slot::TopCenter,
        Slot::BottomCenter,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Slot::TopLeft => "top-left",
            Slot::TopRight => "top-right",
            Slot::CenterLeft => "center-left",
            Slot::CenterRight => "center-right",
            Slot::BottomLeft => "bottom-left",
            Slot::BottomRight => "bottom-right",
            Slot::TopCenter => "top-center",
            Slot::BottomCenter => "bottom-center",
        }
    }

    pub fn is_center(self) -> bool {
        matches!(self, Slot::CenterLeft | Slot::CenterRight)
    }

    fn index(self) -> usize {
        self as usize
    }
}

pub fn action_name(action: DeckAction) -> &'static str {
    match action {
        DeckAction::Pin => "pin",
        DeckAction::Dismiss => "dismiss",
        DeckAction::Copy => "copy",
        DeckAction::Save => "save",
        DeckAction::Edit => "edit",
        DeckAction::Share => "share",
        DeckAction::ShowInFolder => "show-in-folder",
        DeckAction::Preview => "preview",
        DeckAction::Delete => "delete",
    }
}

/// Whether Settings › Overlay has `action` on; Dismiss always is.
pub fn chosen(actions: &CardActions, action: DeckAction) -> bool {
    match action {
        DeckAction::Dismiss => true,
        DeckAction::Copy => actions.copy,
        DeckAction::Save | DeckAction::ShowInFolder => actions.save,
        DeckAction::Edit => actions.edit,
        DeckAction::Pin => actions.pin,
        DeckAction::Share => actions.share,
        DeckAction::Preview => actions.preview,
        DeckAction::Delete => actions.delete,
    }
}

fn parse_action(s: &str) -> Option<DeckAction> {
    DeckAction::ALL.into_iter().find(|a| action_name(*a) == s)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutPreset {
    Standard,
    Sharing,
    Minimal,
    Custom,
}

impl LayoutPreset {
    pub fn name(self) -> &'static str {
        match self {
            LayoutPreset::Standard => "standard",
            LayoutPreset::Sharing => "sharing",
            LayoutPreset::Minimal => "minimal",
            LayoutPreset::Custom => "custom",
        }
    }

    pub fn layout(self) -> Option<ToolLayout> {
        use DeckAction as A;
        let slots = match self {
            LayoutPreset::Standard => [
                Some(A::Pin),
                Some(A::Dismiss),
                Some(A::Copy),
                Some(A::Save),
                Some(A::Edit),
                Some(A::Share),
                Some(A::Preview),
                Some(A::Delete),
            ],
            LayoutPreset::Sharing => [
                Some(A::Pin),
                Some(A::Dismiss),
                Some(A::Copy),
                Some(A::Share),
                Some(A::Edit),
                Some(A::Save),
                Some(A::Preview),
                Some(A::Delete),
            ],
            LayoutPreset::Minimal => [
                None,
                Some(A::Dismiss),
                Some(A::Copy),
                Some(A::Save),
                None,
                None,
                Some(A::Preview),
                Some(A::Delete),
            ],
            LayoutPreset::Custom => return None,
        };
        Some(ToolLayout { slots })
    }
}

/// Which action sits in each of a card's eight hover slots; each at most once, and Dismiss always.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolLayout {
    slots: [Option<DeckAction>; 8],
}

impl Default for ToolLayout {
    fn default() -> Self {
        Self::standard()
    }
}

impl ToolLayout {
    pub fn standard() -> Self {
        Self {
            slots: [
                Some(DeckAction::Pin),
                Some(DeckAction::Dismiss),
                Some(DeckAction::Copy),
                Some(DeckAction::Save),
                Some(DeckAction::Edit),
                Some(DeckAction::Share),
                Some(DeckAction::Preview),
                Some(DeckAction::Delete),
            ],
        }
    }

    pub fn get(&self, slot: Slot) -> Option<DeckAction> {
        self.slots[slot.index()]
    }

    pub fn slot_of(&self, action: DeckAction) -> Option<Slot> {
        Slot::ALL.into_iter().find(|s| self.get(*s) == Some(action))
    }

    pub fn assignments(&self) -> impl Iterator<Item = (Slot, DeckAction)> + '_ {
        Slot::ALL
            .into_iter()
            .filter_map(|s| self.get(s).map(|a| (s, a)))
    }

    pub fn preset(&self) -> LayoutPreset {
        [
            LayoutPreset::Standard,
            LayoutPreset::Sharing,
            LayoutPreset::Minimal,
        ]
        .into_iter()
        .find(|p| p.layout().as_ref() == Some(self))
        .unwrap_or(LayoutPreset::Custom)
    }

    /// Moving an action swaps it with what was in the slot; Dismiss is never dropped, it moves to a free slot instead.
    pub fn assign(&mut self, action: Option<DeckAction>, slot: Slot) {
        let displaced = self.get(slot);
        if displaced == action {
            return;
        }
        if action.is_none() && displaced == Some(DeckAction::Dismiss) {
            return;
        }
        let from = action.and_then(|a| self.slot_of(a));
        self.slots[slot.index()] = action;
        match from {
            Some(from) => self.slots[from.index()] = displaced,
            None if displaced == Some(DeckAction::Dismiss) => {
                if let Some(free) = Slot::ALL.into_iter().find(|s| self.get(*s).is_none()) {
                    self.slots[free.index()] = displaced;
                }
            }
            None => {}
        }
    }

    /// A preset name or a `{slot: action}` object; anything unreadable falls back to Standard, and damage is repaired.
    pub fn parse(setting: &str) -> Self {
        let preset = [
            LayoutPreset::Standard,
            LayoutPreset::Sharing,
            LayoutPreset::Minimal,
        ]
        .into_iter()
        .find(|p| p.name() == setting);
        if let Some(layout) = preset.and_then(LayoutPreset::layout) {
            return layout;
        }
        let Ok(map) = serde_json::from_str::<BTreeMap<String, String>>(setting) else {
            return Self::standard();
        };
        let mut layout = ToolLayout { slots: [None; 8] };
        for slot in Slot::ALL {
            let action = map.get(slot.name()).and_then(|a| parse_action(a));
            if let Some(action) = action
                && layout.slot_of(action).is_none()
            {
                layout.slots[slot.index()] = Some(action);
            }
        }
        if layout.slot_of(DeckAction::Dismiss).is_none() {
            layout.assign(Some(DeckAction::Dismiss), Slot::TopRight);
        }
        layout
    }

    pub fn to_setting(&self) -> String {
        match self.preset() {
            LayoutPreset::Custom => {
                let map: BTreeMap<&str, &str> = self
                    .assignments()
                    .map(|(s, a)| (s.name(), action_name(a)))
                    .collect();
                serde_json::to_string(&map).unwrap_or_else(|_| "standard".into())
            }
            preset => preset.name().into(),
        }
    }
}

/// Where the deck's cards sit inside its panel, in the panel's logical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeckGeometry {
    pub card: Size,
    pub margin: f64,
    pub corner: Corner,
    pub count: usize,
}

impl DeckGeometry {
    pub fn new(card: Size, margin: f64, corner: Corner, count: usize) -> Self {
        DeckGeometry {
            card,
            margin: margin.clamp(0., MAX_MARGIN),
            corner,
            count,
        }
    }

    pub fn has_header(&self) -> bool {
        self.count > 1
    }

    pub fn panel(&self) -> Size {
        let base = |side: f64| side + self.margin + SHADOW_ROOM;
        let stacked = self.count.saturating_sub(1) as f64 * (self.card.height + SPACING);
        let header = if self.has_header() { HEADER } else { 0. };
        Size::new(
            base(self.card.width),
            base(self.card.height) + stacked + header,
        )
    }

    fn card_x(&self) -> f64 {
        match self.corner {
            Corner::BottomRight => SHADOW_ROOM,
            Corner::BottomLeft => self.margin,
        }
    }

    /// Where card `index` sits with the deck fanned out: 0 is the oldest, at the top; the newest sits nearest the corner.
    pub fn card(&self, index: usize) -> Rect {
        let header = if self.has_header() { HEADER } else { 0. };
        let y = SHADOW_ROOM + header + index as f64 * (self.card.height + SPACING);
        Rect::from_origin_size(Point::new(self.card_x(), y), self.card)
    }

    /// How many cards lie in front of card `index` when the deck is folded.
    pub fn depth(&self, index: usize) -> usize {
        self.count.saturating_sub(index + 1)
    }

    fn front(&self) -> Rect {
        self.card(self.count.saturating_sub(1))
    }

    /// Folded, the newest card stays where it is fanned and the older ones tuck behind it; past
    /// [`PEEKING`] they lie under the last one that shows.
    pub fn folded(&self, index: usize) -> Rect {
        let front = self.front();
        let depth = self.depth(index).min(PEEKING) as f64;
        let scale = 1. - PEEK_SHRINK * depth;
        let size = Size::new(self.card.width * scale, self.card.height * scale);
        let x = front.x0 + (self.card.width - size.width) / 2.;
        Rect::from_origin_size(Point::new(x, front.y0 - PEEK * depth), size)
    }

    /// Card `index` part of the way from folded (0) to fanned (1).
    pub fn at(&self, index: usize, fan: f64) -> Rect {
        let (a, b) = (self.folded(index), self.card(index));
        let mix = |from: f64, to: f64| from + (to - from) * fan;
        Rect::new(
            mix(a.x0, b.x0),
            mix(a.y0, b.y0),
            mix(a.x1, b.x1),
            mix(a.y1, b.y1),
        )
    }

    /// The count over the folded stack, on the front card's top corner away from the screen edge.
    pub fn badge(&self) -> Option<Rect> {
        self.has_header().then(|| {
            let front = self.front();
            let x = match self.corner {
                Corner::BottomRight => front.x0 + 2.,
                Corner::BottomLeft => front.x1 - 2.,
            };
            let y = front.y0 - PEEK * self.depth(0).min(PEEKING) as f64 + 2.;
            Rect::from_center_size(Point::new(x, y), Size::new(BADGE, BADGE))
        })
    }

    pub fn header(&self) -> Option<Rect> {
        self.has_header().then(|| {
            Rect::from_origin_size(
                Point::new(self.card_x(), SHADOW_ROOM),
                Size::new(self.card.width, HEADER - SPACING / 2.),
            )
        })
    }

    /// Only these take the pointer; the rest of the panel lets clicks through. Fanned, the gaps
    /// between cards take it too, so crossing one does not fold the deck.
    pub fn input_region(&self, fanned: bool) -> Vec<Rect> {
        if self.count == 0 {
            return Vec::new();
        }
        if fanned {
            let top = self.header().unwrap_or_else(|| self.card(0));
            return vec![top.union(self.front())];
        }
        let stack = (0..self.count).fold(self.front(), |r, i| r.union(self.folded(i)));
        std::iter::once(stack).chain(self.badge()).collect()
    }

    /// The panel's top-left, in global logical pixels, flush with the output's corner.
    pub fn origin(&self, output: Rect) -> Point {
        let panel = self.panel();
        let x = match self.corner {
            Corner::BottomRight => output.x1 - panel.width,
            Corner::BottomLeft => output.x0,
        };
        Point::new(x, output.y1 - panel.height)
    }
}
