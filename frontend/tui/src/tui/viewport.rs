use image::DynamicImage;
use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;
use shinra_engine::engine::Engine;

const RENDER_WIDTH: u32 = 256;
const RENDER_HEIGHT: u32 = 192;

pub struct Viewport {
    engine: Engine,
    picker: Picker,
    pub image_state: StatefulProtocol,
}

impl Viewport {
    pub fn new() -> Self {
        let engine = Engine::new(RENDER_WIDTH, RENDER_HEIGHT);

        let picker = Picker::from_fontsize((8, 16));
        let blank = DynamicImage::new_rgba8(1, 1);
        let image_state = picker.new_resize_protocol(blank);

        Self {
            engine,
            picker,
            image_state,
        }
    }

    pub fn render_scene(&mut self, scene_data: &scene::Scene) {
        self.engine.load_scene(scene_data);
        self.engine.render_current();
        let img = self.engine.frame_image();
        self.image_state = self
            .picker
            .new_resize_protocol(DynamicImage::ImageRgba8(img));
    }
}
