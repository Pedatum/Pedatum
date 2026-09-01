//! The Shinra Engine ABI: everything that crosses a `.so` boundary.
//!
//! This crate is the bundle contract in code form. It has no dependencies and
//! allocates nothing, because both sides of every call in here may have been
//! produced by a different compilation — the host, and a module the host had
//! never heard of when it was built.
//!
//! Six entry points, one per module kind:
//!
//! ```text
//! bundle.so     se_register_layouts()    name -> layout
//!               se_register_buffers()    name -> shape, count, sampled
//! asset/*.so    se_register_assets()     name -> bytes
//! process/*.so  se_register_stages()     spec + function
//! render/*.so   se_register_graph()      nodes + edges
//! game/*.so     se_register_control()    + tick
//! ```
//!
//! Every module additionally exports `se_abi_version`, which the host checks
//! before it trusts anything else in the image.

#![no_std]

pub mod buffer;
pub mod control;
pub mod data;
pub mod graph;
pub mod input;
pub mod prim;
pub mod stage;

pub use buffer::{AssetDesc, AssetSink, BufferDesc, BufferSink, Extent, Format};
pub use control::{ControlSink, ControlSpec, Ctl, Entity, NO_ENTITY};
pub use data::{hash_begin, hash_field, Field, Layout, LayoutSink, ScalarTy, Schema};
pub use graph::{Draw, DrawKind, Edge, GraphDesc, GraphSink, Pass};
pub use input::{key, Frame, Input, Slot, SlotBind, KEY_COUNT};
pub use prim::{AbiVersion, Slice, Str, ABI_MAJOR, ABI_MINOR};
pub use stage::{slot_of, Access, Column, Param, ParamKind, StageCall, StageSink, StageSpec};

/// Symbol names the host looks up. Kept here so host and module cannot drift.
pub mod sym {
    pub const ABI_VERSION: &[u8] = b"se_abi_version";
    pub const LAYOUTS: &[u8] = b"se_register_layouts";
    pub const BUFFERS: &[u8] = b"se_register_buffers";
    pub const ASSETS: &[u8] = b"se_register_assets";
    pub const STAGES: &[u8] = b"se_register_stages";
    pub const GRAPH: &[u8] = b"se_register_graph";
    pub const CONTROL: &[u8] = b"se_register_control";
}
