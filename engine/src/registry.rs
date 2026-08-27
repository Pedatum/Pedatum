//! Host-side view of a loaded game module.
//!
//! A game builds as a cdylib and owns its own World, components and systems.
//! The host knows only the four ABI symbols; nothing about the game's types
//! crosses the boundary.

use std::path::Path;

use gametok_abi::{keys, KeyFrame};
use libloading::{Library, Symbol};

pub struct GameModule {
    _lib: Library,
    tick: unsafe extern "C" fn(f32, *const KeyFrame) -> u32,
    scene_ron: unsafe extern "C" fn(*mut u32) -> *const u8,
    restart: unsafe extern "C" fn() -> u32,
}

impl GameModule {
    /// Load a game cdylib and hand it the scene it should run.
    pub fn load(path: &Path, scene_ron: &str) -> anyhow::Result<Self> {
        unsafe {
            let lib = Library::new(path)?;
            let init: Symbol<unsafe extern "C" fn(*const u8, u32) -> u32> = lib.get(b"game_init")?;
            let tick: Symbol<unsafe extern "C" fn(f32, *const KeyFrame) -> u32> =
                lib.get(b"game_tick")?;
            let scene: Symbol<unsafe extern "C" fn(*mut u32) -> *const u8> =
                lib.get(b"game_scene_ron")?;
            let restart: Symbol<unsafe extern "C" fn() -> u32> = lib.get(b"game_restart")?;

            let code = init(scene_ron.as_ptr(), scene_ron.len() as u32);
            if code != 0 {
                anyhow::bail!("game_init failed with code {code} for {}", path.display());
            }

            let (tick, scene, restart) = (*tick, *scene, *restart);
            Ok(Self { _lib: lib, tick, scene_ron: scene, restart })
        }
    }

    pub fn tick(&self, dt: f32, held: &[u32], pressed: &[u32]) -> anyhow::Result<()> {
        let frame = KeyFrame {
            held: held.as_ptr(),
            held_len: held.len() as u32,
            pressed: pressed.as_ptr(),
            pressed_len: pressed.len() as u32,
        };
        let code = unsafe { (self.tick)(dt, &frame) };
        if code != 0 {
            anyhow::bail!("game_tick failed with code {code}");
        }
        Ok(())
    }

    /// The game's current scene, for the host to render.
    pub fn scene(&self) -> anyhow::Result<scene_format::Scene> {
        let mut len: u32 = 0;
        let ptr = unsafe { (self.scene_ron)(&mut len) };
        if ptr.is_null() {
            anyhow::bail!("game_scene_ron returned null");
        }
        let bytes = unsafe { std::slice::from_raw_parts(ptr, len as usize) };
        Ok(ron::from_str(std::str::from_utf8(bytes)?)?)
    }

    pub fn restart(&self) {
        unsafe { (self.restart)() };
    }
}

/// Map a key name from a game's `input.tres.ron` to the shared code.
pub fn key_code(name: &str) -> Option<u32> {
    keys::code(name)
}
