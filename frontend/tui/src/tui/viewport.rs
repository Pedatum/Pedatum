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
        self.engine.load_scene(scene_data);
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
        let (w, h) = (cols as u32 * 2, rows as u32 * 4);
        if w == 0 || h == 0 {
            return Text::default();
        }
        if self.engine.size != (w, h) {
            self.engine.resize(w, h);
        }

        // Override the camera aspect with the panel's on-screen aspect so the
        // scene isn't stretched by the cell grid.
        let visual_aspect = cols as f32 / (rows as f32 * CELL_ASPECT);
        let mut sd = scene_data.clone();
        let cam = sd.camera.get_or_insert_with(default_camera);
        match &mut cam.projection {
            scene::Projection::Perspective { aspect, .. } => *aspect = visual_aspect,
            scene::Projection::Orthographic { aspect, .. } => *aspect = visual_aspect,
        }

        self.engine.load_scene(&sd);
        self.engine.render_current();
        let packed = self.textart_pass.cells(
            &self.engine.device,
            &self.engine.queue,
            &self.engine.color,
            cols as u32,
            rows as u32,
            DEFAULT_LINEAR_THRESHOLD,
        );
        let cells = textart::packed_to_cells(&packed, cols as usize, mode);

        let lines: Vec<Line> = cells
            .into_iter()
            .map(|row| {
                // Merge runs of same-colored cells into single spans.
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
            })
            .collect();
        Text::from(lines)
    }
}

fn styled(s: String, [r, g, b]: [u8; 3]) -> Span<'static> {
    Span::styled(s, Style::default().fg(Color::Rgb(r, g, b)))
}

/// Mirrors the engine's fallback camera (`Engine::load_scene`) for scenes
/// without an embedded camera, so only the aspect differs between modes.
fn default_camera() -> scene::Camera {
    scene::Camera {
        eye: [3.0, 3.0, 3.0],
        target: [0.0, 0.0, 0.0],
        up: [0.0, 1.0, 0.0],
        projection: scene::Projection::Perspective {
            fov_y_degrees: 60.0,
            aspect: 1.0,
            znear: 0.1,
            zfar: 100.0,
        },
    }
}
