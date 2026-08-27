use std::path::Path;

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use scene::Scene;

use super::hierarchy::HierarchyState;
use super::inspector::InspectorState;
use super::loader::Loader;
use super::overlay::OverlayState;
use super::project::ProjectState;
use super::run::RunState;
use crate::config::ViewportMode;
use crate::tui::terminal::EmbeddedTerminal;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelId {
    Hierarchy,
    Viewport,
    Inspector,
    Project,
    Terminal,
}

/// Translate a terminal key to the code the game ABI shares. The IDE assigns
/// no meaning; that is the game's action map's job.
fn key_code(code: KeyCode) -> Option<u32> {
    use gametok_abi::keys;
    Some(match code {
        KeyCode::Char(' ') => keys::SPACE,
        KeyCode::Char(c) => (c.to_ascii_lowercase() as u32),
        KeyCode::Left => keys::LEFT,
        KeyCode::Right => keys::RIGHT,
        KeyCode::Up => keys::UP,
        KeyCode::Down => keys::DOWN,
        KeyCode::Enter => keys::ENTER,
        _ => return None,
    })
}

const PANEL_ORDER: [PanelId; 5] = [
    PanelId::Hierarchy,
    PanelId::Viewport,
    PanelId::Inspector,
    PanelId::Project,
    PanelId::Terminal,
];

pub struct AppState {
    pub running: bool,
    pub focused: PanelId,
    pub viewport_mode: ViewportMode,
    pub overlays: OverlayState,
    /// `Some` while in running (play) mode.
    pub run: Option<RunState>,
    pub loader: Loader,
    pub hierarchy: HierarchyState,
    pub selected_node: Option<usize>,
    pub inspector: InspectorState,
    pub project: ProjectState,
    pub terminal: Option<EmbeddedTerminal>,
}

impl AppState {
    pub fn new(games_dir: &Path) -> Result<Self> {
        let loader = Loader::scan(games_dir)?;
        let hierarchy = loader
            .current_game()
            .map(|g| HierarchyState::from_scene(&g.scene))
            .unwrap_or_else(|| HierarchyState::from_scene(&Scene::default()));
        let selected_node = hierarchy.selected_node_index();
        let inspector =
            Self::build_inspector(loader.current_game().map(|g| &g.scene), selected_node);
        let project_root = std::env::current_dir().unwrap_or_else(|_| games_dir.to_path_buf());
        let project = ProjectState::scan(&project_root);
        let terminal = EmbeddedTerminal::new(80, 24)?;
        Ok(Self {
            running: true,
            focused: PanelId::Viewport,
            viewport_mode: ViewportMode::default(),
            overlays: OverlayState::default(),
            run: None,
            loader,
            hierarchy,
            selected_node,
            inspector,
            project,
            terminal: Some(terminal),
        })
    }

    pub fn new_headless(games_dir: &Path) -> Result<Self> {
        let loader = Loader::scan(games_dir)?;
        let hierarchy = loader
            .current_game()
            .map(|g| HierarchyState::from_scene(&g.scene))
            .unwrap_or_else(|| HierarchyState::from_scene(&Scene::default()));
        let selected_node = hierarchy.selected_node_index();
        let inspector =
            Self::build_inspector(loader.current_game().map(|g| &g.scene), selected_node);
        let project_root = std::env::current_dir().unwrap_or_else(|_| games_dir.to_path_buf());
        let project = ProjectState::scan(&project_root);
        Ok(Self {
            running: true,
            focused: PanelId::Viewport,
            viewport_mode: ViewportMode::default(),
            overlays: OverlayState::default(),
            run: None,
            loader,
            hierarchy,
            selected_node,
            inspector,
            project,
            terminal: None,
        })
    }

    pub fn current_scene(&self) -> Option<&Scene> {
        self.loader.current_game().map(|g| &g.scene)
    }

    /// The scene the viewport should draw: the live run copy while playing,
    /// the editor scene otherwise.
    pub fn display_scene(&self) -> Option<&Scene> {
        self.run
            .as_ref()
            .map(|r| &r.scene)
            .or_else(|| self.current_scene())
    }

    /// Enter / leave running (play) mode. A game with a built module is driven
    /// by it; one without just ticks a clock.
    pub fn toggle_run(&mut self) {
        if self.run.take().is_none() {
            self.run = self.start_run();
            if self.run.is_some() {
                self.focused = PanelId::Viewport;
            }
        }
    }

