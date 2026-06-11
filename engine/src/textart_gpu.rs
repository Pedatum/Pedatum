//! GPU text-art pass: runs `textart.wgsl` over the engine's offscreen color
//! texture and reads back one packed u32 per character cell. The CPU only
//! turns ints into glyphs (`textart::packed_to_cells`).

use wgpu::util::DeviceExt;

/// Linear-space luminance threshold matching `textart::DEFAULT_THRESHOLD`
/// (which is sRGB-encoded): the clear color sits at ~0.051 linear, the
/// darkest shaded geometry at ~0.117.
pub const DEFAULT_LINEAR_THRESHOLD: f32 = 0.08;

pub struct TextArtGpu {
    pipeline: wgpu::ComputePipeline,
    bgl: wgpu::BindGroupLayout,
    params_buf: wgpu::Buffer,
    /// (storage out, readback) buffers plus the cell grid they were sized for.
    bufs: Option<(wgpu::Buffer, wgpu::Buffer, u32, u32)>,
}

impl TextArtGpu {
    pub fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("textart"),
            source: wgpu::ShaderSource::Wgsl(include_str!("textart.wgsl").into()),
        });

        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("textart_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("textart_layout"),
            bind_group_layouts: &[&bgl],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("textart_pipeline"),
            layout: Some(&layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        let params_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("textart_params"),
            contents: bytemuck::bytes_of(&[0u32; 4]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        Self {
            pipeline,
            bgl,
            params_buf,
            bufs: None,
        }
    }

    fn ensure_buffers(&mut self, device: &wgpu::Device, cols: u32, rows: u32) {
        if matches!(self.bufs, Some((_, _, c, r)) if c == cols && r == rows) {
            return;
        }
        let size = (cols as u64) * (rows as u64) * 4;
        let out = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("textart_out"),
            size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("textart_readback"),
            size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        self.bufs = Some((out, readback, cols, rows));
    }

    /// Run the pass over `texture` (must be at least cols*2 x rows*4 texels)
    /// and return cols*rows packed cells, row-major.
    pub fn cells(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture: &wgpu::Texture,
        cols: u32,
        rows: u32,
        threshold: f32,
    ) -> Vec<u32> {
        if cols == 0 || rows == 0 {
            return Vec::new();
        }
        self.ensure_buffers(device, cols, rows);
        let (out, readback, ..) = self.bufs.as_ref().unwrap();

        let params = [cols, rows, threshold.to_bits(), 0];
        queue.write_buffer(&self.params_buf, 0, bytemuck::bytes_of(&params));

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("textart_bg"),
            layout: &self.bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: out.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.params_buf.as_entire_binding(),
                },
            ],
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("textart_pass"),
        });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("textart"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bg, &[]);
            pass.dispatch_workgroups(cols.div_ceil(8), rows.div_ceil(8), 1);
        }
        encoder.copy_buffer_to_buffer(out, 0, readback, 0, (cols as u64) * (rows as u64) * 4);
        queue.submit([encoder.finish()]);

        let slice = readback.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        let _ = device.poll(wgpu::PollType::wait_indefinitely());
        let data = slice.get_mapped_range();
        let cells: Vec<u32> = bytemuck::cast_slice(&data).to_vec();
        drop(data);
        readback.unmap();
        cells
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Engine;
    use crate::scene::{Camera, Projection, Scene};
    use glam::Vec3;

    fn empty_scene() -> Scene {
        Scene::new(Camera {
            eye: Vec3::new(0.0, 0.0, 3.0),
            target: Vec3::ZERO,
            up: Vec3::Y,
            projection: Projection::Perspective {
                fov_y_radians: 60.0_f32.to_radians(),
                aspect: 1.0,
                znear: 0.1,
                zfar: 100.0,
            },
        })
    }

    #[test]
    fn clear_color_stays_below_default_threshold() {
        let mut engine = Engine::new(4, 8); // 2x2 cells
        engine.render(&empty_scene());
        let mut pass = TextArtGpu::new(&engine.device);
        let cells = pass.cells(
            &engine.device,
            &engine.queue,
            &engine.color,
            2,
            2,
            DEFAULT_LINEAR_THRESHOLD,
        );
        assert_eq!(cells.len(), 4);
        assert!(cells.iter().all(|&c| c == 0), "background must be unlit: {cells:08x?}");
    }

    #[test]
    fn low_threshold_lights_every_dot() {
        let mut engine = Engine::new(4, 8);
        engine.render(&empty_scene());
        let mut pass = TextArtGpu::new(&engine.device);
        let cells = pass.cells(&engine.device, &engine.queue, &engine.color, 2, 2, 0.001);
        assert!(
            cells.iter().all(|&c| c >> 24 == 0xFF),
            "all braille bits must be set: {cells:08x?}"
        );
        // Clear color (0.05, 0.05, 0.07) should sRGB-encode to ~(64, 64, 76).
        let r = (cells[0] >> 16) & 0xFF;
        assert!((50..90).contains(&r), "sRGB-encoded clear red ~64, got {r}");
    }
}
