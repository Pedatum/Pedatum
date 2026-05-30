use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem};

use crate::core::hierarchy::HierarchyState;

pub fn draw(f: &mut Frame, area: Rect, state: &HierarchyState, focused: bool) {
    let items: Vec<ListItem> = state
        .entries
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let indent = "  ".repeat(entry.depth);
            let prefix = if entry.has_children {
                if entry.expanded { "v " } else { "> " }
            } else {
                "  "
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
            .title("Hierarchy")
            .border_style(border_style),
    );
    f.render_widget(list, area);
}
