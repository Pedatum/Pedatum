//! The viewport panel: a view's world, drawn into the terminal.
//!
//! A view declares which projection it looks through, and that choice has to
//! survive all the way to `shader.wgsl`. It does so as one matrix:
//!
//! ```text
//! ViewDef.camera ──> view::runtime_camera ──> Camera::view_proj ──> P * V * M
//! ```

use image::DynamicImage;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span, Text};
use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;
use shinra_engine::engine::Engine;
use shinra_engine::textart::{self, TextArtMode};
use shinra_engine::textart_gpu::{TextArtGpu, DEFAULT_LINEAR_THRESHOLD};
use shinra_engine::view::{runtime_camera, Surface};

const RENDER_WIDTH: u32 = 256;
const RENDER_HEIGHT: u32 = 192;

/// Terminal cells are roughly twice as tall as wide; used to correct the
/// camera aspect so geometry keeps its proportions in cell space.
const CELL_ASPECT: f32 = 2.0;

/// What the engine currently holds. Loading a world spawns an entity per mesh
/// and per sprite, so it happens when something actually changed, not per frame.
#[derive(Clone, Copy, PartialEq)]
struct Loaded {
    /// The scene's revision, bumped by whoever mutates it.
    rev: u64,
    camera: shinra_engine::scene::Camera,
    size: (u32, u32),
}

pub struct Viewport {
    engine: Engine,
    textart_pass: TextArtGpu,
    picker: Picker,
    loaded: Option<Loaded>,
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
            loaded: None,
            image_state,
        }
    }

    /// Resize if needed, then load the world only if this frame differs from
    /// the last one that was loaded.
    fn prepare(
        &mut self,
        world: &scene::Scene,
        camera: &scene::Camera,
        rev: u64,
        size: (u32, u32),
        surface: Surface,
    ) {
        if self.engine.size != size {
            self.engine.resize(size.0, size.1);
        }
        let want = Loaded {
            rev,
            camera: runtime_camera(camera, world, surface),
            size,
        };
        if self.loaded == Some(want) {
            return;
        }
        self.engine.load_world(world, want.camera);
        self.loaded = Some(want);
    }

    /// Image-protocol path (kitty/sixel/halfblocks via ratatui-image).
    pub fn render_scene(&mut self, world: &scene::Scene, camera: &scene::Camera, rev: u64) {
        let size = (RENDER_WIDTH, RENDER_HEIGHT);
        self.prepare(
            world,
            camera,
            rev,
            size,
            Surface::pixels(RENDER_WIDTH, RENDER_HEIGHT),
        );
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
        world: &scene::Scene,
        camera: &scene::Camera,
        rev: u64,
        mode: TextArtMode,
        cols: u16,
        rows: u16,
    ) -> Text<'static> {
        let rows_of_cells = self.world_cells(world, camera, rev, mode, cols, rows);
        let lines: Vec<Line> = rows_of_cells.into_iter().map(cells_to_line).collect();
        Text::from(lines)
    }

    /// The world as cells, for either the viewport or a canvas node.
    pub fn world_cells(
        &mut self,
        world: &scene::Scene,
        camera: &scene::Camera,
        rev: u64,
        mode: TextArtMode,
        cols: u16,
        rows: u16,
    ) -> Vec<Vec<textart::TextCell>> {
        let size = (cols as u32 * 2, rows as u32 * 4);
        if size.0 == 0 || size.1 == 0 {
            return Vec::new();
        }
        // The panel's on-screen aspect, corrected for cells being about twice
        // as tall as wide, so the world is not stretched by the cell grid.
        let surface = Surface::cells(cols as u32, rows as u32, CELL_ASPECT);
        self.prepare(world, camera, rev, size, surface);
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

/// Draws the views a canvas samples. `Sprite(source: View("game"))` resolves
/// here, through the camera that view declared.
///
/// The IDE draws one world per game, so every view name resolves to it. A game
/// with several worlds needs the view's own target, which is `game.ron`'s job.
pub struct WorldHost<'a> {
    pub viewport: &'a mut Viewport,
    pub game: &'a scene::Game,
    pub world: &'a scene::Scene,
    pub rev: u64,
    pub mode: TextArtMode,
}

impl crate::tui::canvas::CanvasHost for WorldHost<'_> {
    fn view(&mut self, name: &str, w: u16, h: u16) -> Vec<Vec<textart::TextCell>> {
        // A canvas naming a view that does not exist would otherwise draw the
        // world through whatever camera happened to be loaded last.
        let Some(view) = self.game.views.get(name) else {
            eprintln!("[ide] canvas names view `{name}`, which game.ron does not declare");
            return Vec::new();
        };
        self.viewport
            .world_cells(self.world, &view.camera, self.rev, self.mode, w, h)
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

/// Where the editor looks from when there is no game to ask — an ad-hoc
/// `world.ron` with no `game.ron` beside it.
pub fn editor_camera() -> scene::Camera {
    scene::Camera {
        projection: scene::Projection::Perspective {
            fov_y_degrees: 60.0,
            znear: 0.01,
            zfar: 100.0,
            eye: [3.0, 3.0, 3.0],
            target: [0.0, 0.0, 0.0],
            up: [0.0, 1.0, 0.0],
        },
        anchor: None,
    }
}
