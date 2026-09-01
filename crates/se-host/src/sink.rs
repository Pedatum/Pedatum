//! The host end of the six sinks.
//!
//! A module never allocates for the host and the host never parses the
//! module's memory layout — registration is a callback per item, and each
//! callback deep-copies. By the time an entry point returns, the host owes the
//! image nothing but its function pointers.

use crate::loader::Module;
use crate::registry::{AssetDef, BufferDef, ControlDef, GraphDef, LayoutDef, StageDef};
use anyhow::Result;
use core::ffi::c_void;
use libloading::Symbol;
use se_abi::{
    sym, AssetDesc, AssetSink, BufferDesc, BufferSink, ControlSink, ControlSpec, GraphDesc,
    GraphSink, Layout, LayoutSink, StageSink, StageSpec,
};

macro_rules! sink {
    ($fname:ident, $push:ident, $sink:ident, $abi:ident, $def:ident, $symbol:expr) => {
        unsafe extern "C" fn $push(ctx: *mut c_void, item: *const $abi) {
            let v = &mut *(ctx as *mut Vec<$def>);
            v.push($def::from_abi(&*item));
        }

        /// # Safety
        /// `m` must stay loaded for as long as the result is used.
        pub unsafe fn $fname(m: &Module) -> Result<Vec<$def>> {
            let f: Symbol<unsafe extern "C" fn(*mut $sink)> = m.sym($symbol)?;
            let mut out: Vec<$def> = Vec::new();
            let mut s = $sink {
                ctx: &mut out as *mut Vec<$def> as *mut c_void,
                push: $push,
            };
            f(&mut s);
            Ok(out)
        }
    };
}

sink!(layouts, push_layout, LayoutSink, Layout, LayoutDef, sym::LAYOUTS);
sink!(buffers, push_buffer, BufferSink, BufferDesc, BufferDef, sym::BUFFERS);
sink!(assets, push_asset, AssetSink, AssetDesc, AssetDef, sym::ASSETS);
sink!(stages, push_stage, StageSink, StageSpec, StageDef, sym::STAGES);
sink!(graphs, push_graph, GraphSink, GraphDesc, GraphDef, sym::GRAPH);
sink!(controls, push_control, ControlSink, ControlSpec, ControlDef, sym::CONTROL);
