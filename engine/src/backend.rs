use std::path::Path;

pub trait EngineBackend {
    fn load_scene(&mut self, scene: &scene_format::Scene);
    fn render(&mut self);
    fn frame_rgba(&self) -> Vec<u8>;
    fn frame_image(&self) -> image::RgbaImage;
    fn snapshot(&self, path: &Path) -> anyhow::Result<()>;
    fn resize(&mut self, width: u32, height: u32);
    fn size(&self) -> (u32, u32);
    fn device(&self) -> &wgpu::Device;
    fn queue(&self) -> &wgpu::Queue;
    fn color_texture(&self) -> &wgpu::Texture;
}
