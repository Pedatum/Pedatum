//! The pure half of the engine: buffers in, buffers out.
//!
//! Nothing in this crate can reach the world, spawn anything, or know what
//! time it is beyond the frame globals it is handed. That is not a convention
//! — `render/*.so` registers nodes and edges and nothing else, so there is no
//! call available to it that could.

pub mod attrs;
pub mod gpu;
pub mod graph;
pub mod mesh;
pub mod prelude_wgsl;
pub mod readback;
pub mod targets;

pub use gpu::Gpu;
pub use graph::{Graph, Scene};
pub use readback::{read, Frame};
pub use targets::{Target, Targets};

use anyhow::Result;
use se_host::registry::{AssetDef, BufferDef, GraphDef};
use se_host::World;

/// Everything the render side needs, kept together so the run loop holds one
/// thing rather than four.
pub struct Renderer {
    pub gpu: Gpu,
    pub targets: Targets,
    pub graph: Graph,
    /// How tall a framebuffer pixel is relative to its width, on the surface
    /// that will show it.
    ///
    /// A window has square pixels and this is 1. A terminal does not: a cell
    /// is about twice as tall as it is wide, so a renderer packing 2x4
    /// subpixels into one gets squares, while 2x2 gets pixels twice as tall as
    /// they are wide. Shaders derive aspect from `se.resolution`, so the
    /// resolution they are told has to be the *apparent* one or every scene
    /// drawn in that mode is stretched.
    pub pixel_aspect: f32,
}

impl Renderer {
    pub fn new(
        buffers: &[BufferDef],
        graph: &GraphDef,
        screen: (u32, u32),
        world: &World,
        assets: &[&AssetDef],
    ) -> Result<Renderer> {
        let gpu = Gpu::new()?;
        let targets = Targets::new(&gpu.device, buffers, screen);
        let graph = Graph::build(&gpu.device, graph, &targets, &Scene { world, assets })?;
        Ok(Renderer { gpu, targets, graph, pixel_aspect: 1.0 })
    }

    /// Swap in a rebuilt `render/*.so` without touching buffers or the world.
    pub fn swap_graph(&mut self, graph: &GraphDef, world: &World, assets: &[&AssetDef]) -> Result<()> {
        self.graph = Graph::build(&self.gpu.device, graph, &self.targets, &Scene { world, assets })?;
        Ok(())
    }

    pub fn resize(&mut self, screen: (u32, u32), world: &World, assets: &[&AssetDef]) -> Result<()> {
        if screen == self.targets.screen() {
            return Ok(());
        }
        self.targets.resize(&self.gpu.device, screen);
        self.graph
            .recompile(&self.gpu.device, &self.targets, &Scene { world, assets })
    }

    /// Run the graph and read back what it presented.
    pub fn frame(
        &mut self,
        world: &World,
        assets: &[&AssetDef],
        time: f64,
        dt: f32,
        index: u64,
    ) -> Result<Frame> {
        self.graph.render(
            &self.gpu.device,
            &self.gpu.queue,
            &self.targets,
            &Scene { world, assets },
            time,
            dt,
            index,
            self.pixel_aspect,
        )?;
        let present = self.targets.get(&self.graph.def().present)?;
        let frame = readback::read(&self.gpu.device, &self.gpu.queue, present)?;
        self.targets.flip();
        Ok(frame)
    }
}
