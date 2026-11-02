mod curved;
mod heads;
mod shared;
mod straight;
#[cfg(test)]
mod tests;
mod types;

pub use heads::{HeadPath, HeadPoints};
pub use types::*;

pub fn resolve(input: &ArrowInput) -> ArrowInfo {
    if is_straight(input.bend) {
        straight::resolve(input)
    } else {
        curved::resolve(input)
    }
}
