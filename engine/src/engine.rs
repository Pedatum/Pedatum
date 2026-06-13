use std::collections::HashMap;
use std::sync::Arc;
use wgpu::util::DeviceExt;

use glam::{Mat4, Quat, Vec3};

pub struct Engine {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub color: wgpu::Texture,
    pub depth: wgpu::Texture,
    pub size: (u32, u32),
    pipeline: wgpu::RenderPipeline,
    camera_buf: wgpu::Buffer,
    #[allow(dead_code)]
    camera_bgl: wgpu::BindGroupLayout,
    camera_bg: wgpu::BindGroup,
    object_bgl: wgpu::BindGroupLayout,
    object_slots: Vec<(wgpu::Buffer, wgpu::BindGroup)>,
    // Stores Arc<Mesh> alongside buffers so the mesh is kept alive and its
    // pointer is stable (no reuse by a different allocation).
    mesh_cache:
        HashMap<*const crate::mesh::Mesh, (Arc<crate::mesh::Mesh>, wgpu::Buffer, wgpu::Buffer)>,
    pub sprites: crate::sprite::SpriteRenderer,
    readback_buf: Option<wgpu::Buffer>,
    readback_pad_bytes_per_row: u32,
    current_scene: Option<crate::scene::Scene>,
    scene_mesh_cache: HashMap<String, Arc<crate::mesh::Mesh>>,
    quad_mesh: Option<Arc<crate::mesh::Mesh>>,
}

