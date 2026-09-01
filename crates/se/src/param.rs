//! How a stage parameter turns into a slot in the call frame.
//!
//! There is deliberately no blanket impl: a type is a component because
//! `#[derive(Schema)]` said so, a resource because the engine says so. A stage
//! cannot invent a third category.

use se_abi::{Column, Param, StageCall};

/// # Safety
/// `fetch` must return a pointer valid for `Self` for the whole call, and
/// `PARAM` must describe the same type it returns.
pub unsafe trait StageParam: Sized {
    const PARAM: Param;

    /// `slot` is the index of this parameter within its own kind; `row` is the
    /// match being visited.
    ///
    /// # Safety
    /// `call` must be the frame the host passed to this stage.
    unsafe fn fetch(call: *const StageCall, slot: u32, row: u32) -> *mut u8;
}

/// # Safety
/// `call` must be live and `slot` a valid column index.
pub unsafe fn fetch_component(call: *const StageCall, slot: u32, row: u32) -> *mut u8 {
    let c = &*call;
    let col: &Column = &*c.cols.add(slot as usize);
    let idx = *c.rows.add(row as usize * c.n_cols as usize + slot as usize);
    col.base.add(idx as usize * col.stride as usize)
}

/// # Safety
/// `call` must be live and `slot` a valid resource index.
pub unsafe fn fetch_resource(call: *const StageCall, slot: u32, _row: u32) -> *mut u8 {
    *(*call).res.add(slot as usize) as *mut u8
}

/// # Safety
/// `call` must be live.
pub unsafe fn fetch_dt(call: *const StageCall, _slot: u32, _row: u32) -> *mut u8 {
    &(*call).dt as *const f32 as *mut u8
}

/// The bare `f32` tail of a signature is dt, and dt is the only time a stage
/// ever sees. `game.so` keeps the clock.
unsafe impl StageParam for f32 {
    const PARAM: Param = Param::dt();
    unsafe fn fetch(call: *const StageCall, slot: u32, row: u32) -> *mut u8 {
        fetch_dt(call, slot, row)
    }
}

unsafe impl StageParam for se_abi::Input {
    const PARAM: Param =
        Param::resource(se_abi::Input::NAME, core::mem::size_of::<se_abi::Input>() as u32);
    unsafe fn fetch(call: *const StageCall, slot: u32, row: u32) -> *mut u8 {
        fetch_resource(call, slot, row)
    }
}
