//! `shinra` — build a bundle, and run it.
//!
//! ```text
//! shinra build <bundle>            compile every module to .so
//! shinra run   <bundle>            build, then run with the TUI
//! shinra shot  <bundle> [-n N]     render N frames headless, print a PPM
//! ```
//!
//! A bundle is a directory laid out the way `design.md` says. There is no
//! project file to keep in sync with it.

mod build;
mod deck;
mod discover;
mod run;
mod session;

use anyhow::{bail, Result};
use build::Engine;
use discover::BundleSrc;
use session::Session;
use std::path::{Path, PathBuf};

const USAGE: &str = "\
shinra — the Shinra Engine runner

  shinra build <bundle> [options]     compile every module to .so
  shinra run   <bundle|deck> [opts]   build, then run it in the IDE
                                      a directory of bundles is a deck;
                                      press n to move through it
  shinra shot  <bundle> [options]     render headless, write a PPM to stdout

options
  -o, --out <dir>     where .so files go     (default target/shinra/<name>)
      --game <name>   which game/*.rs drives (default: the first one)
      --release       optimise modules
      --no-watch      do not rebuild on change while running
  -n, --frames <N>    stop after N frames
      --size <WxH>    headless framebuffer   (default 320x180)
";

struct Args {
    cmd: String,
    bundle: PathBuf,
    out: Option<PathBuf>,
    game: Option<String>,
    profile: String,
    watch: bool,
    frames: Option<u64>,
    size: (u32, u32),
}

fn parse() -> Result<Args> {
    let mut it = std::env::args().skip(1);
    let cmd = it.next().unwrap_or_default();
    if cmd.is_empty() || cmd == "-h" || cmd == "--help" {
        print!("{USAGE}");
        std::process::exit(if cmd.is_empty() { 2 } else { 0 });
    }
    let bundle = PathBuf::from(it.next().unwrap_or_default());
    let mut a = Args {
        cmd,
        bundle,
        out: None,
        game: None,
        profile: "debug".into(),
        watch: true,
        frames: None,
        size: (320, 180),
    };
    while let Some(f) = it.next() {
        match f.as_str() {
            "-o" | "--out" => a.out = it.next().map(PathBuf::from),
            "--game" => a.game = it.next(),
            "--release" => a.profile = "release".into(),
            "--no-watch" => a.watch = false,
            "-n" | "--frames" => a.frames = it.next().and_then(|v| v.parse().ok()),
            "--size" => {
                if let Some(v) = it.next() {
                    let (w, h) = v.split_once('x').unwrap_or(("320", "180"));
                    a.size = (w.parse().unwrap_or(320), h.parse().unwrap_or(180));
                }
            }
            other => bail!("unknown option `{other}`\n\n{USAGE}"),
        }
    }
    if a.bundle.as_os_str().is_empty() {
        bail!("which bundle?\n\n{USAGE}");
    }
    Ok(a)
}

fn out_dir(a: &Args, src: &BundleSrc) -> PathBuf {
    a.out
        .clone()
        .unwrap_or_else(|| Path::new("target").join("shinra").join(&src.name))
}

fn main() -> Result<()> {
    let a = parse()?;
    let engine = Engine::find(&a.profile)?;

    // `run` takes a deck and opens bundles itself; the others act on one.
    if a.cmd == "run" {
        engine.ensure_se(&a.profile)?;
        let deck = deck::Deck::open(&a.bundle)?;
        return run::run(
            deck,
            &engine,
            run::Options {
                profile: a.profile,
                watch: a.watch,
                frames: a.frames,
                game: a.game,
            },
        );
    }

    let src = BundleSrc::open(&a.bundle)?;
    let out = out_dir(&a, &src);

    match a.cmd.as_str() {
        "build" => {
            engine.ensure_se(&a.profile)?;
            let built = build::build(&src, &out, &engine, &a.profile)?;
            for m in &built.modules {
                println!("  {m}.so");
            }
            println!("{} modules → {}", built.modules.len(), out.display());
            // Building a partial bundle is a normal state while one is being
            // written, so this is a note and an exit code of 0 — not an error.
            let missing = src.missing_to_run();
            if !missing.is_empty() {
                println!();
                println!("bundle is not runnable yet; still needs:");
                for m in &missing {
                    println!("  {m}");
                }
                println!("every module present compiled, so this build did check your work.");
            }
            Ok(())
        }
        "shot" => {
            engine.ensure_se(&a.profile)?;
            build::build(&src, &out, &engine, &a.profile)?;
            let s = Session::open(src, out, a.size, a.game.as_deref())?;
            let f = run::headless(s, a.frames.unwrap_or(1))?;
            write_ppm(&f)
        }
        other => bail!("unknown command `{other}`\n\n{USAGE}"),
    }
}

/// Binary PPM: the least ceremony that still opens in an image viewer, and
/// trivially diffable in a test.
fn write_ppm(f: &se_render::Frame) -> Result<()> {
    use std::io::Write;
    let mut o = std::io::stdout().lock();
    write!(o, "P6\n{} {}\n255\n", f.width, f.height)?;
    let mut rgb = Vec::with_capacity((f.width * f.height * 3) as usize);
    for p in f.rgba.chunks_exact(4) {
        rgb.extend_from_slice(&p[..3]);
    }
    o.write_all(&rgb)?;
    o.flush()?;
    Ok(())
}
