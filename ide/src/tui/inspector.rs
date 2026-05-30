use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::core::inspector::InspectorState;

pub fn draw(f: &mut Frame, area: Rect, state: &InspectorState, focused: bool) {
    let mut lines = vec![];
    lines.push(Line::from(format!("Name: {}", state.node_name)));
    lines.push(Line::from(""));
    lines.push(Line::from("Position:"));
    for (i, (label, val)) in [
        ("X", state.translation[0]),
        ("Y", state.translation[1]),
        ("Z", state.translation[2]),
    ]
    .iter()
    .enumerate()
    {
        let style = if state.editing && state.field_cursor == i {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        };
        lines.push(Line::from(Span::styled(
            format!("  {}: {:.3}", label, val),
            style,
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from("Rotation:"));
    for (i, (label, val)) in [
        ("X", state.rotation[0]),
        ("Y", state.rotation[1]),
        ("Z", state.rotation[2]),
        ("W", state.rotation[3]),
    ]
    .iter()
    .enumerate()
    {
        let style = if state.editing && state.field_cursor == i + 3 {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        };
        lines.push(Line::from(Span::styled(
            format!("  {}: {:.3}", label, val),
            style,
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from("Scale:"));
    for (i, (label, val)) in [
        ("X", state.scale[0]),
        ("Y", state.scale[1]),
        ("Z", state.scale[2]),
    ]
    .iter()
    .enumerate()
    {
        let style = if state.editing && state.field_cursor == i + 7 {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        };
        lines.push(Line::from(Span::styled(
            format!("  {}: {:.3}", label, val),
            style,
        )));
    }

    if let Some(ref path) = state.mesh_path {
        lines.push(Line::from(""));
        lines.push(Line::from(format!("Mesh: {}", path)));
    }

    let border_style = if focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title("Inspector")
        .border_style(border_style);
    let para = Paragraph::new(lines).block(block);
    f.render_widget(para, area);
}
