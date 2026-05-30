use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    FocusHierarchy,
    FocusViewport,
    FocusInspector,
    FocusProject,
    FocusTerminal,
    Quit,
}

impl Action {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "focus_hierarchy" => Some(Self::FocusHierarchy),
            "focus_viewport" => Some(Self::FocusViewport),
            "focus_inspector" => Some(Self::FocusInspector),
            "focus_project" => Some(Self::FocusProject),
            "focus_terminal" => Some(Self::FocusTerminal),
            "quit" => Some(Self::Quit),
            _ => None,
        }
    }

}

fn key_to_string(key: &KeyEvent) -> String {
    let mut parts = Vec::new();
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        parts.push("ctrl".to_string());
    }
    if key.modifiers.contains(KeyModifiers::ALT) {
        parts.push("alt".to_string());
    }
    match key.code {
        KeyCode::Char(c) => parts.push(c.to_ascii_lowercase().to_string()),
        KeyCode::Enter => parts.push("enter".to_string()),
        KeyCode::Esc => parts.push("esc".to_string()),
        KeyCode::Tab => parts.push("tab".to_string()),
        KeyCode::Backspace => parts.push("backspace".to_string()),
        KeyCode::Up => parts.push("up".to_string()),
        KeyCode::Down => parts.push("down".to_string()),
        KeyCode::Left => parts.push("left".to_string()),
        KeyCode::Right => parts.push("right".to_string()),
        _ => {}
    }
    parts.join("+")
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub mode: Mode,
    pub keybindings: HashMap<String, String>,
}

#[derive(Debug, Deserialize, Default)]
pub enum Mode {
    #[default]
    Tui,
    Gui,
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        let path = std::path::Path::new("ide.ron");
        if path.exists() {
            let text = std::fs::read_to_string(path)?;
            Ok(ron::from_str(&text)?)
        } else {
            Ok(Self::default())
        }
    }

    pub fn resolve_action(&self, key: &KeyEvent) -> Option<Action> {
        let key_str = key_to_string(key);
        self.keybindings
            .get(&key_str)
            .and_then(|action| Action::from_str(action))
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            mode: Mode::Tui,
            keybindings: HashMap::from([
                ("ctrl+h".into(), "focus_hierarchy".into()),
                ("ctrl+v".into(), "focus_viewport".into()),
                ("ctrl+i".into(), "focus_inspector".into()),
                ("ctrl+f".into(), "focus_project".into()),
                ("ctrl+t".into(), "focus_terminal".into()),
                ("q".into(), "quit".into()),
            ]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn ctrl_key(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
    }

    fn plain_key(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
    }

    #[test]
    fn ctrl_h_resolves_to_focus_hierarchy() {
        let cfg = Config::default();
        assert_eq!(cfg.resolve_action(&ctrl_key('h')), Some(Action::FocusHierarchy));
    }

    #[test]
    fn ctrl_v_resolves_to_focus_viewport() {
        let cfg = Config::default();
        assert_eq!(cfg.resolve_action(&ctrl_key('v')), Some(Action::FocusViewport));
    }

    #[test]
    fn ctrl_i_resolves_to_focus_inspector() {
        let cfg = Config::default();
        assert_eq!(cfg.resolve_action(&ctrl_key('i')), Some(Action::FocusInspector));
    }

    #[test]
    fn ctrl_f_resolves_to_focus_project() {
        let cfg = Config::default();
        assert_eq!(cfg.resolve_action(&ctrl_key('f')), Some(Action::FocusProject));
    }

    #[test]
    fn ctrl_t_resolves_to_focus_terminal() {
        let cfg = Config::default();
        assert_eq!(cfg.resolve_action(&ctrl_key('t')), Some(Action::FocusTerminal));
    }

    #[test]
    fn q_resolves_to_quit() {
        let cfg = Config::default();
        assert_eq!(cfg.resolve_action(&plain_key('q')), Some(Action::Quit));
    }

    #[test]
    fn unbound_key_resolves_to_none() {
        let cfg = Config::default();
        assert_eq!(cfg.resolve_action(&plain_key('x')), None);
        assert_eq!(cfg.resolve_action(&ctrl_key('z')), None);
    }

    #[test]
    fn custom_keybindings_override_defaults() {
        let cfg = Config {
            mode: Mode::Tui,
            keybindings: HashMap::from([
                ("ctrl+a".into(), "focus_hierarchy".into()),
            ]),
        };
        assert_eq!(cfg.resolve_action(&ctrl_key('a')), Some(Action::FocusHierarchy));
        assert_eq!(cfg.resolve_action(&ctrl_key('h')), None);
    }

    #[test]
    fn key_to_string_formats_ctrl_combos() {
        assert_eq!(key_to_string(&ctrl_key('h')), "ctrl+h");
        assert_eq!(key_to_string(&ctrl_key('f')), "ctrl+f");
        assert_eq!(key_to_string(&plain_key('q')), "q");
    }

    #[test]
    fn key_to_string_formats_special_keys() {
        let tab = KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE);
        assert_eq!(key_to_string(&tab), "tab");
        let esc = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        assert_eq!(key_to_string(&esc), "esc");
        let up = KeyEvent::new(KeyCode::Up, KeyModifiers::NONE);
        assert_eq!(key_to_string(&up), "up");
    }

    #[test]
    fn action_from_str_round_trips() {
        let cases = [
            ("focus_hierarchy", Action::FocusHierarchy),
            ("focus_viewport", Action::FocusViewport),
            ("focus_inspector", Action::FocusInspector),
            ("focus_project", Action::FocusProject),
            ("focus_terminal", Action::FocusTerminal),
            ("quit", Action::Quit),
        ];
        for (s, expected) in cases {
            assert_eq!(Action::from_str(s), Some(expected), "failed for {s}");
        }
        assert_eq!(Action::from_str("nonexistent"), None);
    }
}
