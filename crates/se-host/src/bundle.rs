//! A bundle: one contract, and the modules living under it.
//!
//! Everything is loaded through `bundle.so` first, because nothing else means
//! anything without it. Change a layout or a buffer and the contract hash
//! moves, which is not a reload — it is a different game set, and the host
//! tears the whole thing down rather than pretend the pieces still fit.

use crate::loader::Module;
use crate::registry::{AssetDef, ControlDef, GraphDef, LayoutDef, StageDef};
use crate::sink;
use anyhow::{anyhow, bail, Context, Result};
use se_abi::{prim, sym, Slot};
use std::path::{Path, PathBuf};

/// A module plus what was registered out of it. Items are declared first so
/// they drop before the library they came from.
pub struct Loaded<T> {
    pub items: Vec<T>,
    pub module: Module,
}

pub struct Bundle {
    pub name: String,
    pub root: PathBuf,
    pub layouts: Vec<LayoutDef>,
    pub buffers: Vec<crate::registry::BufferDef>,
    /// Fold of every layout and buffer. Two bundles agreeing here are the same
    /// game set as far as any other module can tell.
    pub contract: u64,
    pub assets: Vec<Loaded<AssetDef>>,
    pub process: Vec<Loaded<StageDef>>,
    pub render: Loaded<GraphDef>,
    pub control: Loaded<ControlDef>,
    /// Held last: everything above was described by it.
    pub bundle: Module,
}

fn modules_in(dir: &Path) -> Result<Vec<PathBuf>> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut out: Vec<PathBuf> = std::fs::read_dir(dir)
        .with_context(|| format!("read {}", dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "so"))
        .collect();
    out.sort();
    Ok(out)
}

/// Slot bindings choose *among* alternatives. A category nobody chose from
/// loads everything in it — which is what makes `process/` additive and
/// `render/` a decision.
fn pick(paths: &[PathBuf], bound: Option<&str>) -> Vec<PathBuf> {
    match bound {
        None => paths.to_vec(),
        Some(want) => paths
            .iter()
            .filter(|p| p.file_stem().and_then(|s| s.to_str()) == Some(want))
            .cloned()
            .collect(),
    }
}

impl Bundle {
    /// `root` is a built bundle directory: `bundle.so` plus the category dirs.
    pub fn load(root: &Path, control: Option<&str>) -> Result<Bundle> {
        Bundle::load_with(root, control, &[])
    }

    /// As `load`, but with slot choices the control layer made at runtime
    /// taking precedence over the ones it declared. This is what `set_slot`
    /// turns into: the same bundle, with a different module in one position.
    pub fn load_with(
        root: &Path,
        control: Option<&str>,
        overrides: &[(Slot, String)],
    ) -> Result<Bundle> {
        let name = root
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("bundle")
            .to_string();

        let bundle = Module::open(&root.join("bundle.so"))
            .with_context(|| format!("`{name}` has no bundle.so — data.rs and buffer.rs are both required"))?;
        let layouts = unsafe { sink::layouts(&bundle)? };
        let buffers = unsafe { sink::buffers(&bundle)? };
        if layouts.is_empty() {
            bail!("`{name}` declares no components");
        }
        if buffers.is_empty() {
            bail!("`{name}` declares no buffers, so nothing could ever be shown");
        }
        let contract = contract_hash(&layouts, &buffers);

        // Control comes next: it says which module fills which slot.
        let games = modules_in(&root.join("game"))?;
        let game_path = match control {
            Some(want) => games
                .iter()
                .find(|p| p.file_stem().and_then(|s| s.to_str()) == Some(want))
                .cloned()
                .ok_or_else(|| anyhow!("`{name}` has no game/{want}.so"))?,
            None => games
                .first()
                .cloned()
                .ok_or_else(|| anyhow!("`{name}` has no game/*.so"))?,
        };
        let control = Self::load_one(&game_path, sink::controls)?;
        let spec = control
            .items
            .first()
            .ok_or_else(|| anyhow!("game/{} registered no control", control.module.name))?;

        let bound = |s: Slot| -> Option<String> {
            overrides
                .iter()
                .find(|(k, _)| *k == s)
                .or_else(|| spec.slots.iter().find(|(k, _)| *k == s))
                .map(|(_, m)| m.clone())
        };
        let (want_asset, want_process, want_render) =
            (bound(Slot::Asset), bound(Slot::Process), bound(Slot::Render));

        let mut assets = Vec::new();
        for p in pick(&modules_in(&root.join("asset"))?, want_asset.as_deref()) {
            assets.push(Self::load_one(&p, sink::assets)?);
        }
        let mut process = Vec::new();
        for p in pick(&modules_in(&root.join("process"))?, want_process.as_deref()) {
            process.push(Self::load_one(&p, sink::stages)?);
        }

        let renders = pick(&modules_in(&root.join("render"))?, want_render.as_deref());
        let render_path = match renders.as_slice() {
            [one] => one.clone(),
            [] => bail!("`{name}` has no render module to fill the render slot"),
            many => bail!(
                "`{name}` has {} render modules and the control bound none — a slot holds one graph",
                many.len()
            ),
        };
        let render = Self::load_one(&render_path, sink::graphs)?;
        if render.items.is_empty() {
            bail!("render/{} registered no graph", render.module.name);
        }

        Ok(Bundle {
            name,
            root: root.to_path_buf(),
            layouts,
            buffers,
            contract,
            assets,
            process,
            render,
            control,
            bundle,
        })
    }

