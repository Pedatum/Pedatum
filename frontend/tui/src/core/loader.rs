use anyhow::{Context, Result};
use scene::Scene;
use std::path::{Path, PathBuf};

pub struct Loader {
    games: Vec<GameEntry>,
    current: usize,
}

#[allow(dead_code)]
pub struct GameEntry {
    pub name: String,
    pub dir: PathBuf,
    pub scene: Scene,
    /// The module `shinra build` produces for this game, if it has been built.
    pub module: Option<PathBuf>,
}

impl Loader {
    pub fn scan(games_dir: &Path) -> Result<Self> {
        let mut games = Vec::new();
        for entry in std::fs::read_dir(games_dir)
            .with_context(|| format!("scan {}", games_dir.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let scene_path = path.join("scene.ron");
            if !scene_path.exists() {
                continue;
            }
            let raw = std::fs::read_to_string(&scene_path)
                .with_context(|| format!("read {}", scene_path.display()))?;
            let scene: Scene = ron::from_str(&raw)
                .with_context(|| format!("parse {}", scene_path.display()))?;
            let name = entry.file_name().to_string_lossy().into_owned();
            // <project>/assets/games/<name> -> <project>/target/games/lib<name>.so
            let module = games_dir
                .parent()
                .and_then(|p| p.parent())
                .map(|project| project.join(format!("target/games/lib{name}.so")))
                .filter(|p| p.exists());
            games.push(GameEntry {
                name,
                dir: path,
                scene,
                module,
            });
        }
        games.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(Self { games, current: 0 })
    }

    pub fn current_game(&self) -> Option<&GameEntry> {
        self.games.get(self.current)
    }

    pub fn current_game_mut(&mut self) -> Option<&mut GameEntry> {
        self.games.get_mut(self.current)
    }

    pub fn next_game(&mut self) {
        if !self.games.is_empty() {
            self.current = (self.current + 1) % self.games.len();
        }
    }

    pub fn game_count(&self) -> usize {
        self.games.len()
    }

    pub fn current_index(&self) -> usize {
        self.current
    }
}
