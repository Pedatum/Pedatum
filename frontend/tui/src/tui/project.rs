use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};

use crate::core::project::ProjectState;

pub fn draw(f: &mut Frame, area: Rect, state: &ProjectState, focused: bool) {
    let visible = state.visible_entries();
    let selected_pos = visible.iter().position(|&i| i == state.selected);
    let items: Vec<ListItem> = visible
        .iter()
        .map(|&i| {
            let entry = &state.entries[i];
            let indent = "  ".repeat(entry.depth);
            let prefix = if entry.is_dir {
                if entry.expanded { "v " } else { "> " }
            } else {
                "- "
            };
            let text = format!("{}{}{}", indent, prefix, entry.name);
            let style = if i == state.selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(text).style(style)
        })
        .collect();

    let border_style = if focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };
    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Project")
            .border_style(border_style),
    );
    let mut list_state = ListState::default().with_selected(selected_pos);
    f.render_stateful_widget(list, area, &mut list_state);
}
