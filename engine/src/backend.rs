use std::path::Path;

pub trait EngineBackend {
    fn load_scene(&mut self, scene: &scene_format::Scene);
    fn render(&mut self);
    fn frame_rgba(&mut self) -> Vec<u8>;
    fn frame_image(&mut self) -> image::RgbaImage;
    fn snapshot(&mut self, path: &Path) -> anyhow::Result<()>;
    fn resize(&mut self, width: u32, height: u32);
    fn size(&self) -> (u32, u32);
    fn device(&self) -> &wgpu::Device;
    fn queue(&self) -> &wgpu::Queue;
    fn color_texture(&self) -> &wgpu::Texture;
}

impl EngineBackend for crate::Engine {
    fn load_scene(&mut self, scene: &scene_format::Scene) {
        self.load_scene(scene);
    }

    fn render(&mut self) {
        self.render_current();
    }

    fn frame_rgba(&mut self) -> Vec<u8> {
        self.frame_rgba()
    }

    fn frame_image(&mut self) -> image::RgbaImage {
        self.frame_image()
    }

    fn snapshot(&mut self, path: &Path) -> anyhow::Result<()> {
        self.snapshot(path)
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.resize(width, height);
    }

    fn size(&self) -> (u32, u32) {
        self.size
    }

    fn device(&self) -> &wgpu::Device {
        &self.device
    }

    fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    fn color_texture(&self) -> &wgpu::Texture {
        &self.color
    }
}
