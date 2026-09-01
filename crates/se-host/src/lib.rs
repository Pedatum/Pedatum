//! The Shinra host: it owns the world, the clock and the module table, and it
//! is the only thing in the system that is not hot swappable.
//!
//! Everything the host knows about a game arrives at runtime through six entry
//! points. It has no types for any of it — only layouts, buffer descriptions,
//! stage specs and a graph — which is precisely why a module can be rebuilt
//! and reloaded underneath a world that keeps its contents.

pub mod bundle;
pub mod ctl;
pub mod loader;
pub mod registry;
pub mod schedule;
pub mod sink;
pub mod world;

pub use bundle::{Bundle, Loaded};
pub use ctl::{HostCtx, Requests};
pub use loader::Module;
pub use world::World;