impl Engine {
    /// Build a headless engine (no window/surface) at the given render size.
    pub fn new(width: u32, height: u32) -> Self {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        }))
        .expect("failed to find a suitable wgpu adapter");

        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
                .expect("failed to create wgpu device");

        Self::from_existing(device, queue, width, height)
    }

    /// Build an engine from an already-created device and queue.
    /// Use this when the caller needs to share the device with a surface presenter.
    pub fn from_existing(
        device: wgpu::Device,
        queue: wgpu::Queue,
        width: u32,
        height: u32,
    ) -> Self {
        let color = Self::make_color(&device, width, height);
        let depth = Self::make_depth(&device, width, height);

        let camera_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("camera_bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let object_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("object_bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let identity: [f32; 16] = glam::Mat4::IDENTITY.to_cols_array();
        let camera_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("camera"),
            contents: bytemuck::bytes_of(&identity),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let camera_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("camera_bg"),
            layout: &camera_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buf.as_entire_binding(),
            }],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pipeline_layout"),
            bind_group_layouts: &[&camera_bgl, &object_bgl],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[crate::mesh::Vertex::desc()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8UnormSrgb,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let sprites = crate::sprite::SpriteRenderer::new(&device, &camera_bgl, &object_bgl);

        Self {
            device,
            queue,
            color,
            depth,
            size: (width, height),
            pipeline,
            camera_buf,
            camera_bgl,
            camera_bg,
            object_bgl,
            object_slots: Vec::new(),
            mesh_cache: HashMap::new(),
            sprites,
            readback_buf: None,
            readback_pad_bytes_per_row: 0,
            current_scene: None,
            scene_mesh_cache: HashMap::new(),
            quad_mesh: None,
        }
    }

    pub fn render(&mut self, scene: &crate::scene::Scene) {
        use crate::scene::{MeshHandle, Model};

        let vp: [f32; 16] = scene.camera.view_proj().to_cols_array();
        self.queue
            .write_buffer(&self.camera_buf, 0, bytemuck::bytes_of(&vp));

        // Collect drawables in spawn (query) order — stable within a frame.
        let drawables: Vec<(Arc<crate::mesh::Mesh>, glam::Mat4)> = scene
            .world
            .query::<(&MeshHandle, &Model)>()
            .iter()
            .map(|(_, (mh, m))| (Arc::clone(&mh.0), m.0))
            .collect();

        let sprite_draws: Vec<(crate::sprite::SpriteDraw, glam::Mat4)> = scene
            .world
            .query::<(&crate::sprite::SpriteDraw, &Model)>()
            .iter()
            .map(|(_, (s, m))| (s.clone(), m.0))
            .collect();

        for (mesh, _) in &drawables {
            let mesh_ptr = Arc::as_ptr(mesh);
            if !self.mesh_cache.contains_key(&mesh_ptr) {
                let vbuf = self
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("vbuf"),
                        contents: bytemuck::cast_slice(&mesh.vertices),
                        usage: wgpu::BufferUsages::VERTEX,
                    });
                let ibuf = self
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("ibuf"),
                        contents: bytemuck::cast_slice(&mesh.indices),
                        usage: wgpu::BufferUsages::INDEX,
                    });
                self.mesh_cache
                    .insert(mesh_ptr, (Arc::clone(mesh), vbuf, ibuf));
            }
        }

        // Grow object_slots to cover all drawables, then upload model matrices.
        // Sprites use the slots after the meshes.
        let needed = drawables.len() + sprite_draws.len();
        while self.object_slots.len() < needed {
            let identity: [f32; 16] = glam::Mat4::IDENTITY.to_cols_array();
            let buf = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("object_model"),
                    contents: bytemuck::bytes_of(&identity),
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                });
            let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("object_bg"),
                layout: &self.object_bgl,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buf.as_entire_binding(),
                }],
            });
            self.object_slots.push((buf, bg));
        }
        for (i, model) in drawables
            .iter()
            .map(|(_, m)| m)
            .chain(sprite_draws.iter().map(|(_, m)| m))
            .enumerate()
        {
            let model_arr: [f32; 16] = model.to_cols_array();
            self.queue
                .write_buffer(&self.object_slots[i].0, 0, bytemuck::bytes_of(&model_arr));
        }

        let color_view = self
            .color
            .create_view(&wgpu::TextureViewDescriptor::default());
        let depth_view = self
            .depth
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render"),
            });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &color_view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.05,
                            g: 0.05,
                            b: 0.07,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });

            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.camera_bg, &[]);

            for (i, (mesh, _)) in drawables.iter().enumerate() {
                let mesh_ptr = Arc::as_ptr(mesh);
                let (_, vbuf, ibuf) = self.mesh_cache.get(&mesh_ptr).unwrap();
                pass.set_vertex_buffer(0, vbuf.slice(..));
                pass.set_index_buffer(ibuf.slice(..), wgpu::IndexFormat::Uint32);
                pass.set_bind_group(1, &self.object_slots[i].1, &[]);
                pass.draw_indexed(0..mesh.indices.len() as u32, 0, 0..1);
            }

            if !sprite_draws.is_empty() {
                // Camera bind group (0) carries over — same layout.
                pass.set_pipeline(&self.sprites.pipeline);
                for (i, (sd, _)) in sprite_draws.iter().enumerate() {
                    let slot = drawables.len() + i;
                    pass.set_bind_group(1, &self.object_slots[slot].1, &[]);
                    pass.set_bind_group(2, &sd.texture.bind_group, &[]);
                    pass.set_vertex_buffer(0, sd.mesh.vbuf.slice(..));
                    pass.set_index_buffer(sd.mesh.ibuf.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..sd.mesh.index_count, 0, 0..1);
                }
            }
        }

        self.queue.submit([encoder.finish()]);
    }

    pub fn ensure_readback_buffer(&mut self) {
        if self.readback_buf.is_some() {
            return;
        }
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let unpadded = self.size.0 * 4;
        let padded = unpadded.div_ceil(align) * align;
        self.readback_pad_bytes_per_row = padded;
        self.readback_buf = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: (padded as u64) * (self.size.1 as u64),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        }));
    }

    pub fn frame_rgba(&mut self) -> Vec<u8> {
        self.ensure_readback_buffer();
        let buf = self.readback_buf.as_ref().unwrap();
        let padded = self.readback_pad_bytes_per_row;
        let (w, h) = self.size;

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("readback"),
            });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.color,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: buf,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: None,
                },
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([encoder.finish()]);

        let slice = buf.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());

        let data = slice.get_mapped_range();
        let unpadded_bytes_per_row = w as usize * 4;
        let mut pixels = Vec::with_capacity(unpadded_bytes_per_row * h as usize);
        for row in 0..h as usize {
            let start = row * padded as usize;
            pixels.extend_from_slice(&data[start..start + unpadded_bytes_per_row]);
        }
        drop(data);
        buf.unmap();
        pixels
    }

    pub fn frame_image(&mut self) -> image::RgbaImage {
        let (w, h) = self.size;
        let pixels = self.frame_rgba();
        image::RgbaImage::from_raw(w, h, pixels).expect("pixel buffer size mismatch")
    }

    pub fn snapshot(&mut self, path: &std::path::Path) -> anyhow::Result<()> {
        let img = self.frame_image();
        img.save(path)?;
        Ok(())
    }

    /// Reallocate color + depth textures at a new size.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.color = Self::make_color(&self.device, width, height);
        self.depth = Self::make_depth(&self.device, width, height);
        self.size = (width, height);
        self.readback_buf = None;
    }

    pub fn load_scene(&mut self, scene_data: &scene_format::Scene) {
        use crate::scene::{
            Camera as EngineCamera, Projection as EngineProjection, Scene as EngineScene,
        };

        let cam = scene_data
            .camera
            .as_ref()
            .map(|c| {
                let projection = match &c.projection {
                    scene_format::Projection::Perspective {
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
                    scene_format::Projection::Orthographic {
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
                    eye: Vec3::from(c.eye),
                    target: Vec3::from(c.target),
                    up: Vec3::from(c.up),
                    projection,
                }
            })
            .unwrap_or_else(|| {
                let (w, h) = self.size;
                EngineCamera {
                    eye: Vec3::new(3.0, 3.0, 3.0),
                    target: Vec3::ZERO,
                    up: Vec3::Y,
                    projection: EngineProjection::Perspective {
                        fov_y_radians: 60.0_f32.to_radians(),
                        aspect: w as f32 / h as f32,
                        znear: 0.1,
                        zfar: 100.0,
                    },
                }
            });

        let mut sc = EngineScene::new(cam);

        for node in &scene_data.nodes {
            self.spawn_node(&mut sc, node, Mat4::IDENTITY);
        }

        self.current_scene = Some(sc);
    }

    fn spawn_node(
        &mut self,
        scene: &mut crate::scene::Scene,
        node: &scene_format::Node,
        parent_transform: Mat4,
    ) {
        use crate::mesh::Mesh;
        let t = &node.transform;
        let local = Mat4::from_scale_rotation_translation(
            Vec3::from(t.scale),
            Quat::from_array(t.rotation),
            Vec3::from(t.translation),
        );
        let world = parent_transform * local;

        if let Some(sprite) = &node.sprite {
            if let Some(draw) = self.sprites.instance(&self.device, &self.queue, sprite) {
                let model =
                    world * Mat4::from_scale(Vec3::new(sprite.size[0], sprite.size[1], 1.0));
                scene.world.spawn((draw, crate::scene::Model(model)));
            }
        }

        if let Some(tilemap) = &node.tilemap {
            if self.quad_mesh.is_none() {
                if let Ok(m) = Mesh::from_obj_file("assets/obj/quad.obj") {
                    self.quad_mesh = Some(Arc::new(m));
                }
            }
            if let Some(quad) = self.quad_mesh.as_ref() {
                for cell in &tilemap.cells {
                    let model = world
                        * Mat4::from_translation(Vec3::new(
                            cell.x as f32 * tilemap.tile_size[0],
                            0.0,
                            cell.y as f32 * tilemap.tile_size[1],
                        ));
                    scene.spawn_mesh(Arc::clone(quad), model);
                }
            }
        }

        if let Some(mesh_ref) = &node.mesh {
            if !self.scene_mesh_cache.contains_key(&mesh_ref.path) {
                if let Ok(m) = Mesh::from_obj_file(&mesh_ref.path) {
                    self.scene_mesh_cache
                        .insert(mesh_ref.path.clone(), Arc::new(m));
                }
            }
            if let Some(mesh) = self.scene_mesh_cache.get(&mesh_ref.path) {
                scene.spawn_mesh(Arc::clone(mesh), world);
            }
        }

        for child in &node.children {
            self.spawn_node(scene, child, world);
        }
    }

    pub fn render_current(&mut self) {
        if let Some(scene) = self.current_scene.take() {
            self.render(&scene);
            self.current_scene = Some(scene);
        }
    }

    fn make_color(device: &wgpu::Device, width: u32, height: u32) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some("color"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        })
    }

    fn make_depth(device: &wgpu::Device, width: u32, height: u32) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some("depth"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_init() {
        let engine = Engine::new(64, 64);
        assert_eq!(engine.size, (64, 64));

        let color_size = engine.color.size();
        assert_eq!(color_size.width, 64);
        assert_eq!(color_size.height, 64);
        assert_eq!(engine.color.format(), wgpu::TextureFormat::Rgba8UnormSrgb);

        let depth_size = engine.depth.size();
        assert_eq!(depth_size.width, 64);
        assert_eq!(depth_size.height, 64);
        assert_eq!(engine.depth.format(), wgpu::TextureFormat::Depth32Float);
    }

    #[test]
    fn engine_resize() {
        let mut engine = Engine::new(64, 64);
        engine.resize(128, 96);
        assert_eq!(engine.size, (128, 96));

        let color_size = engine.color.size();
        assert_eq!(color_size.width, 128);
        assert_eq!(color_size.height, 96);

        let depth_size = engine.depth.size();
        assert_eq!(depth_size.width, 128);
        assert_eq!(depth_size.height, 96);
    }

    #[test]
    fn engine_pipeline() {
        let engine = Engine::new(64, 64);
        assert_eq!(engine.camera_buf.size(), 64);
    }

    #[test]
    fn engine_snapshot() {
        let mut engine = Engine::new(64, 64);
        let scene = crate::scene::Scene::new(crate::scene::Camera {
            eye: glam::Vec3::new(0.0, 0.0, 3.0),
            target: glam::Vec3::ZERO,
            up: glam::Vec3::Y,
            projection: crate::scene::Projection::Perspective {
                fov_y_radians: 60.0_f32.to_radians(),
                aspect: 1.0,
                znear: 0.1,
                zfar: 100.0,
            },
        });
        engine.render(&scene);
        let path = std::env::temp_dir().join("shinra_snapshot_test.png");
        engine.snapshot(&path).expect("snapshot should succeed");
        assert!(path.exists(), "PNG file should be created");
        let img = image::open(&path).expect("should open as valid image");
        assert_eq!(img.width(), 64);
        assert_eq!(img.height(), 64);
        std::fs::remove_file(&path).ok();
    }
}