    fn load_one<T>(
        path: &Path,
        collect: unsafe fn(&Module) -> Result<Vec<T>>,
    ) -> Result<Loaded<T>> {
        let module = Module::open(path)?;
        let items = unsafe { collect(&module)? };
        Ok(Loaded { items, module })
    }

    pub fn graph(&self) -> &GraphDef {
        &self.render.items[0]
    }
    pub fn control_spec(&self) -> &ControlDef {
        &self.control.items[0]
    }
    pub fn stages(&self) -> impl Iterator<Item = &StageDef> {
        self.process.iter().flat_map(|l| l.items.iter())
    }
    pub fn all_assets(&self) -> Vec<&AssetDef> {
        self.assets.iter().flat_map(|l| l.items.iter()).collect()
    }

    /// Every module that could be swapped without breaking the contract.
    pub fn hot_modules(&self) -> Vec<&Module> {
        let mut v: Vec<&Module> = Vec::new();
        v.extend(self.assets.iter().map(|l| &l.module));
        v.extend(self.process.iter().map(|l| &l.module));
        v.push(&self.render.module);
        v.push(&self.control.module);
        v
    }

    /// Which loaded modules have been rebuilt since they were opened.
    pub fn stale(&self) -> Vec<String> {
        let mut v: Vec<String> = self
            .hot_modules()
            .iter()
            .filter(|m| m.stale())
            .map(|m| m.name.clone())
            .collect();
        if self.bundle.stale() {
            v.push("bundle".into());
        }
        v
    }
}

fn contract_hash(layouts: &[LayoutDef], buffers: &[crate::registry::BufferDef]) -> u64 {
    let mut ls: Vec<u64> = layouts.iter().map(|l| l.hash).collect();
    ls.sort_unstable();
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for x in ls {
        h = prim::mix(h, x);
    }
    let mut bs: Vec<u64> = buffers
        .iter()
        .map(|b| {
            let mut x = prim::fnv1a(b.name.as_bytes());
            x = prim::mix(x, b.format as u32 as u64);
            x = prim::mix(x, b.extent as u32 as u64);
            x = prim::mix(x, ((b.width as u64) << 32) | b.height as u64);
            x = prim::mix(x, b.count as u64);
            prim::mix(x, b.sampled as u64)
        })
        .collect();
    bs.sort_unstable();
    for x in bs {
        h = prim::mix(h, x);
    }
    h
}

/// The one symbol every module must export, checked at open time.
pub const ABI_SYMBOL: &[u8] = sym::ABI_VERSION;
