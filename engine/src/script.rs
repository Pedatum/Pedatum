//! The engine's script-facing API — the only surface a game's `.hom` systems
//! may touch.
//!
//! Systems are plain functions with no access to engine state, so per-frame
//! context (input, collision pairs) is published here before systems run and
//! cleared after. `engine/hom/engine.rs` is the shim `homunc` inlines so that
//! Homun's `use engine` resolves these names.

use std::cell::RefCell;
use std::collections::HashSet;

/// What the engine publishes for the current tick.
#[derive(Default)]
pub struct Frame {
    /// Actions held this tick, already resolved from the game's action map.
    pub held: HashSet<String>,
    /// Actions that went down this tick only.
    pub pressed: HashSet<String>,
    /// Collision pairs, as (component name, component name) that overlap.
    pub overlaps: Vec<(String, String)>,
    /// Set by `restart()`; the host checks and clears it after the tick.
    pub restart_requested: bool,
}

thread_local! {
    static FRAME: RefCell<Frame> = RefCell::new(Frame::default());
}

/// Publish this tick's context. Called by the host before running systems.
pub fn begin_frame(frame: Frame) {
    FRAME.with(|f| *f.borrow_mut() = frame);
}

/// Take the frame back after systems have run, to read `restart_requested`.
pub fn end_frame() -> Frame {
    FRAME.with(|f| std::mem::take(&mut *f.borrow_mut()))
}

// ---- names visible to `.hom` ----

/// True while the action is held.
pub fn action(name: &str) -> bool {
    FRAME.with(|f| f.borrow().held.contains(name))
}

/// True on the tick the action goes down.
pub fn action_pressed(name: &str) -> bool {
    FRAME.with(|f| f.borrow().pressed.contains(name))
}

/// Collider overlaps between entities carrying the two named components.
pub fn overlapping(a: &str, b: &str) -> Vec<(String, String)> {
    FRAME.with(|f| {
        f.borrow()
            .overlaps
            .iter()
            .filter(|(x, y)| (x == a && y == b) || (x == b && y == a))
            .cloned()
            .collect()
    })
}

/// Ask the host to return the run to its initial state.
pub fn restart() {
    FRAME.with(|f| f.borrow_mut().restart_requested = true);
}

pub mod math {
    pub fn cos(x: f32) -> f32 { x.cos() }
    pub fn sin(x: f32) -> f32 { x.sin() }
    pub fn sqrt(x: f32) -> f32 { x.sqrt() }
    pub fn floor(x: f32) -> f32 { x.floor() }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame_with(held: &[&str], pressed: &[&str]) -> Frame {
        Frame {
            held: held.iter().map(|s| s.to_string()).collect(),
            pressed: pressed.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn action_reads_held_not_pressed() {
        begin_frame(frame_with(&["jump"], &[]));
        assert!(action("jump"));
        assert!(!action_pressed("jump"));
        let _ = end_frame();
    }

    #[test]
    fn action_pressed_reads_pressed() {
        begin_frame(frame_with(&[], &["advance"]));
        assert!(action_pressed("advance"));
        assert!(!action("advance"));
        let _ = end_frame();
    }

    #[test]
    fn unknown_action_is_false() {
        begin_frame(Frame::default());
        assert!(!action("nope"));
        assert!(!action_pressed("nope"));
        let _ = end_frame();
    }

    #[test]
    fn restart_is_reported_once_then_cleared() {
        begin_frame(Frame::default());
        restart();
        assert!(end_frame().restart_requested);
        begin_frame(Frame::default());
        assert!(!end_frame().restart_requested);
    }

    #[test]
    fn overlapping_matches_either_order() {
        begin_frame(Frame {
            overlaps: vec![("PlayerControlled".into(), "Obstacle".into())],
            ..Default::default()
        });
        assert_eq!(overlapping("PlayerControlled", "Obstacle").len(), 1);
        assert_eq!(overlapping("Obstacle", "PlayerControlled").len(), 1);
        assert_eq!(overlapping("Obstacle", "Scenery").len(), 0);
        let _ = end_frame();
    }

    #[test]
    fn end_frame_clears_context() {
        begin_frame(frame_with(&["jump"], &[]));
        let _ = end_frame();
        assert!(!action("jump"));
    }
}
