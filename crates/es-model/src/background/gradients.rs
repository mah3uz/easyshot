use super::{BackgroundGradient, UnitPoint};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const MIN_GRADIENT_STOPS: usize = 2;
pub const MAX_GRADIENT_STOPS: usize = 4;
pub const MAX_GRADIENT_NAME_CHARS: usize = 48;
const USER_PREFIX: &str = "user-";

/// The gradients the user made; the built-in ones are fixed and live in the renderer.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GradientLibrary {
    pub gradients: Vec<BackgroundGradient>,
}

impl BackgroundGradient {
    pub fn is_user(&self) -> bool {
        self.id.starts_with(USER_PREFIX)
    }

    /// Clockwise from pointing up, so 90 runs left to right and 180 top to bottom.
    pub fn angle_degrees(&self) -> f64 {
        let (dx, dy) = (self.end.x - self.start.x, self.end.y - self.start.y);
        if dx == 0.0 && dy == 0.0 {
            return 0.0;
        }
        dx.atan2(-dy).to_degrees().rem_euclid(360.0)
    }

    /// The axis runs corner to corner at the diagonals, so no corner is left past the last colour.
    pub fn set_angle_degrees(&mut self, degrees: f64) {
        let (sin, cos) = degrees.to_radians().sin_cos();
        let half = 0.5 * (sin.abs() + cos.abs());
        let (dx, dy) = (sin * half, -cos * half);
        let snap = |v: f64| (v * 1e9).round() / 1e9;
        self.start = UnitPoint {
            x: snap(0.5 - dx),
            y: snap(0.5 - dy),
        };
        self.end = UnitPoint {
            x: snap(0.5 + dx),
            y: snap(0.5 + dy),
        };
    }

    /// Two to four colours; extra ones are dropped and a lone colour is repeated.
    pub fn clamp_stops(&mut self) {
        self.colors.truncate(MAX_GRADIENT_STOPS);
        while self.colors.len() < MIN_GRADIENT_STOPS {
            let last = self.colors.last().cloned().unwrap_or_default();
            self.colors.push(last);
        }
    }
}

/// Whitespace collapsed and at most 48 characters; `None` when nothing is left.
pub fn clean_gradient_name(raw: &str) -> Option<String> {
    let collapsed = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    let cut: String = collapsed.chars().take(MAX_GRADIENT_NAME_CHARS).collect();
    let name = cut.trim_end();
    (!name.is_empty()).then(|| name.to_owned())
}

impl GradientLibrary {
    pub fn get(&self, id: &str) -> Option<&BackgroundGradient> {
        self.gradients.iter().find(|g| g.id == id)
    }

    /// Replaces the user gradient with the same id, or adds it under a new id, so saving a copy
    /// of a built-in never touches the built-in.
    pub fn save(&mut self, mut gradient: BackgroundGradient) -> String {
        gradient.clamp_stops();
        if let Some(name) = clean_gradient_name(&gradient.title) {
            gradient.title = name;
        }
        if let Some(slot) = self.gradients.iter_mut().find(|g| g.id == gradient.id) {
            *slot = gradient;
            return slot.id.clone();
        }
        gradient.id = format!("{USER_PREFIX}{}", Uuid::new_v4());
        let id = gradient.id.clone();
        self.gradients.push(gradient);
        id
    }

    /// An empty name keeps the old one.
    pub fn rename(&mut self, id: &str, raw: &str) -> bool {
        let Some(name) = clean_gradient_name(raw) else {
            return false;
        };
        match self.gradients.iter_mut().find(|g| g.id == id) {
            Some(g) => {
                g.title = name;
                true
            }
            None => false,
        }
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let before = self.gradients.len();
        self.gradients.retain(|g| g.id != id);
        self.gradients.len() != before
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::background::BackgroundColor;

    fn two_stop(id: &str) -> BackgroundGradient {
        BackgroundGradient {
            id: id.into(),
            title: "Violet".into(),
            colors: vec![
                BackgroundColor::custom(0.5, 0.1, 0.9),
                BackgroundColor::custom(0.3, 0.0, 0.9),
            ],
            ..BackgroundGradient::default()
        }
    }

    #[test]
    fn saving_a_built_in_adds_a_user_copy_under_a_new_id() {
        let mut lib = GradientLibrary::default();
        let id = lib.save(two_stop("violet"));
        assert_ne!(id, "violet");
        assert!(lib.get(&id).is_some_and(BackgroundGradient::is_user));
        assert!(
            lib.get("violet").is_none(),
            "the built-in id is never stored"
        );
    }

    #[test]
    fn saving_a_user_gradient_replaces_it_in_place() {
        let mut lib = GradientLibrary::default();
        let first = lib.save(two_stop("violet"));
        lib.save(two_stop("jade"));
        let mut edited = lib.get(&first).cloned().unwrap();
        edited.colors.push(BackgroundColor::custom(1.0, 1.0, 1.0));
        assert_eq!(lib.save(edited), first);
        assert_eq!(lib.gradients.len(), 2);
        assert_eq!(lib.gradients[0].colors.len(), 3);
    }

    #[test]
    fn stops_are_kept_between_two_and_four() {
        let mut lib = GradientLibrary::default();
        let mut many = two_stop("x");
        many.colors = vec![BackgroundColor::custom(0.0, 0.0, 0.0); 6];
        let id = lib.save(many);
        assert_eq!(lib.get(&id).unwrap().colors.len(), 4);
        let mut one = two_stop("y");
        one.colors.truncate(1);
        let id = lib.save(one);
        assert_eq!(lib.get(&id).unwrap().colors.len(), 2);
    }

    #[test]
    fn rename_collapses_caps_and_ignores_an_empty_name() {
        let mut lib = GradientLibrary::default();
        let id = lib.save(two_stop("violet"));
        assert!(lib.rename(&id, "  deep \t sea  "));
        assert_eq!(lib.get(&id).unwrap().title, "deep sea");
        assert!(!lib.rename(&id, "   "));
        assert_eq!(lib.get(&id).unwrap().title, "deep sea");
        assert!(lib.rename(&id, &"z".repeat(80)));
        assert_eq!(lib.get(&id).unwrap().title.chars().count(), 48);
        assert!(!lib.rename("missing", "x"));
    }

    #[test]
    fn remove_takes_out_only_that_gradient() {
        let mut lib = GradientLibrary::default();
        let a = lib.save(two_stop("a"));
        let b = lib.save(two_stop("b"));
        assert!(lib.remove(&a));
        assert!(!lib.remove(&a));
        assert_eq!(lib.gradients.len(), 1);
        assert_eq!(lib.gradients[0].id, b);
    }

    #[test]
    fn angles_map_to_the_built_in_axes() {
        let point = |x: f64, y: f64| UnitPoint { x, y };
        let mut g = two_stop("a");
        for (degrees, start, end) in [
            (180.0, point(0.5, 0.0), point(0.5, 1.0)),
            (90.0, point(0.0, 0.5), point(1.0, 0.5)),
            (135.0, point(0.0, 0.0), point(1.0, 1.0)),
            (45.0, point(0.0, 1.0), point(1.0, 0.0)),
        ] {
            g.set_angle_degrees(degrees);
            assert_eq!((g.start, g.end), (start, end), "{degrees} degrees");
            assert!((g.angle_degrees() - degrees).abs() < 1e-6);
        }
        g.set_angle_degrees(300.0);
        assert!((g.angle_degrees() - 300.0).abs() < 1e-6);
    }
}
