//! Sprite-sheet rendering: textured quads cut from a grid PNG by UV region
//! (`scene::Sprite { sheet, grid, cell, size }`). Quads face +Z, for
//! side-view / 2D scenes under an orthographic camera.

use std::collections::HashMap;
use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable, Debug)]
pub struct SpriteVertex {
    pub position: [f32; 3],
    pub uv: [f32; 2],
}

impl SpriteVertex {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<SpriteVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}

/// UV rectangle for a sheet cell: returns ((u0, v0), (u1, v1)) with v=0 at
/// the top of the image, matching texture coordinates.
pub fn cell_uv(grid: [u32; 2], cell: [u32; 2]) -> ([f32; 2], [f32; 2]) {
    let (gx, gy) = (grid[0].max(1) as f32, grid[1].max(1) as f32);
    let (cx, cy) = (cell[0] as f32, cell[1] as f32);
    ([cx / gx, cy / gy], [(cx + 1.0) / gx, (cy + 1.0) / gy])
}

pub struct SpriteMesh {
    pub vbuf: wgpu::Buffer,
    pub ibuf: wgpu::Buffer,
    pub index_count: u32,
}

pub struct SpriteTexture {
    pub bind_group: wgpu::BindGroup,
}

/// hecs component: everything needed to draw one sprite instance.
#[derive(Clone)]
pub struct SpriteDraw {
    pub mesh: Arc<SpriteMesh>,
    pub texture: Arc<SpriteTexture>,
}

pub struct SpriteRenderer {
    pub pipeline: wgpu::RenderPipeline,
    texture_bgl: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    /// `None` marks a sheet that failed to load, so we don't retry per frame.
    textures: HashMap<String, Option<Arc<SpriteTexture>>>,
    meshes: HashMap<(u32, u32, u32, u32), Arc<SpriteMesh>>,
}

impl SpriteRenderer {
    pub fn new(
        device: &wgpu::Device,
        camera_bgl: &wgpu::BindGroupLayout,
        object_bgl: &wgpu::BindGroupLayout,
    ) -> Self {
        let texture_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sprite_texture_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sprite_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("sprite.wgsl").into()),
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("sprite_pipeline_layout"),
            bind_group_layouts: &[camera_bgl, object_bgl, &texture_bgl],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sprite_pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[SpriteVertex::desc()],
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
                cull_mode: None, // sprites are visible from both sides
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

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("sprite_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        Self {
            pipeline,
            texture_bgl,
            sampler,
            textures: HashMap::new(),
            meshes: HashMap::new(),
        }
    }

    /// Resolve a scene sprite into a drawable instance. Returns `None` (and
    /// remembers the failure) when the sheet image can't be loaded.
    pub fn instance(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        sprite: &scene_format::Sprite,
    ) -> Option<SpriteDraw> {
        let texture = self.texture(device, queue, &sprite.sheet)?;
        let mesh = self.mesh(device, sprite.grid, sprite.cell);
        Some(SpriteDraw { mesh, texture })
    }

    fn texture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        path: &str,
    ) -> Option<Arc<SpriteTexture>> {
        if let Some(cached) = self.textures.get(path) {
            return cached.clone();
        }
        let loaded = Self::load_texture(device, queue, &self.texture_bgl, &self.sampler, path);
        if loaded.is_none() {
            eprintln!("[sprite] failed to load sheet: {path}");
        }
        self.textures.insert(path.to_string(), loaded.clone());
        loaded
    }

    fn load_texture(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bgl: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        path: &str,
    ) -> Option<Arc<SpriteTexture>> {
        let img = image::open(path).ok()?.to_rgba8();
        let (w, h) = img.dimensions();
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("sprite_sheet"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &img,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * w),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sprite_sheet_bg"),
            layout: bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        });
        Some(Arc::new(SpriteTexture { bind_group }))
    }

    /// Unit quad on the XY plane (centered, facing +Z) with UVs baked for
    /// the given sheet cell. Cached per (grid, cell).
    fn mesh(&mut self, device: &wgpu::Device, grid: [u32; 2], cell: [u32; 2]) -> Arc<SpriteMesh> {
        let key = (grid[0], grid[1], cell[0], cell[1]);
        if let Some(m) = self.meshes.get(&key) {
            return Arc::clone(m);
        }
        let ([u0, v0], [u1, v1]) = cell_uv(grid, cell);
        let vertices = [
            SpriteVertex {
                position: [-0.5, -0.5, 0.0],
                uv: [u0, v1],
            },
            SpriteVertex {
                position: [0.5, -0.5, 0.0],
                uv: [u1, v1],
            },
            SpriteVertex {
                position: [0.5, 0.5, 0.0],
                uv: [u1, v0],
            },
            SpriteVertex {
                position: [-0.5, 0.5, 0.0],
                uv: [u0, v0],
            },
        ];
        let indices: [u32; 6] = [0, 1, 2, 0, 2, 3];
        let vbuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("sprite_vbuf"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let ibuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("sprite_ibuf"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        let mesh = Arc::new(SpriteMesh {
            vbuf,
            ibuf,
            index_count: indices.len() as u32,
        });
        self.meshes.insert(key, Arc::clone(&mesh));
        mesh
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cell_uv_quadrants() {
        // 2x2 grid: top-left cell spans the first UV quadrant.
        assert_eq!(cell_uv([2, 2], [0, 0]), ([0.0, 0.0], [0.5, 0.5]));
        // top-right
        assert_eq!(cell_uv([2, 2], [1, 0]), ([0.5, 0.0], [1.0, 0.5]));
        // bottom-left
        assert_eq!(cell_uv([2, 2], [0, 1]), ([0.0, 0.5], [0.5, 1.0]));
        // bottom-right
        assert_eq!(cell_uv([2, 2], [1, 1]), ([0.5, 0.5], [1.0, 1.0]));
    }

    #[test]
    fn cell_uv_single_cell_is_full_sheet() {
        assert_eq!(cell_uv([1, 1], [0, 0]), ([0.0, 0.0], [1.0, 1.0]));
    }

    #[test]
    fn cell_uv_zero_grid_does_not_divide_by_zero() {
        let ([u0, _], [u1, _]) = cell_uv([0, 0], [0, 0]);
        assert!(u0.is_finite() && u1.is_finite());
    }
}
