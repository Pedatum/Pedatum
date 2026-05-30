use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui_image::StatefulImage;

use crate::core::app::{AppState, PanelId};
use super::viewport::Viewport;

pub struct PanelAreas {
    pub hierarchy: Rect,
    pub viewport: Rect,
    pub inspector: Rect,
    pub project: Rect,
    pub terminal: Rect,
}

impl PanelAreas {
    pub fn panel_at(&self, col: u16, row: u16) -> Option<PanelId> {
        if self.hierarchy.contains((col, row).into()) {
            Some(PanelId::Hierarchy)
        } else if self.viewport.contains((col, row).into()) {
            Some(PanelId::Viewport)
        } else if self.inspector.contains((col, row).into()) {
            Some(PanelId::Inspector)
        } else if self.project.contains((col, row).into()) {
            Some(PanelId::Project)
        } else if self.terminal.contains((col, row).into()) {
            Some(PanelId::Terminal)
        } else {
            None
        }
    }
}

fn panel_block(title: &str, focused: bool) -> Block<'_> {
    let block = Block::default().borders(Borders::ALL).title(title);
    if focused {
        block.border_style(Style::default().fg(Color::Yellow))
    } else {
        block
    }
}

fn draw_menu_bar(f: &mut Frame, area: Rect, app: &AppState) {
    let left = " File  Edit  View  Run";
    let game_info = if let Some(game) = app.loader.current_game() {
        format!(
            " game {}/{}: {}",
            app.loader.current_index() + 1,
            app.loader.game_count(),
            game.name,
        )
    } else {
        " (no games)".into()
    };
    let right_len = game_info.len();
    let padding = (area.width as usize).saturating_sub(left.len() + right_len);
    let line = Line::from(vec![
        Span::raw(left),
        Span::raw(" ".repeat(padding.max(1))),
        Span::raw(game_info),
    ]);
    f.render_widget(Paragraph::new(line), area);
}

pub fn draw(f: &mut Frame, app: &AppState, viewport: &mut Viewport) -> PanelAreas {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(10),
            Constraint::Percentage(30),
        ])
        .split(f.area());

    draw_menu_bar(f, outer[0], app);

    let middle = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Percentage(60),
            Constraint::Percentage(20),
        ])
        .split(outer[1]);

    super::hierarchy::draw(f, middle[0], &app.hierarchy, app.focused == PanelId::Hierarchy);

    let vp_block = panel_block("Viewport", app.focused == PanelId::Viewport);
    let vp_inner = vp_block.inner(middle[1]);
    f.render_widget(vp_block, middle[1]);

    if let Some(game) = app.loader.current_game() {
        viewport.render_scene(&game.scene);
        let img_widget = StatefulImage::default();
        f.render_stateful_widget(img_widget, vp_inner, &mut viewport.image_state);
    }

    super::inspector::draw(f, middle[2], &app.inspector, app.focused == PanelId::Inspector);

    let bottom = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(outer[2]);

    super::project::draw(f, bottom[0], &app.project, app.focused == PanelId::Project);

    super::terminal::draw(f, bottom[1], &app.terminal, app.focused == PanelId::Terminal);

    PanelAreas {
        hierarchy: middle[0],
        viewport: middle[1],
        inspector: middle[2],
        project: bottom[0],
        terminal: bottom[1],
    }
}
