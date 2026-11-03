use es_app::deck::DeckAction;
use es_geom::{Point, Rect, Size};
use std::collections::BTreeMap;

pub const SPACING: f64 = 10.;
pub const HEADER: f64 = 26.;
/// Room on the far sides of the cards for their shadow.
pub const SHADOW_ROOM: f64 = 24.;
pub const DEFAULT_MARGIN: f64 = 20.;
pub const MAX_MARGIN: f64 = 48.;

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
}

impl Slot {
    pub const ALL: [Slot; 6] = [
        Slot::TopLeft,
        Slot::TopRight,
        Slot::CenterLeft,
        Slot::CenterRight,
        Slot::BottomLeft,
        Slot::BottomRight,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Slot::TopLeft => "top-left",
            Slot::TopRight => "top-right",
            Slot::CenterLeft => "center-left",
            Slot::CenterRight => "center-right",
            Slot::BottomLeft => "bottom-left",
            Slot::BottomRight => "bottom-right",
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
            ],
            LayoutPreset::Sharing => [
                Some(A::Pin),
                Some(A::Dismiss),
                Some(A::Copy),
                Some(A::Share),
                Some(A::Edit),
                Some(A::Save),
            ],
            LayoutPreset::Minimal => [
                None,
                Some(A::Dismiss),
                Some(A::Copy),
                Some(A::Save),
                None,
                None,
            ],
            LayoutPreset::Custom => return None,
        };
        Some(ToolLayout { slots })
    }
}

/// Which action sits in each of a card's six hover slots; each at most once, and Dismiss always.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolLayout {
    slots: [Option<DeckAction>; 6],
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
        let mut layout = ToolLayout { slots: [None; 6] };
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

    /// `index` 0 is the oldest card, at the top; the newest sits nearest the corner.
    pub fn card(&self, index: usize) -> Rect {
        let header = if self.has_header() { HEADER } else { 0. };
        let y = SHADOW_ROOM + header + index as f64 * (self.card.height + SPACING);
        Rect::from_origin_size(Point::new(self.card_x(), y), self.card)
    }

    pub fn header(&self) -> Option<Rect> {
        self.has_header().then(|| {
            Rect::from_origin_size(
                Point::new(self.card_x(), SHADOW_ROOM),
                Size::new(self.card.width, HEADER - SPACING / 2.),
            )
        })
    }

    /// Only these take the pointer; the rest of the panel lets clicks through.
    pub fn input_region(&self) -> Vec<Rect> {
        self.header()
            .into_iter()
            .chain((0..self.count).map(|i| self.card(i)))
            .collect()
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
