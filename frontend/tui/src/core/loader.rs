//! Finding games on disk and reading what each one declares.
//!
//! ```text
//! game.ron ──> views ──> render units (world.ron | canvas.ron)
//! ```
//!
//! The loader walks exactly that: `game.ron` first, then whatever its views
//! point at. A game whose root view is a world has no canvas at all — a bunny
//! spinning on screen needs no UI, and inventing an empty canvas for it would
//! only put a compositing pass between the world and the terminal.

use anyhow::{anyhow, Context, Result};
use scene::{Camera, Game, Scene, ViewDef};
use std::path::{Path, PathBuf};

pub struct Loader {
    games: Vec<GameEntry>,
    current: usize,
}

#[allow(dead_code)]
pub struct GameEntry {
    pub name: String,
    pub dir: PathBuf,
    /// What the game declares: its views, and the camera each one looks through.
    pub game: Game,
    /// The world every view of this game draws. One world per game for now; a
    /// game with several needs each view's own `unit` resolved separately.
    pub scene: Scene,
    /// The module `shinra build` produces for this game, if it has been built.
    pub module: Option<PathBuf>,
    /// The screen, when the root view draws one: the game picture is a node in
    /// it and any UI is a sibling. `None` when the root view is a world.
    pub canvas: Option<scene::Canvas>,
}

impl GameEntry {
    /// The root view, which is what the viewport panel shows.
    pub fn root_view(&self) -> Option<&ViewDef> {
        self.game.root()
    }

    /// The camera a named view looks through, falling back to the root's.
    pub fn camera(&self, view: &str) -> Option<&Camera> {
        self.game
            .views
            .get(view)
            .or_else(|| self.root_view())
            .map(|v| &v.camera)
    }
}

impl Loader {
    pub fn scan(games_dir: &Path) -> Result<Self> {
        let mut games = Vec::new();
        for entry in std::fs::read_dir(games_dir)
            .with_context(|| format!("scan {}", games_dir.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            if !path.is_dir() || !path.join("game.ron").exists() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            match load_game(&path, games_dir, &name) {
                Ok(game) => games.push(game),
                // One malformed game must not take the IDE down with it.
                Err(e) => eprintln!("[ide] skipping {name}: {e:#}"),
            }
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

fn load_game(dir: &Path, games_dir: &Path, name: &str) -> Result<GameEntry> {
    let game: Game = read_ron(&dir.join("game.ron"))?;
    let root = game
        .root()
        .ok_or_else(|| anyhow!("no `{}` view", scene::ROOT_VIEW))?;

    // Every declared view is checked now rather than when it first draws, so a
    // mistyped stage chain is an error next to the file that has it.
    for (view_name, view) in &game.views {
        view.stages_fit()
            .map_err(|e| anyhow!("view `{view_name}`: {e}"))?;
    }

    // The root view says whether this game has a screen. A world root draws
    // straight to the terminal; a canvas root composites.
    let canvas = match &root.unit {
        scene::UnitRef::Canvas(path) => Some(read_ron(&dir.join(path))?),
        scene::UnitRef::World(_) => None,
    };

    // Whichever view targets a world, that is the world this game simulates.
    let world_path = game
        .views
        .values()
        .find_map(|v| match &v.unit {
            scene::UnitRef::World(p) => Some(p.clone()),
            scene::UnitRef::Canvas(_) => None,
        })
        .ok_or_else(|| anyhow!("no view draws a world"))?;
    let scene: Scene = read_ron(&dir.join(&world_path))?;

    // <project>/assets/games/<name> -> <project>/target/games/lib<name>.so
    let module = games_dir
        .parent()
        .and_then(|p| p.parent())
        .map(|project| project.join(format!("target/games/lib{name}.so")))
        .filter(|p| p.exists());

    Ok(GameEntry {
        name: name.to_string(),
        dir: dir.to_path_buf(),
        game,
        scene,
        module,
        canvas,
    })
}

fn read_ron<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let raw =
        std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    ron::from_str(&raw).with_context(|| format!("parse {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `<tmp>/assets/games/<name>/`, so the module path derivation has the
    /// directory depth it expects.
    fn game_dir(tmp: &Path, name: &str) -> PathBuf {
        let dir = tmp.join("assets/games").join(name);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("world.ron"),
            r#"( name: "w", nodes: [ ( name: "a" ) ] )"#,
        )
        .unwrap();
        dir
    }

    const WORLD_ROOT: &str = r#"(
        name: "w", input: "input.tres.ron",
        views: { "main": (
            unit: World("world.ron"), graphics: View3D,
            camera: ( projection: Perspective( fov_y_degrees: 60.0, znear: 0.1, zfar: 100.0 ) ),
        ) },
    )"#;

    const CANVAS_ROOT: &str = r#"(
        name: "c", input: "input.tres.ron",
        views: {
            "main": ( unit: Canvas("canvas.ron"), graphics: View2D, camera: ( projection: Screen ) ),
            "game": ( unit: World("world.ron"), graphics: View2D,
                      camera: ( projection: Orthographic( half_height: 2.0, znear: 0.1, zfar: 50.0 ) ) ),
        },
    )"#;

    #[test]
    fn a_world_rooted_game_has_no_canvas() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = game_dir(tmp.path(), "solo");
        std::fs::write(dir.join("game.ron"), WORLD_ROOT).unwrap();

        let loader = Loader::scan(&tmp.path().join("assets/games")).unwrap();
        let g = loader.current_game().unwrap();
        assert_eq!(g.name, "solo");
        assert!(g.canvas.is_none(), "a 3D world needs no screen over it");
        assert_eq!(g.scene.nodes.len(), 1);
    }

    #[test]
    fn a_canvas_rooted_game_loads_both_files() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = game_dir(tmp.path(), "ui");
        std::fs::write(dir.join("game.ron"), CANVAS_ROOT).unwrap();
        std::fs::write(
            dir.join("canvas.ron"),
            r#"( name: "c", nodes: [ ( name: "viewport", rect: ( fill: true ) ) ] )"#,
        )
        .unwrap();

        let loader = Loader::scan(&tmp.path().join("assets/games")).unwrap();
        let g = loader.current_game().unwrap();
        assert!(g.canvas.is_some());
        assert_eq!(g.scene.name, "w");
    }

