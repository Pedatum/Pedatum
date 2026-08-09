use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Text};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use unicode_width::UnicodeWidthStr;

use crate::core::overlay::{OverlayAnchor, OverlayState, TextBoxOverlay};

const VIEWPORT_MARGIN: u16 = 1;
const MIN_BOX_SIZE: u16 = 3;

pub fn draw(f: &mut Frame, viewport: Rect, overlays: &OverlayState) {
    for text_box in overlays.iter() {
        let Some(area) = text_box_rect(viewport, text_box) else {
            continue;
        };

        // Clear makes the overlay opaque over both text-art and image modes.
        f.render_widget(Clear, area);
        let mut block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow))
            .style(Style::default().fg(Color::White).bg(Color::Black));
        if let Some(title) = &text_box.title {
            block = block.title(format!(" {title} "));
        }

        let text = Text::from(
            text_box
                .lines
                .iter()
                .map(|line| Line::raw(line.clone()))
                .collect::<Vec<_>>(),
        );
        f.render_widget(
            Paragraph::new(text).block(block).wrap(Wrap { trim: false }),
            area,
        );
    }
}

fn text_box_rect(viewport: Rect, text_box: &TextBoxOverlay) -> Option<Rect> {
    let available_width = viewport.width.saturating_sub(VIEWPORT_MARGIN * 2);
    let available_height = viewport.height.saturating_sub(VIEWPORT_MARGIN * 2);
    if available_width < MIN_BOX_SIZE || available_height < MIN_BOX_SIZE {
        return None;
    }

    let width = text_box.width.clamp(MIN_BOX_SIZE, available_width);
    let content_width = width.saturating_sub(2).max(1) as usize;
    let content_height = text_box
        .lines
        .iter()
        .map(|line| wrapped_line_count(line, content_width))
        .sum::<usize>()
        .max(1) as u16;
    let requested_height = content_height.saturating_add(2);
    let maximum_height = text_box
        .max_height
        .unwrap_or(available_height)
        .clamp(MIN_BOX_SIZE, available_height);
    let height = requested_height.clamp(MIN_BOX_SIZE, maximum_height);

    let left = viewport.x + VIEWPORT_MARGIN;
    let top = viewport.y + VIEWPORT_MARGIN;
    let right = viewport.right().saturating_sub(VIEWPORT_MARGIN + width);
    let bottom = viewport.bottom().saturating_sub(VIEWPORT_MARGIN + height);
    let center_x = viewport.x + viewport.width.saturating_sub(width) / 2;
    let center_y = viewport.y + viewport.height.saturating_sub(height) / 2;

    let (x, y) = match text_box.anchor {
        OverlayAnchor::TopLeft => (left, top),
        OverlayAnchor::TopRight => (right, top),
        OverlayAnchor::BottomLeft => (left, bottom),
        OverlayAnchor::BottomRight => (right, bottom),
        OverlayAnchor::Center => (center_x, center_y),
    };

    Some(Rect::new(x, y, width, height))
}

fn wrapped_line_count(line: &str, content_width: usize) -> usize {
    line.split('\n')
        .map(|part| part.width().max(1).div_ceil(content_width))
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bottom_right_box_is_anchored_inside_viewport_margin() {
        let viewport = Rect::new(10, 5, 60, 20);
        let text_box = TextBoxOverlay::new("info", ["hello"])
            .anchor(OverlayAnchor::BottomRight)
            .width(20);

        assert_eq!(
            text_box_rect(viewport, &text_box),
            Some(Rect::new(49, 21, 20, 3))
        );
    }

    #[test]
    fn box_is_constrained_to_a_small_viewport() {
        let viewport = Rect::new(0, 0, 12, 6);
        let text_box = TextBoxOverlay::new("info", ["a long line of text"])
            .anchor(OverlayAnchor::Center)
            .width(80)
            .max_height(40);

        assert_eq!(
            text_box_rect(viewport, &text_box),
            Some(Rect::new(1, 1, 10, 4))
        );
    }

    #[test]
    fn wrapping_uses_display_width_for_cjk_text() {
        assert_eq!(wrapped_line_count("矮人要塞", 4), 2);
        assert_eq!(wrapped_line_count("abc", 4), 1);
    }
}
