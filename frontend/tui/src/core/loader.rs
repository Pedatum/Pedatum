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
    /// The screen: the game picture plus whatever UI sits beside it.
    pub canvas: Option<scene::Canvas>,
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
            // A game folder is one that has a game.ron. The world it simulates
            // is what the IDE's viewport draws.
            let manifest = path.join("game.ron");
            if !manifest.exists() {
                continue;
            }
            let world_path = path.join("world.ron");
            let raw = std::fs::read_to_string(&world_path)
                .with_context(|| format!("read {}", world_path.display()))?;
            let scene: Scene = ron::from_str(&raw)
                .with_context(|| format!("parse {}", world_path.display()))?;
            let name = entry.file_name().to_string_lossy().into_owned();
            // <project>/assets/games/<name> -> <project>/target/games/lib<name>.so
            let module = games_dir
                .parent()
                .and_then(|p| p.parent())
                .map(|project| project.join(format!("target/games/lib{name}.so")))
                .filter(|p| p.exists());
            // A canvas is optional: without one the viewport draws the world
            // directly, which is what the editor does anyway.
            let canvas = std::fs::read_to_string(path.join("canvas.ron"))
                .ok()
                .and_then(|raw| ron::from_str::<scene::Canvas>(&raw).ok());
            games.push(GameEntry {
                name,
                dir: path,
                scene,
                module,
                canvas,
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
