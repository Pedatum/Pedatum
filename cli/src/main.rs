//! `shinra` — the engine's build tool.
//!
//! A game folder holds only data and `.hom`:
//!
//!     assets/games/game3/
//!       scene.ron
//!       input.tres.ron
//!       player.hom  scroller.hom  obstacle.hom
//!
//! `shinra build assets/games/game3` compiles those systems with `homunc`,
//! derives the engine glue from what they declared, and produces a loadable
//! module. The game never writes a line of Rust.

mod discover;
mod generate;

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};

/// Where the engine's own sources live, for the generated crate's dependency
/// paths and for the Homun shim.
fn engine_root() -> PathBuf {
    if let Ok(p) = std::env::var("SHINRA_ENGINE_ROOT") {
        return PathBuf::from(p);
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("cli lives inside the engine")
        .to_path_buf()
}

fn find_homunc() -> Result<PathBuf> {
    if let Ok(p) = std::env::var("HOMUNC") {
        return Ok(PathBuf::from(p));
    }
    let root = engine_root()
        .parent()
        .context("engine root has no parent")?
        .join("Homun-Lang");
    for candidate in [
        "target/release/homunc",
        "target/debug/homunc",
        ".tmp/homunc",
    ] {
        let p = root.join(candidate);
        if p.exists() {
            return Ok(p);
        }
    }
    bail!(
        "homunc not found under {}. Build it, or set HOMUNC=/path/to/homunc.",
        root.display()
    )
}

struct Layout {
    game: String,
    game_dir: PathBuf,
    staging: PathBuf,
    out_dir: PathBuf,
}

fn layout(game_dir: &Path) -> Result<Layout> {
    let game_dir = game_dir
        .canonicalize()
        .with_context(|| format!("no such game folder: {}", game_dir.display()))?;
    let game = game_dir
        .file_name()
        .context("game folder has no name")?
        .to_string_lossy()
        .into_owned();
    // <project>/assets/games/<name> → <project>
    let project = game_dir
        .ancestors()
        .nth(3)
        .context("expected <project>/assets/games/<name>")?
        .to_path_buf();
    Ok(Layout {
        staging: project.join("target/shinra").join(&game),
        out_dir: project.join("target/games"),
        game,
        game_dir,
    })
}

fn build(game_dir: &Path) -> Result<PathBuf> {
    let lo = layout(game_dir)?;
    let homunc = find_homunc()?;
    let engine = engine_root();
    let shim = engine.join("engine/hom/engine.rs");
    if !shim.exists() {
        bail!("Homun shim missing at {}", shim.display());
    }

    let src = lo.staging.join("src");
    let _ = std::fs::remove_dir_all(&lo.staging);
    std::fs::create_dir_all(&src)?;

    // One shared copy of the shim: every module is compiled with
    // `--extern engine`, so they all reference this one and share its types.
    std::fs::copy(&shim, src.join("engine.rs"))?;

    // The Homun runtime, once. `--module` output uses its macros, and
    // `--runtime-path` below tells each module where to import them from.
    let rt = Command::new(&homunc)
        .arg("--emit-runtime")
        .output()
        .with_context(|| format!("running {} --emit-runtime", homunc.display()))?;
    if !rt.status.success() {
        bail!("homunc --emit-runtime failed");
    }
    std::fs::write(src.join("runtime.rs"), rt.stdout)?;

    // game.ron names the action map; the module compiles it in, because which
    // key means what is this game's data.
    let manifest = lo.game_dir.join("game.ron");
    let text = std::fs::read_to_string(&manifest)
        .with_context(|| format!("read {}", manifest.display()))?;
    let game: scene::Game =
        ron::from_str(&text).with_context(|| format!("parse {}", manifest.display()))?;
    if game.root().is_none() {
        bail!(
            "{} declares no view named `{}`",
            manifest.display(),
            scene::ROOT_VIEW
        );
    }
    let action_map = lo.game_dir.join(&game.input);
    if !action_map.exists() {
        bail!("{} names {}, which is missing", manifest.display(), game.input);
    }
    std::fs::copy(&action_map, src.join("input.tres.ron"))?;

    // Compile each .hom, then learn what it declared.
    let mut hom_files: Vec<PathBuf> = std::fs::read_dir(&lo.game_dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("hom"))
        .collect();
    hom_files.sort();
    if hom_files.is_empty() {
        bail!("{} has no .hom systems", lo.game_dir.display());
    }

    let mut modules = Vec::new();
    for hom in &hom_files {
        let stem = hom.file_stem().unwrap().to_string_lossy().into_owned();
        let out = src.join(format!("{stem}.rs"));
        let status = Command::new(&homunc)
            .arg("--include")
            .arg(engine.join("engine/hom"))
            .arg("--extern")
            .arg("engine")
            .arg("--runtime-path")
            .arg("super::runtime")
            .arg("--module")
            .arg(hom)
            .arg("-o")
            .arg(&out)
            .status()
            .with_context(|| format!("running {}", homunc.display()))?;
        if !status.success() {
            bail!("homunc failed on {}", hom.display());
        }
        let generated = std::fs::read_to_string(&out)?;
        modules.push(discover::parse_module(&stem, &generated));
    }

    std::fs::write(
        lo.staging.join("Cargo.toml"),
        generate::cargo_toml(&lo.game, &engine.to_string_lossy()),
    )?;
    std::fs::write(src.join("lib.rs"), generate::lib_rs(&lo.game, &modules))?;

    // One target directory for every game in the project: each links the same
    // engine, so a per-game target dir would rebuild the whole dependency tree
    // once per game.
    let shared_target = lo.staging.parent().context("staging has no parent")?.join("build");
    let status = Command::new("cargo")
        .arg("build")
        .current_dir(&lo.staging)
        .env("CARGO_TARGET_DIR", &shared_target)
        .status()
        .context("running cargo build for the generated crate")?;
    if !status.success() {
        bail!(
            "the generated crate did not build; inspect {}",
            lo.staging.display()
        );
    }

    let so = shared_target.join(format!("debug/lib{}.so", lo.game));
    std::fs::create_dir_all(&lo.out_dir)?;
    let dest = lo.out_dir.join(format!("lib{}.so", lo.game));
    std::fs::copy(&so, &dest)
        .with_context(|| format!("copying {} to {}", so.display(), dest.display()))?;

    let summary: Vec<String> = modules
        .iter()
        .map(|m| {
            format!(
                "{} ({} components, {} systems)",
                m.name,
                m.components.len(),
                m.systems.len()
            )
        })
        .collect();
    println!("built {} from {}", lo.game, summary.join(", "));
    println!("  -> {}", dest.display());
    Ok(dest)
}

fn usage() -> ! {
    eprintln!(
        "\
shinra — build a game folder into a loadable module

USAGE:
  shinra build <game-folder>     Compile its .hom systems and emit lib<name>.so

A game folder holds scene.ron, input.tres.ron and .hom files. Everything else
is generated."
    );
    std::process::exit(2)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.split_first() {
        Some((cmd, rest)) if cmd == "build" && rest.len() == 1 => {
            if let Err(e) = build(Path::new(&rest[0])) {
                eprintln!("shinra: {e:#}");
                std::process::exit(1);
            }
        }
        _ => usage(),
    }
}
