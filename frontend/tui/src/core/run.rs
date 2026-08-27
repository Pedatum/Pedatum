//! Play mode: ticks a game module against a cloned scene, so the editor's copy
//! is untouched.
//!
//! This module holds no game rule and no tuning constant. Gravity, jumping,
//! scrolling, collision response and dialogue flow all live in the game's
//! `.hom` systems, compiled by `shinra build` into the module loaded here.
//! Without a module, ticking advances the clock and nothing else.

use std::path::Path;

use scene::Scene;
use shinra_engine::registry::GameModule;

/// Clamp a frame's delta so a stalled draw cannot let a system integrate across
/// a huge step. A scheduler concern, not a game one.
const MAX_DT: f32 = 0.1;

pub struct RunState {
    /// The live scene the viewport renders.
    pub scene: Scene,
    /// The scene as loaded, for restarting without a module.
    initial: Scene,
    pub elapsed: f32,
    module: Option<GameModule>,
    /// Key codes seen since the last tick. A terminal sends no key-up, so each
    /// press counts for exactly one tick.
    keys: Vec<u32>,
}

impl RunState {
    pub fn new(scene: Scene) -> Self {
        Self {
            initial: scene.clone(),
            scene,
            elapsed: 0.0,
            module: None,
            keys: Vec::new(),
        }
    }

    /// Load the game's compiled module and hand it the scene to run.
    pub fn with_module(scene: Scene, module_path: &Path) -> Self {
        let mut state = Self::new(scene);
        let ron = ron::to_string(&state.scene).unwrap_or_default();
        match GameModule::load(module_path, &ron) {
            Ok(m) => state.module = Some(m),
            Err(e) => eprintln!("[ide] {}: {e:#}", module_path.display()),
        }
        state
    }

    /// True once a game module is driving this run.
    pub fn has_module(&self) -> bool {
        self.module.is_some()
    }

    /// Record a key for the next tick. Its meaning is the game's business.
    pub fn key(&mut self, code: u32) {
        if !self.keys.contains(&code) {
            self.keys.push(code);
        }
    }

    pub fn restart(&mut self) {
        match &self.module {
            Some(m) => {
                m.restart();
                if let Ok(s) = m.scene() {
                    self.scene = s;
                }
            }
            None => self.scene = self.initial.clone(),
        }
        self.elapsed = 0.0;
    }

    pub fn tick(&mut self, dt: f32) {
        let dt = dt.clamp(0.0, MAX_DT);
        self.elapsed += dt;

        let Some(module) = &self.module else {
            self.keys.clear();
            return;
        };
        let keys = std::mem::take(&mut self.keys);
        if let Err(e) = module.tick(dt, &keys, &keys) {
            eprintln!("[ide] game_tick: {e:#}");
            return;
        }
        match module.scene() {
            Ok(s) => self.scene = s,
            Err(e) => eprintln!("[ide] game_scene_ron: {e:#}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_state() -> RunState {
        RunState::new(Scene {
            name: "t".into(),
            nodes: vec![],
        })
    }

    #[test]
    fn elapsed_accumulates() {
        let mut r = run_state();
        r.tick(0.05);
        r.tick(0.05);
        assert!((r.elapsed - 0.1).abs() < 1e-6);
    }

    #[test]
    fn dt_is_clamped() {
        let mut r = run_state();
        r.tick(10.0);
        assert!((r.elapsed - MAX_DT).abs() < 1e-6);
    }

    #[test]
    fn negative_dt_does_not_rewind() {
        let mut r = run_state();
        r.tick(-1.0);
        assert_eq!(r.elapsed, 0.0);
    }

    #[test]
    fn restart_resets_the_clock() {
        let mut r = run_state();
        r.tick(0.05);
        r.restart();
        assert_eq!(r.elapsed, 0.0);
    }

    #[test]
    fn without_a_module_no_keys_accumulate_across_ticks() {
        let mut r = run_state();
        r.key(32);
        r.tick(0.016);
        assert!(!r.has_module());
    }

    #[test]
    fn a_key_is_recorded_once_per_tick() {
        let mut r = run_state();
        r.key(32);
        r.key(32);
        assert_eq!(r.keys.len(), 1);
    }
}
