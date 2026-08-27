//! Compositing a canvas into a cell grid.
//!
//! A canvas is the screen: its nodes are placed by layout rule, and one of them
//! usually shows a view of a world. UI is not layered on afterwards — it is a
//! sibling of the game picture.

use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span, Text};
use scene::{Canvas, CanvasNode, Rect, RectAnchor, TexSource};
use shinra_engine::textart::TextCell;

/// A rectangle in cells, resolved from a layout rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Area {
    pub x: u16,
    pub y: u16,
    pub w: u16,
    pub h: u16,
}

impl Area {
    pub fn new(w: u16, h: u16) -> Self {
        Self { x: 0, y: 0, w, h }
    }
}

/// Place a node inside its parent.
///
/// `fill` takes the whole parent and ignores everything else. Otherwise the
/// anchor picks a point in the parent, the node's size is placed against it so
/// it stays inside, and `offset` nudges it in cells.
pub fn resolve(rect: &Rect, parent: Area) -> Area {
    if rect.fill {
        return parent;
    }
    let (w, h) = match rect.size {
        Some([w, h]) => (w.min(parent.w as u32) as u16, h.min(parent.h as u32) as u16),
        None => (parent.w, 1),
    };
    let (fx, fy) = rect.anchor.fractions();
    // The anchor point in the parent, then back off by the same fraction of the
    // node so an anchored corner lines up with the parent's corner.
    let ax = parent.x as f32 + (parent.w as f32 - w as f32) * fx;
    let ay = parent.y as f32 + (parent.h as f32 - h as f32) * fy;
    // Clamp against the parent's absolute edges, not its size: a child of a
    // parent that itself sits low on the screen belongs down there too.
    let lo_x = parent.x as i32;
    let lo_y = parent.y as i32;
    let hi_x = ((parent.x + parent.w).saturating_sub(w) as i32).max(lo_x);
    let hi_y = ((parent.y + parent.h).saturating_sub(h) as i32).max(lo_y);
    let x = (ax.round() as i32 + rect.offset[0]).clamp(lo_x, hi_x);
    let y = (ay.round() as i32 + rect.offset[1]).clamp(lo_y, hi_y);
    Area {
        x: x as u16,
        y: y as u16,
        w,
        h,
    }
}

/// A grid of cells, composited into and then handed to ratatui.
pub struct CellBuf {
    cols: u16,
    rows: u16,
    cells: Vec<TextCell>,
}

impl CellBuf {
    pub fn new(cols: u16, rows: u16) -> Self {
        Self {
            cols,
            rows,
            cells: vec![
                TextCell {
                    ch: ' ',
                    rgb: [0, 0, 0]
                };
                cols as usize * rows as usize
            ],
        }
    }

    fn put(&mut self, x: u16, y: u16, cell: TextCell) {
        if x < self.cols && y < self.rows {
            self.cells[y as usize * self.cols as usize + x as usize] = cell;
        }
    }

    /// Draw rows of cells at an area's top left, clipped to the area.
    pub fn blit(&mut self, area: Area, rows: &[Vec<TextCell>]) {
        for (dy, row) in rows.iter().take(area.h as usize).enumerate() {
            for (dx, cell) in row.iter().take(area.w as usize).enumerate() {
                self.put(area.x + dx as u16, area.y + dy as u16, *cell);
            }
        }
    }

    /// Fill an area with one colour, as a block character.
    pub fn fill(&mut self, area: Area, rgb: [u8; 3]) {
        for dy in 0..area.h {
            for dx in 0..area.w {
                self.put(area.x + dx, area.y + dy, TextCell { ch: '█', rgb });
            }
        }
    }

    /// Draw a string at an area's top left, clipped to its width.
    pub fn write(&mut self, area: Area, text: &str, rgb: [u8; 3]) {
        for (dx, ch) in text.chars().take(area.w as usize).enumerate() {
            self.put(area.x + dx as u16, area.y, TextCell { ch, rgb });
        }
    }

    pub fn to_text(&self) -> Text<'static> {
        let lines: Vec<Line> = (0..self.rows)
            .map(|y| {
                let mut spans: Vec<Span> = Vec::new();
                let mut run = String::new();
                let mut run_rgb = [0u8; 3];
                for x in 0..self.cols {
                    let cell = self.cells[y as usize * self.cols as usize + x as usize];
                    if cell.rgb != run_rgb && !run.is_empty() {
                        spans.push(styled(std::mem::take(&mut run), run_rgb));
                    }
                    run_rgb = cell.rgb;
                    run.push(cell.ch);
                }
                if !run.is_empty() {
                    spans.push(styled(run, run_rgb));
                }
                Line::from(spans)
            })
            .collect();
        Text::from(lines)
    }
}

fn styled(s: String, [r, g, b]: [u8; 3]) -> Span<'static> {
    Span::styled(s, Style::default().fg(Color::Rgb(r, g, b)))
}

fn to_rgb(color: [f32; 4]) -> [u8; 3] {
    [
        (color[0].clamp(0.0, 1.0) * 255.0) as u8,
        (color[1].clamp(0.0, 1.0) * 255.0) as u8,
        (color[2].clamp(0.0, 1.0) * 255.0) as u8,
    ]
}