    fn start_run(&self) -> Option<RunState> {
        let game = self.loader.current_game()?;
        Some(match &game.module {
            Some(path) => RunState::with_module(game.scene.clone(), path),
            None => RunState::new(game.scene.clone()),
        })
    }

    /// Advance run-mode systems; no-op while editing.
    pub fn tick(&mut self, dt: f32) {
        if let Some(run) = &mut self.run {
            run.tick(dt);
        }
    }

    fn switch_to_next_game(&mut self) {
        self.loader.next_game();
        self.hierarchy = self
            .current_scene()
            .map(HierarchyState::from_scene)
            .unwrap_or_else(|| HierarchyState::from_scene(&Scene::default()));
        self.selected_node = self.hierarchy.selected_node_index();
        self.sync_inspector();
    }

    pub fn save_scene(&self) {
        if let Some(game) = self.loader.current_game() {
            let path = game.dir.join("scene.ron");
            if let Err(e) = game.scene.save(&path) {
                eprintln!("[ide] save failed: {e}");
            }
        }
    }

    fn build_inspector(scene: Option<&Scene>, selected: Option<usize>) -> InspectorState {
        match (scene, selected) {
            (Some(s), Some(idx)) if idx < s.nodes.len() => InspectorState::from_node(&s.nodes[idx]),
            _ => InspectorState::clear(),
        }
    }

    fn sync_inspector(&mut self) {
        self.inspector = Self::build_inspector(self.current_scene(), self.selected_node);
    }

    fn apply_inspector_to_scene(&mut self) {
        if let Some(node_idx) = self.selected_node {
            let t = self.inspector.to_transform();
            if let Some(game) = self.loader.current_game_mut() {
                if node_idx < game.scene.nodes.len() {
                    game.scene.nodes[node_idx].transform = t;
                }
            }
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        // Play mode. Gameplay keys are resolved by the running game's action
        // map, not here; the IDE only keeps the keys that leave or switch a run.
        if self.run.is_some() {
            match key.code {
                KeyCode::Char('n') => {
                    self.switch_to_next_game();
                    self.run = self.start_run();
                }
                KeyCode::Esc => {
                    self.run = None;
                }
                other => {
                    // Everything else is gameplay: hand the raw code over and
                    // let the game's action map decide what it means.
                    if let (Some(run), Some(code)) = (self.run.as_mut(), key_code(other)) {
                        run.key(code);
                    }
                }
            }
            return;
        }

        match key.code {
            KeyCode::Esc if !(self.focused == PanelId::Inspector && self.inspector.editing) => {
                self.running = false;
            }
            KeyCode::Char('n') => {
                self.switch_to_next_game();
            }
            KeyCode::Tab => {
                let idx = PANEL_ORDER
                    .iter()
                    .position(|&p| p == self.focused)
                    .unwrap_or(0);
                self.focused = PANEL_ORDER[(idx + 1) % PANEL_ORDER.len()];
            }
            _ if self.focused == PanelId::Hierarchy => {
                match key.code {
                    KeyCode::Up => self.hierarchy.move_up(),
                    KeyCode::Down => self.hierarchy.move_down(),
                    KeyCode::Enter => self.hierarchy.toggle_expand(),
                    _ => return,
                }
                self.selected_node = self.hierarchy.selected_node_index();
                self.sync_inspector();
            }
            _ if self.focused == PanelId::Inspector => {
                if self.inspector.editing {
                    match key.code {
                        KeyCode::Esc => self.inspector.exit_edit(),
                        KeyCode::Tab => self.inspector.next_field(),
                        KeyCode::Char('+') | KeyCode::Char('=') => {
                            self.inspector.adjust(0.1);
                            self.apply_inspector_to_scene();
                        }
                        KeyCode::Char('-') | KeyCode::Char('_') => {
                            self.inspector.adjust(-0.1);
                            self.apply_inspector_to_scene();
                        }
                        _ => {}
                    }
                } else {
                    match key.code {
                        KeyCode::Char('e') => self.inspector.enter_edit(),
                        _ => {}
                    }
                }
            }
            _ if self.focused == PanelId::Project => match key.code {
                KeyCode::Up => self.project.move_up(),
                KeyCode::Down => self.project.move_down(),
                KeyCode::Enter => self.project.toggle_expand(),
                _ => {}
            },
            _ => {}
        }
    }
}
