// The C ABI between the host and every game module. Only #[repr(C)] POD types
// and UTF-8 bytes cross the boundary.

// ============================================================
// Game module ABI (v2)
// ============================================================
//
// A game builds as a cdylib — game1.so, game2.so, … — and owns its own World
// and systems. The host hands it the scene and raw keys; it hands back the
// mutated scene for rendering. Nothing but #[repr(C)] POD and UTF-8 bytes
// crosses the boundary.
//
//   extern "C" fn game_init(scene_ron: *const u8, len: u32) -> u32;
//   extern "C" fn game_tick(dt: f32, keys: *const KeyFrame) -> u32;
//   extern "C" fn game_scene_ron(out_len: *mut u32) -> *const u8;
//   extern "C" fn game_restart() -> u32;
//
// All return 0 on success. Pointers handed back stay valid until the next call
// on the same module.
//
// A module is single-instance per process: dlopen of the same file returns the
// same handle and therefore the same state. Hosts must serialize calls to one
// module, and must copy out of `game_scene_ron` before calling anything else.

/// Raw keyboard state for one tick. The game resolves these against its own
/// action map; the host binds no key to any meaning.
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct KeyFrame {
    /// Key codes held this tick.
    pub held: *const u32,
    pub held_len: u32,
    /// Key codes that went down this tick only.
    pub pressed: *const u32,
    pub pressed_len: u32,
}

/// Key codes shared by host and game. Printable keys use their ASCII value.
pub mod keys {
    pub const SPACE: u32 = 32;
    pub const LEFT: u32 = 0x1_0000;
    pub const RIGHT: u32 = 0x1_0001;
    pub const UP: u32 = 0x1_0002;
    pub const DOWN: u32 = 0x1_0003;
    pub const ENTER: u32 = 0x1_0004;

    /// Resolve the name used in a game's `input.tres.ron` to a code.
    pub fn code(name: &str) -> Option<u32> {
        Some(match name {
            "Space" => SPACE,
            "Left" => LEFT,
            "Right" => RIGHT,
            "Up" => UP,
            "Down" => DOWN,
            "Enter" => ENTER,
            s if s.len() == 1 => s.chars().next()?.to_ascii_lowercase() as u32,
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::keys;

    #[test]
    fn named_keys_resolve() {
        assert_eq!(keys::code("Space"), Some(keys::SPACE));
        assert_eq!(keys::code("Left"), Some(keys::LEFT));
    }

    #[test]
    fn single_letters_are_ascii_lowercase() {
        assert_eq!(keys::code("A"), Some('a' as u32));
        assert_eq!(keys::code("d"), Some('d' as u32));
    }

    #[test]
    fn unknown_key_is_none() {
        assert_eq!(keys::code("Hyperspace"), None);
    }
}
