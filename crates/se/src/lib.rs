//! What a Shinra module is written against.
//!
//! A module is a single `.rs` file at depth 1 of a bundle. It depends on this
//! crate, declares itself with one of the entry macros, and exports nothing
//! else. It never links the host, never sees the world, and can be rebuilt and
//! swapped underneath a running game.
//!
//! ```ignore
//! // bundle/data.rs
//! #[repr(C)]
//! #[derive(Clone, Copy, se::Schema)]
//! pub struct Transform { pub pos: [f32; 3], pub rot: [f32; 4], pub scale: [f32; 3] }
//!
//! se::layouts!(Transform);
//! ```
//!
//! ```ignore
//! // process/motion.rs
//! #[se::stage]
//! fn motion(t: &mut Transform, b: &Body, dt: f32) { /* ... */ }
//!
//! se::stages!(motion);
//! ```

mod build_graph;
mod entry;
mod param;

pub use build_graph::{GraphBuilder, PassBuild};
pub use param::{fetch_component, fetch_dt, fetch_resource, StageParam};
pub use se_abi::*;

/// `#[derive(se::Schema)]`
pub use se_macro::Schema;
/// `#[se::stage]`
pub use se_macro::stage;
/// `se::stages!(a, b);`
pub use se_macro::stages;

/// Everything a module file normally wants in scope.
pub mod prelude {
    pub use crate::{
        key, Access, AssetDesc, BufferDesc, Ctl, Draw, Edge, Entity, Extent, Field, Format, Frame,
        GraphDesc, Input, Layout, Pass, ScalarTy, Schema, Slot, SlotBind, Str,
    };
}