    /// The projection reaches the viewport through the view, so the view has to
    /// be findable by name.
    #[test]
    fn a_named_view_gives_its_own_camera() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = game_dir(tmp.path(), "ui");
        std::fs::write(dir.join("game.ron"), CANVAS_ROOT).unwrap();
        std::fs::write(
            dir.join("canvas.ron"),
            r#"( name: "c", nodes: [ ( name: "viewport", rect: ( fill: true ) ) ] )"#,
        )
        .unwrap();

        let loader = Loader::scan(&tmp.path().join("assets/games")).unwrap();
        let g = loader.current_game().unwrap();
        assert!(matches!(
            g.camera("game").unwrap().projection,
            scene::Projection::Orthographic { .. }
        ));
        assert!(matches!(
            g.camera("main").unwrap().projection,
            scene::Projection::Screen
        ));
        // An unknown view falls back to the root rather than drawing nothing.
        assert!(matches!(
            g.camera("nope").unwrap().projection,
            scene::Projection::Screen
        ));
    }

    #[test]
    fn a_game_whose_canvas_is_missing_is_skipped_not_fatal() {
        let tmp = tempfile::tempdir().unwrap();
        let bad = game_dir(tmp.path(), "broken");
        std::fs::write(bad.join("game.ron"), CANVAS_ROOT).unwrap();
        // no canvas.ron
        let good = game_dir(tmp.path(), "fine");
        std::fs::write(good.join("game.ron"), WORLD_ROOT).unwrap();

        let loader = Loader::scan(&tmp.path().join("assets/games")).unwrap();
        assert_eq!(loader.game_count(), 1);
        assert_eq!(loader.current_game().unwrap().name, "fine");
    }

    #[test]
    fn a_game_with_no_main_view_is_skipped() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = game_dir(tmp.path(), "nameless");
        std::fs::write(
            dir.join("game.ron"),
            r#"( name: "n", input: "i.ron", views: {
                "game": ( unit: World("world.ron"), graphics: View3D,
                          camera: ( projection: Screen ) ) } )"#,
        )
        .unwrap();
        let loader = Loader::scan(&tmp.path().join("assets/games")).unwrap();
        assert_eq!(loader.game_count(), 0);
    }
}
