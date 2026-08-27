use image::DynamicImage;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span, Text};
use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;
use shinra_engine::engine::Engine;
use shinra_engine::textart::{self, TextArtMode};
use shinra_engine::textart_gpu::{TextArtGpu, DEFAULT_LINEAR_THRESHOLD};

const RENDER_WIDTH: u32 = 256;
const RENDER_HEIGHT: u32 = 192;

/// Terminal cells are roughly twice as tall as wide; used to correct the
/// camera aspect so geometry keeps its proportions in cell space.
const CELL_ASPECT: f32 = 2.0;

pub struct Viewport {
    engine: Engine,
    textart_pass: TextArtGpu,
    picker: Picker,
    pub image_state: StatefulProtocol,
}

impl Viewport {
    pub fn new() -> Self {
        let engine = Engine::new(RENDER_WIDTH, RENDER_HEIGHT);
        let textart_pass = TextArtGpu::new(&engine.device);

        let picker = Picker::from_fontsize((8, 16));
        let blank = DynamicImage::new_rgba8(1, 1);
        let image_state = picker.new_resize_protocol(blank);

        Self {
            engine,
            textart_pass,
            picker,
            image_state,
        }
    }

    /// Image-protocol path (kitty/sixel/halfblocks via ratatui-image).
    pub fn render_scene(&mut self, scene_data: &scene::Scene) {
        if self.engine.size != (RENDER_WIDTH, RENDER_HEIGHT) {
            self.engine.resize(RENDER_WIDTH, RENDER_HEIGHT);
        }
        let aspect = RENDER_WIDTH as f32 / RENDER_HEIGHT as f32;
        self.engine.load_world(scene_data, editor_camera(aspect));
        self.engine.render_current();
        let img = self.engine.frame_image();
        self.image_state = self
            .picker
            .new_resize_protocol(DynamicImage::ImageRgba8(img));
    }

    /// Unicode text-art path: render at 2x4 subpixels per cell, collapse to
    /// packed cells in a GPU compute pass (`textart.wgsl`), then map the
    /// readback ints to colored glyphs on the CPU.
    pub fn render_text(
        &mut self,
        scene_data: &scene::Scene,
        mode: TextArtMode,
        cols: u16,
        rows: u16,
    ) -> Text<'static> {
        let rows_of_cells = self.world_cells(scene_data, mode, cols, rows);
        let lines: Vec<Line> = rows_of_cells.into_iter().map(cells_to_line).collect();
        Text::from(lines)
    }

    /// The world as cells, for either the viewport or a canvas node.
    pub fn world_cells(
        &mut self,
        scene_data: &scene::Scene,
        mode: TextArtMode,
        cols: u16,
        rows: u16,
    ) -> Vec<Vec<textart::TextCell>> {
        let (w, h) = (cols as u32 * 2, rows as u32 * 4);
        if w == 0 || h == 0 {
            return Vec::new();
        }
        if self.engine.size != (w, h) {
            self.engine.resize(w, h);
        }

        // The panel's on-screen aspect, corrected for cells being about twice
        // as tall as wide, so the world is not stretched by the cell grid.
        let visual_aspect = cols as f32 / (rows as f32 * CELL_ASPECT);
        self.engine
            .load_world(scene_data, editor_camera(visual_aspect));
        self.engine.render_current();
        let packed = self.textart_pass.cells(
            &self.engine.device,
            &self.engine.queue,
            &self.engine.color,
            cols as u32,
            rows as u32,
            DEFAULT_LINEAR_THRESHOLD,
        );
        textart::packed_to_cells(&packed, cols as usize, mode)
    }
}

/// Merge runs of same-coloured cells into single spans.
fn cells_to_line(row: Vec<textart::TextCell>) -> Line<'static> {
    let mut spans: Vec<Span> = Vec::new();
    let mut run = String::new();
    let mut run_rgb = [0u8; 3];
    for cell in row {
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
}

fn styled(s: String, [r, g, b]: [u8; 3]) -> Span<'static> {
    Span::styled(s, Style::default().fg(Color::Rgb(r, g, b)))
}

/// Render the world into cells, for a canvas node that shows a view of it.
///
/// The IDE draws one world per game, so every view name resolves to it. A game
/// with several worlds needs the view's own target, which is `game.ron`'s job.
pub struct WorldHost<'a> {
    pub viewport: &'a mut Viewport,
    pub world: &'a scene::Scene,
    pub mode: TextArtMode,
}

impl crate::tui::canvas::CanvasHost for WorldHost<'_> {
    fn view(&mut self, _name: &str, w: u16, h: u16) -> Vec<Vec<textart::TextCell>> {
        self.viewport.world_cells(self.world, self.mode, w, h)
    }

    fn field(&self, path: &str) -> Option<String> {
        let (component, field) = path.split_once('.')?;
        let node = self
            .world
            .nodes
            .iter()
            .find(|n| n.components.contains_key(component))?;
        let value = node.components.get(component)?;
        // The component is opaque here, so read the field out of its RON.
        let text = ron::to_string(value).ok()?;
        let after = text.split(&format!("\"{field}\":")).nth(1)?;
        Some(
            after
                .trim_start()
                .chars()
                .take_while(|c| !matches!(c, ',' | '}' | ')'))
                .collect::<String>()
                .trim()
                .trim_matches('"')
                .to_string(),
        )
    }
}

/// Where the editor looks from.
///
/// This is not scene data: a world declares no camera, and the IDE's viewport is
/// not the game's view. Only the aspect differs between the two render modes.
fn editor_camera(aspect: f32) -> shinra_engine::scene::Camera {
    use shinra_engine::scene::{Camera, Projection};
    Camera {
        eye: glam::Vec3::new(3.0, 3.0, 3.0),
        target: glam::Vec3::ZERO,
        up: glam::Vec3::Y,
        projection: Projection::Perspective {
            fov_y_radians: 60.0_f32.to_radians(),
            aspect,
            znear: 0.01,
            zfar: 100.0,
        },
    }
}
