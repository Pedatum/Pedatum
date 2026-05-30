use std::path::Path;

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use scene::Scene;

use super::hierarchy::HierarchyState;
use super::inspector::InspectorState;
use super::loader::Loader;
use super::project::ProjectState;
use crate::tui::terminal::EmbeddedTerminal;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelId {
    Hierarchy,
    Viewport,
    Inspector,
    Project,
    Terminal,
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
    pub loader: Loader,
    pub hierarchy: HierarchyState,
    pub selected_node: Option<usize>,
    pub inspector: InspectorState,
    pub project: ProjectState,
    pub terminal: EmbeddedTerminal,
}

impl AppState {
    pub fn new(games_dir: &Path) -> Result<Self> {
        let loader = Loader::scan(games_dir)?;
        let hierarchy = loader
            .current_game()
            .map(|g| HierarchyState::from_scene(&g.scene))
            .unwrap_or_else(|| HierarchyState::from_scene(&Scene::default()));
        let selected_node = hierarchy.selected_node_index();
        let inspector = Self::build_inspector(loader.current_game().map(|g| &g.scene), selected_node);
        let project_root = std::env::current_dir().unwrap_or_else(|_| games_dir.to_path_buf());
        let project = ProjectState::scan(&project_root);
        let terminal = EmbeddedTerminal::new(80, 24)?;
        Ok(Self {
            running: true,
            focused: PanelId::Viewport,
            loader,
            hierarchy,
            selected_node,
            inspector,
            project,
            terminal,
        })
    }

    pub fn current_scene(&self) -> Option<&Scene> {
        self.loader.current_game().map(|g| &g.scene)
    }

    fn build_inspector(scene: Option<&Scene>, selected: Option<usize>) -> InspectorState {
        match (scene, selected) {
            (Some(s), Some(idx)) if idx < s.nodes.len() => {
                InspectorState::from_node(&s.nodes[idx])
            }
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
        match key.code {
            KeyCode::Esc
                if !(self.focused == PanelId::Inspector && self.inspector.editing) =>
            {
                self.running = false;
            }
            KeyCode::Char('n') => {
                self.loader.next_game();
                self.hierarchy = self
                    .current_scene()
                    .map(|s| HierarchyState::from_scene(s))
                    .unwrap_or_else(|| HierarchyState::from_scene(&Scene::default()));
                self.selected_node = self.hierarchy.selected_node_index();
                self.sync_inspector();
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
