//! Buffers, as `bundle/buffer.rs` declared them.
//!
//! A `sampled` buffer gets two textures, not one. A pass writes the current
//! and reads the previous, which is what makes "a target you also read" legal
//! and keeps the graph acyclic. Recursion through a mirror then costs one
//! frame per bounce — deep reflections lag, like slow light.

use anyhow::{anyhow, Result};
use se_host::registry::BufferDef;
use std::collections::HashMap;
use wgpu::{Device, Texture, TextureFormat, TextureUsages, TextureView};

pub fn wgpu_format(f: se_abi::Format) -> TextureFormat {
    match f {
        se_abi::Format::Rgba8Unorm => TextureFormat::Rgba8Unorm,
        se_abi::Format::Rgba8UnormSrgb => TextureFormat::Rgba8UnormSrgb,
        se_abi::Format::Rgba16Float => TextureFormat::Rgba16Float,
        se_abi::Format::R32Float => TextureFormat::R32Float,
        se_abi::Format::Depth32Float => TextureFormat::Depth32Float,
    }
}

pub struct Target {
    pub def: BufferDef,
    pub size: (u32, u32),
    pub format: TextureFormat,
    /// One texture, or two when `sampled` — write `[cur]`, read `[1 - cur]`.
    textures: Vec<Texture>,
    views: Vec<TextureView>,
    cur: usize,
}

impl Target {
    fn create(device: &Device, def: &BufferDef, size: (u32, u32)) -> Target {
        let format = wgpu_format(def.format);
        let mut usage = TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC;
        if def.sampled || !format.is_depth_stencil_format() {
            usage |= TextureUsages::TEXTURE_BINDING;
        }
        let n = if def.sampled { 2 } else { 1 };
        let mut textures = Vec::with_capacity(n);
        let mut views = Vec::with_capacity(n);
        for i in 0..n {
            let t = device.create_texture(&wgpu::TextureDescriptor {
                label: Some(&format!("{}#{i}", def.name)),
                size: wgpu::Extent3d {
                    width: size.0,
                    height: size.1,
                    depth_or_array_layers: def.count.max(1),
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            });
            views.push(t.create_view(&wgpu::TextureViewDescriptor::default()));
            textures.push(t);
        }
        Target { def: def.clone(), size, format, textures, views, cur: 0 }
    }

    /// Where this frame writes.
    pub fn write_view(&self) -> &TextureView {
        &self.views[self.cur]
    }
    pub fn write_texture(&self) -> &Texture {
        &self.textures[self.cur]
    }
    /// What this frame reads: last frame's contents.
    pub fn read_view(&self) -> &TextureView {
        &self.views[(self.cur + 1) % self.views.len()]
    }
    fn flip(&mut self) {
        if self.textures.len() > 1 {
            self.cur = 1 - self.cur;
        }
    }
}

/// Every buffer the bundle declared, sized against the current screen.
pub struct Targets {
    map: HashMap<String, Target>,
    screen: (u32, u32),
    pub sampler: wgpu::Sampler,
}

impl Targets {
    pub fn new(device: &Device, defs: &[BufferDef], screen: (u32, u32)) -> Targets {
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("shinra.linear"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let mut map = HashMap::new();
        for d in defs {
            map.insert(d.name.clone(), Target::create(device, d, d.resolve(screen)));
        }
        Targets { map, screen, sampler }
    }

    pub fn screen(&self) -> (u32, u32) {
        self.screen
    }

    pub fn get(&self, name: &str) -> Result<&Target> {
        self.map
            .get(name)
            .ok_or_else(|| anyhow!("no buffer named `{name}` in this bundle"))
    }

    /// Reallocate everything whose size depends on the screen.
    pub fn resize(&mut self, device: &Device, screen: (u32, u32)) {
        if screen == self.screen || screen.0 == 0 || screen.1 == 0 {
            return;
        }
        self.screen = screen;
        for t in self.map.values_mut() {
            let want = t.def.resolve(screen);
            if want != t.size {
                *t = Target::create(device, &t.def, want);
            }
        }
    }

    /// End of frame: what was written becomes what can be read.
    pub fn flip(&mut self) {
        for t in self.map.values_mut() {
            t.flip();
        }
    }
}
