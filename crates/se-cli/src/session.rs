//! One bundle, loaded and running.
//!
//! The world is a field of this struct and the modules are behind it, which is
//! the whole hot-swap story in one sentence: reloading replaces `bundle`,
//! never `world`. A contract change is the exception, and it is an exception
//! precisely because the world can no longer be trusted to mean the same thing.

use crate::build::{self, Engine};
use crate::discover::BundleSrc;
use anyhow::{bail, Context, Result};
use se_abi::Slot;
use se_host::{schedule, Bundle, HostCtx, World};
use se_render::Renderer;
use std::path::PathBuf;

pub struct Session {
    pub src: BundleSrc,
    pub out: PathBuf,
    pub bundle: Bundle,
    pub world: World,
    pub renderer: Renderer,
    pub contract: u64,
    pub started: bool,
    pub log: Vec<String>,
    /// Set by the last tick; the run loop acts on them at the frame boundary.
    pub quit: bool,
    pub swipe: i32,
    /// Slot choices `set_slot` made, overriding what the control declared.
    pub slots: Vec<(Slot, String)>,
    control: Option<String>,
}

impl Session {
    pub fn open(
        src: BundleSrc,
        out: PathBuf,
        screen: (u32, u32),
        control: Option<&str>,
    ) -> Result<Session> {
        let bundle = Bundle::load(&out, control)?;
        let mut world = World::new();
        world.declare(&bundle.layouts);
        for s in bundle.stages() {
            schedule::validate(&world, s)
                .with_context(|| format!("stage `{}` does not fit the bundle contract", s.name))?;
        }
        let renderer = {
            let assets = bundle.all_assets();
            Renderer::new(&bundle.buffers, bundle.graph(), screen, &world, &assets)?
        };
        let contract = bundle.contract;
        Ok(Session {
            src,
            out,
            bundle,
            world,
            renderer,
            contract,
            started: false,
            log: Vec::new(),
            quit: false,
            swipe: 0,
            slots: Vec::new(),
            control: control.map(str::to_string),
        })
    }

    pub fn note(&mut self, s: impl Into<String>) {
        self.log.push(s.into());
        if self.log.len() > 64 {
            self.log.remove(0);
        }
    }

    /// Call `start` once, after slots are bound.
    pub fn start(&mut self) {
        if self.started {
            return;
        }
        self.started = true;
        let Some(f) = self.bundle.control_spec().start else { return };
        let assets = self.bundle.all_assets();
        let mut ctx = HostCtx::new(&mut self.world, &assets);
        let mut vt = ctx.vtable();
        // SAFETY: the vtable and its context outlive the call.
        unsafe { f(&mut vt) };
        let req = std::mem::take(&mut ctx.req);
        self.log.extend(req.log);
    }

    /// One control tick, then every stage, then the graph.
    pub fn tick(&mut self, frame: &se_abi::Frame) -> Result<se_render::Frame> {
        let assets = self.bundle.all_assets();
        let mut ctx = HostCtx::new(&mut self.world, &assets);
        let mut vt = ctx.vtable();
        let tick = self.bundle.control_spec().tick;
        // SAFETY: `vt` borrows `ctx`, which outlives this call.
        unsafe { tick(&mut vt, frame as *const se_abi::Frame) };
        let req = std::mem::take(&mut ctx.req);
        drop(assets);

        for m in req.log {
            self.note(m);
        }
        if let Some(p) = req.save {
            self.note(format!("save `{p}` — snapshots are not implemented yet"));
        }
        if let Some(p) = req.load {
            self.note(format!("load `{p}` — snapshots are not implemented yet"));
        }
        self.quit = req.quit;
        self.swipe = req.swipe;
        if !req.slots.is_empty() {
            let want = req.slots.clone();
            if let Err(e) = self.rebind(&want) {
                self.note(format!("slot change refused: {e}"));
            }
        }

        // SAFETY of ordering: control has finished writing before any stage
        // reads, and no stage can spawn, so column bases stay put.
        let input = unsafe { &*frame.input };
        let stages: Vec<_> = self.bundle.stages().collect();
        for s in stages {
            schedule::run(&mut self.world, s, input, frame.dt)?;
        }

        let assets = self.bundle.all_assets();
        self.renderer
            .frame(&self.world, &assets, frame.t, frame.dt, frame.index)
    }

    /// Reallocate screen-sized buffers and rebuild the pipelines behind them.
    pub fn resize(&mut self, screen: (u32, u32)) -> Result<()> {
        let assets = self.bundle.all_assets();
        self.renderer.resize(screen, &self.world, &assets)
    }

    /// Draw the world as it stands, advancing nothing.
    ///
    /// This is what the editor shows: control's `start` has reconciled the
    /// roster, so there is a scene to inspect, but no tick has run and nothing
    /// moves. An engine editor opens on a still scene; it does not start
    /// playing at you.
    pub fn render_only(&mut self, frame: &se_abi::Frame) -> Result<se_render::Frame> {
        let assets = self.bundle.all_assets();
        self.renderer
            .frame(&self.world, &assets, frame.t, 0.0, frame.index)
    }

    /// Point a slot at a different module and reload just enough to make it
    /// true. The world is untouched: swapping `asset/bunny` for
    /// `asset/teapot` changes what a thing looks like, never what exists.
    pub fn rebind(&mut self, want: &[(Slot, String)]) -> Result<()> {
        for (slot, module) in want {
            self.slots.retain(|(k, _)| k != slot);
            self.slots.push((*slot, module.clone()));
        }
        let control = self
            .control
            .clone()
            .unwrap_or_else(|| self.bundle.control.module.name.clone());
        let fresh = Bundle::load_with(&self.out, Some(&control), &self.slots)?;
        if fresh.contract != self.contract {
            bail!("that module answers to a different bundle contract");
        }
        for s in fresh.stages() {
            schedule::validate(&self.world, s)?;
        }
        let screen = self.renderer.targets.screen();
        {
            let assets = fresh.all_assets();
            self.renderer.swap_graph(fresh.graph(), &self.world, &assets)?;
            self.renderer.resize(screen, &self.world, &assets)?;
        }
        let names: Vec<&str> = want.iter().map(|(_, m)| m.as_str()).collect();
        self.bundle = fresh;
        self.note(format!("slot → {}", names.join(", ")));
        Ok(())
    }

    /// Rebuild what changed on disk and swap it in, keeping the world.
    pub fn reload(&mut self, engine: &Engine, profile: &str, screen: (u32, u32)) -> Result<bool> {
        let changed = build::build_changed(&self.src, &self.out, engine, profile)?;
        if changed.is_empty() {
            return Ok(false);
        }
        let control = self.bundle.control.module.name.clone();

        // Let the outgoing control clean up before its image goes away.
        if let Some(f) = self.bundle.control_spec().stop {
            let assets = self.bundle.all_assets();
            let mut ctx = HostCtx::new(&mut self.world, &assets);
            let mut vt = ctx.vtable();
            unsafe { f(&mut vt) };
        }

        let fresh = Bundle::load_with(&self.out, Some(&control), &self.slots)?;
        if fresh.contract != self.contract {
            bail!("the bundle contract changed — reload the whole game set");
        }
        for s in fresh.stages() {
            schedule::validate(&self.world, s)?;
        }
        {
            let assets = fresh.all_assets();
            self.renderer.swap_graph(fresh.graph(), &self.world, &assets)?;
            self.renderer.resize(screen, &self.world, &assets)?;
        }
        self.bundle = fresh;
        self.started = false;
        self.note(format!("reloaded {}", changed.join(", ")));
        self.start();
        Ok(true)
    }
}