/// What a canvas needs from its host to draw a node.
pub trait CanvasHost {
    /// Render a named view into cells, at most `w` by `h`.
    fn view(&mut self, name: &str, w: u16, h: u16) -> Vec<Vec<TextCell>>;
    /// Resolve a `text.from` path such as `Run.crashes` to a display string.
    fn field(&self, path: &str) -> Option<String>;
}

/// Composite a canvas, depth first, parents before children.
pub fn composite(canvas: &Canvas, host: &mut impl CanvasHost, cols: u16, rows: u16) -> Text<'static> {
    let mut buf = CellBuf::new(cols, rows);
    for node in &canvas.nodes {
        draw(node, host, &mut buf, Area::new(cols, rows));
    }
    buf.to_text()
}

fn draw(node: &CanvasNode, host: &mut impl CanvasHost, buf: &mut CellBuf, parent: Area) {
    let area = resolve(&node.rect, parent);

    if let Some(c) = &node.color_rect {
        buf.fill(area, to_rgb(c.color));
    }
    if let Some(sprite) = &node.sprite {
        match &sprite.source {
            TexSource::View(name) => {
                let rows = host.view(name, area.w, area.h);
                buf.blit(area, &rows);
            }
            // An image in a cell grid would have to go through a view that
            // converts it; a canvas does not rasterise on its own.
            TexSource::Png(_) => {}
        }
    }
    if let Some(text) = &node.text {
        let value = text
            .from
            .as_deref()
            .and_then(|p| host.field(p))
            .or_else(|| text.literal.clone())
            .unwrap_or_default();
        let shown = text.format.replacen("{}", &value, 1);
        buf.write(area, &shown, [220, 220, 220]);
    }
    for child in &node.children {
        draw(child, host, buf, area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scene::{CanvasSprite, ColorRect, Text as CanvasText};

    fn rect(anchor: RectAnchor, size: Option<[u32; 2]>, offset: [i32; 2]) -> Rect {
        Rect {
            anchor,
            offset,
            size,
            fill: false,
        }
    }

    // ── layout ──────────────────────────────────────────────────────────────

    #[test]
    fn fill_takes_the_whole_parent() {
        let r = Rect {
            fill: true,
            ..Default::default()
        };
        assert_eq!(resolve(&r, Area::new(80, 24)), Area::new(80, 24));
    }

    #[test]
    fn top_left_sits_at_the_origin() {
        let a = resolve(&rect(RectAnchor::TopLeft, Some([10, 2]), [0, 0]), Area::new(80, 24));
        assert_eq!((a.x, a.y, a.w, a.h), (0, 0, 10, 2));
    }

    #[test]
    fn top_right_lines_up_with_the_parents_right_edge() {
        let a = resolve(&rect(RectAnchor::TopRight, Some([10, 1]), [0, 0]), Area::new(80, 24));
        assert_eq!((a.x, a.y), (70, 0), "10 wide against an 80 wide parent");
    }

    #[test]
    fn bottom_right_lines_up_with_both_far_edges() {
        let a = resolve(&rect(RectAnchor::BottomRight, Some([10, 2]), [0, 0]), Area::new(80, 24));
        assert_eq!((a.x, a.y), (70, 22));
    }

    #[test]
    fn center_splits_the_remainder() {
        let a = resolve(&rect(RectAnchor::Center, Some([20, 4]), [0, 0]), Area::new(80, 24));
        assert_eq!((a.x, a.y), (30, 10));
    }

    #[test]
    fn offset_nudges_in_cells() {
        let a = resolve(&rect(RectAnchor::TopRight, Some([10, 1]), [-2, 1]), Area::new(80, 24));
        assert_eq!((a.x, a.y), (68, 1));
    }

    #[test]
    fn a_node_wider_than_its_parent_is_clamped() {
        let a = resolve(&rect(RectAnchor::TopLeft, Some([200, 100]), [0, 0]), Area::new(80, 24));
        assert_eq!((a.w, a.h), (80, 24));
    }

    #[test]
    fn no_size_spans_the_parent_on_one_row() {
        let a = resolve(&rect(RectAnchor::TopLeft, None, [0, 0]), Area::new(80, 24));
        assert_eq!((a.w, a.h), (80, 1));
    }

    // ── compositing ─────────────────────────────────────────────────────────

    struct Host {
        asked: Vec<String>,
    }

    impl CanvasHost for Host {
        fn view(&mut self, name: &str, w: u16, h: u16) -> Vec<Vec<TextCell>> {
            self.asked.push(name.to_string());
            vec![
                vec![
                    TextCell {
                        ch: '#',
                        rgb: [1, 2, 3]
                    };
                    w as usize
                ];
                h as usize
            ]
        }

        fn field(&self, path: &str) -> Option<String> {
            (path == "Run.crashes").then(|| "7".to_string())
        }
    }

    fn plain(text: &Text<'_>) -> Vec<String> {
        text.lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect()
    }

    #[test]
    fn a_view_node_is_asked_for_its_view_and_blitted() {
        let canvas = Canvas {
            name: "c".into(),
            nodes: vec![CanvasNode {
                name: "viewport".into(),
                rect: Rect {
                    fill: true,
                    ..Default::default()
                },
                sprite: Some(CanvasSprite {
                    source: TexSource::View("game".into()),
                }),
                ..Default::default()
            }],
        };
        let mut host = Host { asked: vec![] };
        let text = composite(&canvas, &mut host, 4, 2);
        assert_eq!(host.asked, vec!["game"]);
        assert_eq!(plain(&text), vec!["####", "####"]);
    }

    #[test]
    fn text_reads_a_component_field_through_its_format() {
        let canvas = Canvas {
            name: "c".into(),
            nodes: vec![CanvasNode {
                name: "score".into(),
                rect: rect(RectAnchor::TopLeft, Some([12, 1]), [0, 0]),
                text: Some(CanvasText {
                    from: Some("Run.crashes".into()),
                    literal: None,
                    format: "crashes: {}".into(),
                }),
                ..Default::default()
            }],
        };
        let mut host = Host { asked: vec![] };
        let text = composite(&canvas, &mut host, 12, 1);
        assert_eq!(plain(&text), vec!["crashes: 7  "], "the row is padded to the buffer width");
    }

    #[test]
    fn text_falls_back_to_its_literal() {
        let canvas = Canvas {
            name: "c".into(),
            nodes: vec![CanvasNode {
                name: "t".into(),
                rect: rect(RectAnchor::TopLeft, Some([6, 1]), [0, 0]),
                text: Some(CanvasText {
                    from: Some("Nope.nope".into()),
                    literal: Some("idle".into()),
                    format: "{}".into(),
                }),
                ..Default::default()
            }],
        };
        let mut host = Host { asked: vec![] };
        assert_eq!(plain(&composite(&canvas, &mut host, 6, 1)), vec!["idle  "]);
    }

    /// A later sibling draws over an earlier one, so UI sits on the picture.
    #[test]
    fn siblings_composite_in_order() {
        let canvas = Canvas {
            name: "c".into(),
            nodes: vec![
                CanvasNode {
                    name: "picture".into(),
                    rect: Rect {
                        fill: true,
                        ..Default::default()
                    },
                    sprite: Some(CanvasSprite {
                        source: TexSource::View("game".into()),
                    }),
                    ..Default::default()
                },
                CanvasNode {
                    name: "hud".into(),
                    rect: rect(RectAnchor::TopLeft, Some([2, 1]), [0, 0]),
                    text: Some(CanvasText {
                        from: None,
                        literal: Some("hi".into()),
                        format: "{}".into(),
                    }),
                    ..Default::default()
                },
            ],
        };
        let mut host = Host { asked: vec![] };
        assert_eq!(plain(&composite(&canvas, &mut host, 4, 2)), vec!["hi##", "####"]);
    }

    #[test]
    fn a_color_rect_fills_its_area() {
        let canvas = Canvas {
            name: "c".into(),
            nodes: vec![CanvasNode {
                name: "bg".into(),
                rect: rect(RectAnchor::TopLeft, Some([2, 1]), [0, 0]),
                color_rect: Some(ColorRect {
                    color: [1.0, 0.0, 0.0, 1.0],
                }),
                ..Default::default()
            }],
        };
        let mut host = Host { asked: vec![] };
        assert_eq!(plain(&composite(&canvas, &mut host, 3, 1)), vec!["██ "]);
    }

    /// A child of a parent low on the screen stays down there: the clamp is
    /// against the parent's edges, not its size.
    #[test]
    fn a_child_stays_inside_a_parent_that_is_not_at_the_origin() {
        let parent = Area { x: 10, y: 16, w: 64, h: 4 };
        let a = resolve(&rect(RectAnchor::TopLeft, Some([20, 1]), [2, 1]), parent);
        assert_eq!((a.x, a.y), (12, 17));
    }

    #[test]
    fn a_child_offset_past_its_parent_is_pulled_back_in() {
        let parent = Area { x: 10, y: 16, w: 64, h: 4 };
        let a = resolve(&rect(RectAnchor::TopLeft, Some([20, 1]), [100, 100]), parent);
        assert_eq!((a.x, a.y), (54, 19), "the far edge, minus the node's size");
    }

    #[test]
    fn a_child_is_placed_inside_its_parent() {
        let canvas = Canvas {
            name: "c".into(),
            nodes: vec![CanvasNode {
                name: "panel".into(),
                rect: rect(RectAnchor::TopLeft, Some([4, 2]), [2, 0]),
                children: vec![CanvasNode {
                    name: "label".into(),
                    rect: rect(RectAnchor::TopLeft, Some([2, 1]), [0, 0]),
                    text: Some(CanvasText {
                        from: None,
                        literal: Some("ab".into()),
                        format: "{}".into(),
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            }],
        };
        let mut host = Host { asked: vec![] };
        assert_eq!(plain(&composite(&canvas, &mut host, 6, 1)), vec!["  ab  "]);
    }
}
