//! Reading the layout rule off the filesystem.
//!
//! > A `.rs` at depth 1 is a module. Anything deeper is a source file inside
//! > one. A folder named after a sibling `.rs` is private to it; one without
//! > is shared across the category.
//!
//! That is the entire configuration format. There is no manifest, because a
//! manifest could disagree with the tree.

use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};

pub const CATEGORIES: [&str; 4] = ["asset", "process", "render", "game"];

#[derive(Debug, Clone)]
pub struct ModuleSrc {
    /// Module name — the file stem.
    pub name: String,
    pub file: PathBuf,
    /// Category directory name, or "bundle".
    pub category: String,
}

#[derive(Debug)]
pub struct Category {
    pub name: String,
    pub dir: PathBuf,
    pub modules: Vec<ModuleSrc>,
    /// Folders with no sibling `.rs`: shared across the category.
    pub shared: Vec<String>,
}

#[derive(Debug)]
pub struct BundleSrc {
    pub name: String,
    pub root: PathBuf,
    pub data: PathBuf,
    pub buffer: PathBuf,
    pub categories: Vec<Category>,
}

fn entries(dir: &Path) -> Result<(Vec<PathBuf>, Vec<PathBuf>)> {
    let mut files = Vec::new();
    let mut dirs = Vec::new();
    for e in std::fs::read_dir(dir).with_context(|| format!("read {}", dir.display()))? {
        let p = e?.path();
        let hidden = p
            .file_name()
            .and_then(|s| s.to_str())
            .is_some_and(|s| s.starts_with('.'));
        if hidden {
            continue;
        }
        if p.is_dir() {
            dirs.push(p);
        } else if p.extension().is_some_and(|e| e == "rs") {
            files.push(p);
        }
    }
    files.sort();
    dirs.sort();
    Ok((files, dirs))
}

fn stem(p: &Path) -> String {
    p.file_name().and_then(|s| s.to_str()).unwrap_or("").to_string()
}

impl BundleSrc {
    pub fn open(root: &Path) -> Result<BundleSrc> {
        if !root.is_dir() {
            bail!("{} is not a directory", root.display());
        }
        let name = root
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("bundle")
            .to_string();

        let bdir = root.join("bundle");
        let data = bdir.join("data.rs");
        let buffer = bdir.join("buffer.rs");
        if !data.is_file() || !buffer.is_file() {
            bail!(
                "`{name}` needs bundle/data.rs and bundle/buffer.rs — both are required, and together they are the contract"
            );
        }

        let mut categories = Vec::new();
        for cat in CATEGORIES {
            let dir = root.join(cat);
            if !dir.is_dir() {
                continue;
            }
            let (files, dirs) = entries(&dir)?;
            let modules: Vec<ModuleSrc> = files
                .iter()
                .map(|f| ModuleSrc {
                    name: f.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string(),
                    file: f.clone(),
                    category: cat.to_string(),
                })
                .collect();
            let names: Vec<String> = modules.iter().map(|m| m.name.clone()).collect();
            let shared: Vec<String> = dirs
                .iter()
                .map(|d| stem(d))
                .filter(|d| !names.contains(d))
                // A shared folder is a module tree, so it needs a root.
                .filter(|d| dir.join(d).join("mod.rs").is_file())
                .collect();
            categories.push(Category { name: cat.to_string(), dir, modules, shared });
        }

        // Deliberately no check for render/ or game/ here. Those are needed to
        // *run* a bundle, and `Bundle::load` says so clearly when they are
        // missing. Requiring them to *build* would mean no module could be
        // compiled until the last one in its bundle existed — which makes a
        // half-written bundle uncheckable and sends you reverse-engineering
        // the compiler invocation instead.
        Ok(BundleSrc { name, root: root.to_path_buf(), data, buffer, categories })
    }

    pub fn category(&self, name: &str) -> Option<&Category> {
        self.categories.iter().find(|c| c.name == name)
    }

    /// Categories a bundle needs before it can run, but not before it can
    /// build. Empty means the bundle is complete.
    pub fn missing_to_run(&self) -> Vec<&'static str> {
        let has = |n: &str| {
            self.categories
                .iter()
                .any(|c| c.name == n && !c.modules.is_empty())
        };
        let mut out = Vec::new();
        if !has("render") {
            out.push("render/*.rs (nothing would ever be shown)");
        }
        if !has("game") {
            out.push("game/*.rs (nothing would know what time it is)");
        }
        out
    }

}

/// Every file under `dir`, skipping dotted entries.
pub fn walkdir(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    if !dir.is_dir() {
        return Ok(out);
    }
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d)? {
            let p = e?.path();
            if p.file_name().and_then(|s| s.to_str()).is_some_and(|s| s.starts_with('.')) {
                continue;
            }
            if p.is_dir() {
                stack.push(p);
            } else {
                out.push(p);
            }
        }
    }
    Ok(out)
}
