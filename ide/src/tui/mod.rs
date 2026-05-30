pub mod hierarchy;
pub mod inspector;
pub mod layout;
pub mod project;
pub mod terminal;
pub mod viewport;

use std::io;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers, MouseEventKind, EnableMouseCapture, DisableMouseCapture};
use crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen, enable_raw_mode, disable_raw_mode};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use crate::config::{Action, Config};
use crate::core::app::{AppState, PanelId};
use self::terminal::EmbeddedTerminal;
use self::viewport::Viewport;

struct TerminalGuard;

impl TerminalGuard {
    fn new() -> anyhow::Result<Self> {
        enable_raw_mode()?;
        crossterm::execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = crossterm::execute!(io::stdout(), DisableMouseCapture, LeaveAlternateScreen);
        let _ = disable_raw_mode();
    }
}

pub fn run(config: Config) -> anyhow::Result<()> {
    let _guard = TerminalGuard::new()?;

    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    let games_dir = std::path::Path::new("assets/games");
    let mut app = AppState::new(games_dir)?;
    let mut viewport = Viewport::new();
    let mut panel_areas: Option<layout::PanelAreas> = None;

    // Initial draw to determine actual panel sizes, then resize the PTY
    // before bash outputs anything meaningful.
    terminal.draw(|f| {
        panel_areas = Some(layout::draw(f, &app, &mut viewport));
    })?;
    if let Some(ref areas) = panel_areas {
        let inner_h = areas.terminal.height.saturating_sub(2);
        let inner_w = areas.terminal.width.saturating_sub(2);
        if inner_h > 0 && inner_w > 0 {
            app.terminal.resize(inner_h, inner_w);
            // Clear screen so bash redraws at the correct size
            app.terminal.send_key(b"\x0c");
        }
    }

    while app.running {
        if let Some(ref areas) = panel_areas {
            let inner_h = areas.terminal.height.saturating_sub(2);
            let inner_w = areas.terminal.width.saturating_sub(2);
            if inner_h > 0 && inner_w > 0 {
                app.terminal.resize(inner_h, inner_w);
            }
        }

        terminal.draw(|f| {
            panel_areas = Some(layout::draw(f, &app, &mut viewport));
        })?;

        if event::poll(Duration::from_millis(50))? {
            match event::read()? {
                Event::Key(key) => {
                    if let Some(action) = config.resolve_action(&key) {
                        handle_action(&mut app, action);
                    } else if key.code == KeyCode::Char('s') && key.modifiers.contains(KeyModifiers::CONTROL) {
                        app.save_scene();
                    } else if app.focused == PanelId::Terminal {
                        send_key_to_pty(&mut app.terminal, &key);
                    } else {
                        app.handle_key(key);
                    }
                }
                Event::Mouse(mouse) => {
                    if let MouseEventKind::Down(_) = mouse.kind {
                        if let Some(ref areas) = panel_areas {
                            if let Some(panel) = areas.panel_at(mouse.column, mouse.row) {
                                app.focused = panel;
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    Ok(())
}

pub(crate) fn handle_action(app: &mut AppState, action: Action) {
    match action {
        Action::FocusHierarchy => app.focused = PanelId::Hierarchy,
        Action::FocusViewport => app.focused = PanelId::Viewport,
        Action::FocusInspector => app.focused = PanelId::Inspector,
        Action::FocusProject => app.focused = PanelId::Project,
        Action::FocusTerminal => app.focused = PanelId::Terminal,
        Action::Quit => app.running = false,
    }
}

fn send_key_to_pty(terminal: &mut EmbeddedTerminal, key: &KeyEvent) {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        if let KeyCode::Char(c) = key.code {
            let ctrl_byte = (c as u8) & 0x1f;
            terminal.send_key(&[ctrl_byte]);
        }
        return;
    }
    match key.code {
        KeyCode::Char(c) => terminal.send_key(c.to_string().as_bytes()),
        KeyCode::Enter => terminal.send_key(b"\r"),
        KeyCode::Backspace => terminal.send_key(b"\x7f"),
        KeyCode::Tab => terminal.send_key(b"\t"),
        KeyCode::Esc => terminal.send_key(b"\x1b"),
        KeyCode::Up => terminal.send_key(b"\x1b[A"),
        KeyCode::Down => terminal.send_key(b"\x1b[B"),
        KeyCode::Right => terminal.send_key(b"\x1b[C"),
        KeyCode::Left => terminal.send_key(b"\x1b[D"),
        _ => {}
    }
}
