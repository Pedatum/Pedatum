use std::collections::HashMap;
use std::sync::Arc;

use glam::{Mat4, Quat, Vec3};
use image::{DynamicImage, RgbaImage};
use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;
use shinra_engine::engine::Engine;
use shinra_engine::mesh::Mesh;
use shinra_engine::scene::{
    Camera as EngineCamera, Projection as EngineProjection, Scene as EngineScene,
};

const RENDER_WIDTH: u32 = 256;
const RENDER_HEIGHT: u32 = 192;

pub struct Viewport {
    engine: Engine,
    readback: wgpu::Buffer,
    pad_bytes_per_row: u32,
    width: u32,
    height: u32,
    picker: Picker,
    pub image_state: StatefulProtocol,
    mesh_cache: HashMap<String, Arc<Mesh>>,
    quad_mesh: Option<Arc<Mesh>>,
}

impl Viewport {
    pub fn new() -> Self {
        let width = RENDER_WIDTH;
        let height = RENDER_HEIGHT;
        let engine = Engine::new(width, height);

        let unpadded = width * 4;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let pad_bytes_per_row = unpadded.div_ceil(align) * align;
        let buffer_size = (pad_bytes_per_row * height) as u64;

        let readback = engine.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("viewport_readback"),
            size: buffer_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let picker = Picker::from_fontsize((8, 16));
        let blank = DynamicImage::new_rgba8(1, 1);
        let image_state = picker.new_resize_protocol(blank);

        Self {
            engine,
            readback,
            pad_bytes_per_row,
            width,
            height,
            picker,
            image_state,
            mesh_cache: HashMap::new(),
            quad_mesh: None,
        }
    }

    pub fn render_scene(&mut self, scene_data: &scene::Scene) {
        let cam = scene_data
            .camera
            .as_ref()
            .map(to_engine_camera)
            .unwrap_or_else(|| fallback_camera(self.width, self.height));

        let engine_scene =
            build_engine_scene(scene_data, cam, &mut self.mesh_cache, &mut self.quad_mesh);

        self.engine.render(&engine_scene);

        let mut encoder = self
            .engine
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("viewport_readback"),
            });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.engine.color,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &self.readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(self.pad_bytes_per_row),
                    rows_per_image: None,
                },
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );
        self.engine.queue.submit([encoder.finish()]);

        let slice = self.readback.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        let _ = self.engine.device.poll(wgpu::PollType::wait_indefinitely());

        let pixels = {
            let data = slice.get_mapped_range();
            let w = self.width as usize;
            let h = self.height as usize;
            let pad_bpr = self.pad_bytes_per_row as usize;
            let real_bpr = w * 4;
            let mut buf = Vec::with_capacity(w * h * 4);
            for row in 0..h {
                let start = row * pad_bpr;
                buf.extend_from_slice(&data[start..start + real_bpr]);
            }
            buf
        };
        self.readback.unmap();

        let img = RgbaImage::from_raw(self.width, self.height, pixels).unwrap();
        self.image_state = self
            .picker
            .new_resize_protocol(DynamicImage::ImageRgba8(img));
    }
}

fn fallback_camera(width: u32, height: u32) -> EngineCamera {
    EngineCamera {
        eye: Vec3::new(3.0, 3.0, 3.0),
        target: Vec3::ZERO,
        up: Vec3::Y,
        projection: EngineProjection::Perspective {
            fov_y_radians: 60.0_f32.to_radians(),
            aspect: width as f32 / height as f32,
            znear: 0.1,
            zfar: 100.0,
        },
    }
}

fn to_engine_camera(cam: &scene::Camera) -> EngineCamera {
    let projection = match &cam.projection {
        scene::Projection::Perspective {
            fov_y_degrees,
            aspect,
            znear,
            zfar,
        } => EngineProjection::Perspective {
            fov_y_radians: fov_y_degrees.to_radians(),
            aspect: *aspect,
            znear: *znear,
            zfar: *zfar,
        },
        scene::Projection::Orthographic {
            half_height,
            aspect,
            znear,
            zfar,
        } => EngineProjection::Orthographic {
            half_height: *half_height,
            aspect: *aspect,
            znear: *znear,
            zfar: *zfar,
        },
    };
    EngineCamera {
        eye: Vec3::from(cam.eye),
        target: Vec3::from(cam.target),
        up: Vec3::from(cam.up),
        projection,
    }
}

fn build_engine_scene(
    doc: &scene::Scene,
    camera: EngineCamera,
    mesh_cache: &mut HashMap<String, Arc<Mesh>>,
    quad_mesh: &mut Option<Arc<Mesh>>,
) -> EngineScene {
    let mut sc = EngineScene::new(camera);

    for node in &doc.nodes {
        if let Some(tilemap) = &node.tilemap {
            if quad_mesh.is_none() {
                if let Ok(m) = Mesh::from_obj_file("assets/quad.obj") {
                    *quad_mesh = Some(Arc::new(m));
                }
            }
            if let Some(quad) = quad_mesh.as_ref() {
                for cell in &tilemap.cells {
                    let model = Mat4::from_translation(Vec3::new(
                        cell.x as f32 * tilemap.tile_size[0],
                        0.0,
                        cell.y as f32 * tilemap.tile_size[1],
                    ));
                    sc.spawn_mesh(Arc::clone(quad), model);
                }
            }
        }

        if let Some(mesh_ref) = &node.mesh {
            if !mesh_cache.contains_key(&mesh_ref.path) {
                if let Ok(m) = Mesh::from_obj_file(&mesh_ref.path) {
                    mesh_cache.insert(mesh_ref.path.clone(), Arc::new(m));
                }
            }
            if let Some(mesh) = mesh_cache.get(&mesh_ref.path) {
                let t = &node.transform;
                let model = Mat4::from_scale_rotation_translation(
                    Vec3::from(t.scale),
                    Quat::from_array(t.rotation),
                    Vec3::from(t.translation),
                );
                sc.spawn_mesh(Arc::clone(mesh), model);
            }
        }
    }

    sc
}
