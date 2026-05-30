use image::{DynamicImage, RgbaImage};
use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;
use shinra_engine::engine::Engine;

const RENDER_WIDTH: u32 = 256;
const RENDER_HEIGHT: u32 = 192;

pub struct Viewport {
    engine: Engine,
    width: u32,
    height: u32,
    picker: Picker,
    pub image_state: StatefulProtocol,
}

impl Viewport {
    pub fn new() -> Self {
        let width = RENDER_WIDTH;
        let height = RENDER_HEIGHT;
        let engine = Engine::new(width, height);

        let picker = Picker::from_fontsize((8, 16));
        let blank = DynamicImage::new_rgba8(1, 1);
        let image_state = picker.new_resize_protocol(blank);

        Self {
            engine,
            width,
            height,
            picker,
            image_state,
        }
    }

    pub fn render_scene(&mut self, scene_data: &scene::Scene) {
        self.engine.load_scene(scene_data);
        self.engine.render_current();
        let rgba = self.engine.frame_rgba();
        let img = RgbaImage::from_raw(self.width, self.height, rgba).unwrap();
        self.image_state = self
            .picker
            .new_resize_protocol(DynamicImage::ImageRgba8(img));
    }
}
