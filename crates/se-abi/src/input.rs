//! The one resource `game.so` writes and every stage only reads.

use crate::prim::Str;

pub const KEY_COUNT: usize = 256;

/// Key codes. Printable ASCII maps to itself, so `b'M' as usize` indexes the
/// M key and the named constants cover the rest.
pub mod key {
    pub const ESC: u8 = 27;
    pub const SPACE: u8 = 32;
    pub const ENTER: u8 = 13;
    pub const TAB: u8 = 9;
    pub const BACKSPACE: u8 = 8;
    pub const LEFT: u8 = 1;
    pub const RIGHT: u8 = 2;
    pub const UP: u8 = 3;
    pub const DOWN: u8 = 4;
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Input {
    /// 1 while held.
    pub down: [u8; KEY_COUNT],
    /// 1 only on the frame the key went down.
    pub pressed: [u8; KEY_COUNT],
    /// 1 only on the frame the key came up.
    pub released: [u8; KEY_COUNT],
    pub mouse: [f32; 2],
    pub mouse_down: u8,
    /// GameTok: +1 swiped to the next game, -1 to the previous, 0 otherwise.
    pub swipe: i8,
    pub _pad: [u8; 2],
}

impl Input {
    pub const NAME: &'static str = "Input";

    pub const fn zeroed() -> Input {
        Input {
            down: [0; KEY_COUNT],
            pressed: [0; KEY_COUNT],
            released: [0; KEY_COUNT],
            mouse: [0.0, 0.0],
            mouse_down: 0,
            swipe: 0,
            _pad: [0; 2],
        }
    }

    pub fn is_down(&self, k: u8) -> bool {
        self.down[k as usize] != 0
    }
    pub fn just_pressed(&self, k: u8) -> bool {
        self.pressed[k as usize] != 0
    }
    pub fn just_released(&self, k: u8) -> bool {
        self.released[k as usize] != 0
    }
}

/// What `game.so` is told at the top of every tick. Time lives here and
/// nowhere else — a stage that wants it takes `dt`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Frame {
    /// Seconds since the bundle was loaded.
    pub t: f64,
    /// Seconds since the previous tick.
    pub dt: f32,
    pub index: u64,
    pub input: *const Input,
    /// Presentation size in pixels.
    pub width: u32,
    pub height: u32,
}

impl Frame {
    /// # Safety
    /// Only valid inside the tick the host handed this to.
    pub unsafe fn input(&self) -> &Input {
        &*self.input
    }
}

/// A slot is a module position the control layer may repoint at runtime —
/// this is how `game.so` decides which `render.so` or `asset.so` is live.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    Asset = 0,
    Process = 1,
    Render = 2,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SlotBind {
    pub slot: Slot,
    /// Module name within its category, e.g. `"graph1"`.
    pub module: Str,
}

impl SlotBind {
    pub const fn new(slot: Slot, module: &'static str) -> SlotBind {
        SlotBind { slot, module: Str::new(module) }
    }
}
