//! The deck: every bundle in a directory, and which one is playing.
//!
//! This is GameTok. `design.md` asks for a platform where a swipe takes you to
//! the next game, so the runner's unit is not a bundle but a *stack* of them.
//! Switching tears the old world down and stands a new one up — a different
//! game set is a different contract, and nothing may survive across it.

use crate::discover::BundleSrc;
use anyhow::{bail, Result};
use std::path::{Path, PathBuf};

pub struct Deck {
    pub roots: Vec<PathBuf>,
    pub current: usize,
}

/// A directory is a bundle if it has `bundle/data.rs`; otherwise it is a
/// directory *of* bundles. One rule, so `shinra run game1` and
/// `shinra run shinra-examples` both mean the obvious thing.
fn is_bundle(p: &Path) -> bool {
    p.join("bundle").join("data.rs").is_file()
}

impl Deck {
    pub fn open(path: &Path) -> Result<Deck> {
        if is_bundle(path) {
            return Ok(Deck { roots: vec![path.to_path_buf()], current: 0 });
        }
        let mut roots: Vec<PathBuf> = std::fs::read_dir(path)?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.is_dir() && is_bundle(p))
            .collect();
        roots.sort();
        if roots.is_empty() {
            bail!(
                "{} is neither a bundle nor a directory of bundles (looked for */bundle/data.rs)",
                path.display()
            );
        }
        Ok(Deck { roots, current: 0 })
    }

    pub fn names(&self) -> Vec<String> {
        self.roots
            .iter()
            .map(|p| {
                p.file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("?")
                    .to_string()
            })
            .collect()
    }

    pub fn root(&self) -> &Path {
        &self.roots[self.current]
    }

    pub fn src(&self) -> Result<BundleSrc> {
        BundleSrc::open(self.root())
    }

    /// Wraps, because a deck has no end — that is the whole idea.
    pub fn step(&mut self, delta: i32) -> bool {
        if self.roots.len() < 2 {
            return false;
        }
        let n = self.roots.len() as i32;
        self.current = (((self.current as i32 + delta) % n + n) % n) as usize;
        true
    }
}
