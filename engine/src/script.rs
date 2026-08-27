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
    /// Axes resolved from the game's action map, each in -1.0..=1.0.
    pub axes: std::collections::HashMap<String, f32>,
    /// Contacts found after the movement systems ran this tick.
    pub overlaps: Vec<HitRecord>,
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

/// An axis in -1.0..=1.0. Opposing keys cancel; an unmapped axis reads 0.
pub fn axis(name: &str) -> f32 {
    FRAME.with(|f| f.borrow().axes.get(name).copied().unwrap_or(0.0))
}

/// Resolve one axis from held key codes: +1 for a positive key, -1 for a
/// negative one, and 0 when both or neither are down.
pub fn resolve_axis(held: &[u32], neg: &[u32], pos: &[u32]) -> f32 {
    let down = |keys: &[u32]| keys.iter().any(|k| held.contains(k));
    (down(pos) as i32 - down(neg) as i32) as f32
}

/// One contact, with both sides named so a game can tell one object from
/// another.
#[derive(Clone, Debug, PartialEq)]
pub struct HitRecord {
    /// Component that matched the first side of the query.
    pub a_component: String,
    pub b_component: String,
    /// Node names, so a script can act on a specific object.
    pub a_node: String,
    pub b_node: String,
    /// Unit direction to move `a_node` off `b_node`.
    pub normal: [f32; 2],
    pub depth: f32,
}

/// Contacts between entities carrying the two named components, oriented so
/// `a` is the side the caller asked about first.
pub fn overlapping(a: &str, b: &str) -> Vec<HitRecord> {
    FRAME.with(|f| {
        f.borrow()
            .overlaps
            .iter()
            .filter_map(|h| {
                if h.a_component == a && h.b_component == b {
                    Some(h.clone())
                } else if h.a_component == b && h.b_component == a {
                    // Reported the other way round: flip it, normal included.
                    Some(HitRecord {
                        a_component: h.b_component.clone(),
                        b_component: h.a_component.clone(),
                        a_node: h.b_node.clone(),
                        b_node: h.a_node.clone(),
                        normal: [-h.normal[0], -h.normal[1]],
                        depth: h.depth,
                    })
                } else {
                    None
                }
            })
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

    fn record() -> HitRecord {
        HitRecord {
            a_component: "PlayerControlled".into(),
            b_component: "Obstacle".into(),
            a_node: "dino".into(),
            b_node: "tree2".into(),
            normal: [-1.0, 0.0],
            depth: 0.25,
        }
    }

    #[test]
    fn overlapping_matches_either_order() {
        begin_frame(Frame {
            overlaps: vec![record()],
            ..Default::default()
        });
        assert_eq!(overlapping("PlayerControlled", "Obstacle").len(), 1);
        assert_eq!(overlapping("Obstacle", "PlayerControlled").len(), 1);
        assert_eq!(overlapping("Obstacle", "Scenery").len(), 0);
        let _ = end_frame();
    }

    /// A hit names both nodes, so a game can act on the specific object.
    #[test]
    fn a_hit_names_the_nodes() {
        begin_frame(Frame {
            overlaps: vec![record()],
            ..Default::default()
        });
        let h = &overlapping("PlayerControlled", "Obstacle")[0];
        assert_eq!(h.a_node, "dino");
        assert_eq!(h.b_node, "tree2");
        let _ = end_frame();
    }

    /// Asking the other way round flips the sides and the normal, so the
    /// normal always moves the side the caller named first.
    #[test]
    fn querying_the_other_way_flips_sides_and_normal() {
        begin_frame(Frame {
            overlaps: vec![record()],
            ..Default::default()
        });
        let h = &overlapping("Obstacle", "PlayerControlled")[0];
        assert_eq!(h.a_node, "tree2");
        assert_eq!(h.b_node, "dino");
        assert_eq!(h.normal, [1.0, 0.0]);
        let _ = end_frame();
    }

    #[test]
    fn an_unmapped_axis_reads_zero() {
        begin_frame(Frame::default());
        assert_eq!(axis("move_x"), 0.0);
        let _ = end_frame();
    }

    #[test]
    fn a_mapped_axis_reads_its_value() {
        let mut axes = std::collections::HashMap::new();
        axes.insert("move_x".to_string(), -1.0);
        begin_frame(Frame {
            axes,
            ..Default::default()
        });
        assert_eq!(axis("move_x"), -1.0);
        let _ = end_frame();
    }

    #[test]
    fn resolve_axis_signs_each_direction() {
        assert_eq!(resolve_axis(&[100], &[100], &[200]), -1.0);
        assert_eq!(resolve_axis(&[200], &[100], &[200]), 1.0);
    }

    #[test]
    fn opposing_keys_cancel() {
        assert_eq!(resolve_axis(&[100, 200], &[100], &[200]), 0.0);
    }

    #[test]
    fn no_key_is_a_neutral_axis() {
        assert_eq!(resolve_axis(&[], &[100], &[200]), 0.0);
    }

    /// Several keys may drive one direction, so either satisfies it.
    #[test]
    fn any_key_in_a_direction_counts() {
        assert_eq!(resolve_axis(&[301], &[100, 101], &[300, 301]), 1.0);
    }

    #[test]
    fn end_frame_clears_context() {
        begin_frame(frame_with(&["jump"], &[]));
        let _ = end_frame();
        assert!(!action("jump"));
    }
}
