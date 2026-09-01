//! `game/*.so`: the only station that knows the time.
//!
//! It owns time, input, and which module fills which slot. Its write surface
//! is deliberately two things wide — the input components, and spawn/despawn
//! reconciling what should exist against what does. Everything else it can
//! only read.

use crate::input::{Frame, Slot, SlotBind};
use crate::prim::{Slice, Str};

pub type Entity = u64;
pub const NO_ENTITY: Entity = 0;

/// The host, as seen from inside a tick. A vtable rather than a trait object
/// so the shape survives being compiled by a different rustc.
#[repr(C)]
pub struct Ctl {
    pub ctx: *mut core::ffi::c_void,

    pub spawn: unsafe extern "C" fn(*mut core::ffi::c_void) -> Entity,
    pub despawn: unsafe extern "C" fn(*mut core::ffi::c_void, Entity),
    pub alive: unsafe extern "C" fn(*mut core::ffi::c_void, Entity) -> bool,

    /// Write a component. Refused if size disagrees with the bundle layout.
    pub set: unsafe extern "C" fn(*mut core::ffi::c_void, Entity, Str, *const u8, u32) -> bool,
    /// Read a component into caller memory.
    pub get: unsafe extern "C" fn(*mut core::ffi::c_void, Entity, Str, *mut u8, u32) -> bool,
    pub remove: unsafe extern "C" fn(*mut core::ffi::c_void, Entity, Str) -> bool,

    /// Every entity carrying a component, written into `out`; returns the true
    /// count, which may exceed `cap`.
    pub query: unsafe extern "C" fn(*mut core::ffi::c_void, Str, *mut Entity, u32) -> u32,

    /// Content, read-only. Definitions live here; spawning from them is the
    /// control layer's job, never the asset's.
    pub asset: unsafe extern "C" fn(*mut core::ffi::c_void, Str, *mut Slice<u8>) -> bool,
    /// Names of every loaded asset, for reconciling a roster against content.
    pub asset_names: unsafe extern "C" fn(*mut core::ffi::c_void, *mut Str, u32) -> u32,

    /// Repoint a slot. Takes effect at the next frame boundary.
    pub set_slot: unsafe extern "C" fn(*mut core::ffi::c_void, Slot, Str),

    /// Status, not roster: what happened, so it can happen again.
    pub save: unsafe extern "C" fn(*mut core::ffi::c_void, Str) -> bool,
    pub load: unsafe extern "C" fn(*mut core::ffi::c_void, Str) -> bool,

    /// GameTok: hand the deck to the next or previous bundle.
    pub swipe: unsafe extern "C" fn(*mut core::ffi::c_void, i32),
    pub quit: unsafe extern "C" fn(*mut core::ffi::c_void),
    pub log: unsafe extern "C" fn(*mut core::ffi::c_void, Str),
}

/// Ergonomic wrappers. Every one is a straight vtable call.
impl Ctl {
    pub fn spawn(&mut self) -> Entity {
        unsafe { (self.spawn)(self.ctx) }
    }
    pub fn despawn(&mut self, e: Entity) {
        unsafe { (self.despawn)(self.ctx, e) }
    }
    pub fn alive(&mut self, e: Entity) -> bool {
        unsafe { (self.alive)(self.ctx, e) }
    }
    pub fn set<T: Copy>(&mut self, e: Entity, name: &'static str, v: &T) -> bool {
        unsafe {
            (self.set)(
                self.ctx,
                e,
                Str::new(name),
                v as *const T as *const u8,
                core::mem::size_of::<T>() as u32,
            )
        }
    }
    pub fn get<T: Copy>(&mut self, e: Entity, name: &'static str) -> Option<T> {
        let mut v = core::mem::MaybeUninit::<T>::uninit();
        let ok = unsafe {
            (self.get)(
                self.ctx,
                e,
                Str::new(name),
                v.as_mut_ptr() as *mut u8,
                core::mem::size_of::<T>() as u32,
            )
        };
        if ok {
            Some(unsafe { v.assume_init() })
        } else {
            None
        }
    }
    pub fn remove(&mut self, e: Entity, name: &'static str) -> bool {
        unsafe { (self.remove)(self.ctx, e, Str::new(name)) }
    }
    pub fn query(&mut self, name: &'static str, out: &mut [Entity]) -> u32 {
        unsafe { (self.query)(self.ctx, Str::new(name), out.as_mut_ptr(), out.len() as u32) }
    }
    pub fn asset(&mut self, name: &str) -> Option<&'static [u8]> {
        let mut s = Slice::<u8>::empty();
        let name = Str { ptr: name.as_ptr(), len: name.len() };
        if unsafe { (self.asset)(self.ctx, name, &mut s) } {
            Some(unsafe { s.as_slice() })
        } else {
            None
        }
    }
    pub fn set_slot(&mut self, slot: Slot, module: &'static str) {
        unsafe { (self.set_slot)(self.ctx, slot, Str::new(module)) }
    }
    pub fn swipe(&mut self, dir: i32) {
        unsafe { (self.swipe)(self.ctx, dir) }
    }
    pub fn quit(&mut self) {
        unsafe { (self.quit)(self.ctx) }
    }
    pub fn log(&mut self, msg: &str) {
        unsafe { (self.log)(self.ctx, Str { ptr: msg.as_ptr(), len: msg.len() }) }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct ControlSpec {
    pub name: Str,
    /// Which module fills which slot when this control takes over.
    pub slots: Slice<SlotBind>,
    /// Called once before the first tick, after slots are bound.
    pub start: Option<unsafe extern "C" fn(*mut Ctl)>,
    pub tick: unsafe extern "C" fn(*mut Ctl, *const Frame),
    /// Called before this control is unloaded, including on hot swap.
    pub stop: Option<unsafe extern "C" fn(*mut Ctl)>,
}

#[repr(C)]
pub struct ControlSink {
    pub ctx: *mut core::ffi::c_void,
    pub push: unsafe extern "C" fn(*mut core::ffi::c_void, *const ControlSpec),
}

impl ControlSink {
    /// # Safety
    /// `self` must be the sink the host just handed to `se_register_control`.
    pub unsafe fn push(&mut self, c: &ControlSpec) {
        (self.push)(self.ctx, c as *const ControlSpec)
    }
}
