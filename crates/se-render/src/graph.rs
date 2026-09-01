//! Executing `render/*.so`'s nodes and edges.
//!
//! Pure: the graph reads component columns and asset bytes and writes buffers.
//! It cannot spawn, cannot write data, and knows no time beyond the globals it
//! is handed — not by discipline, but because `se_register_graph` hands over
//! nodes and edges and there is no other call it could make.

use crate::attrs::{self, Instancing, Uniform};
use crate::mesh::{self, Mesh};
use crate::prelude_wgsl::{compose, FS_ENTRY, VS_FULLSCREEN, VS_MAIN};
use crate::targets::Targets;
use anyhow::{anyhow, bail, Context, Result};
use se_host::registry::{AssetDef, GraphDef, PassDef};
use se_host::World;
use std::collections::HashMap;
use wgpu::{BindGroupLayout, Device, Queue, RenderPipeline};

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Globals {
    resolution: [f32; 2],
    time: f32,
    dt: f32,
    frame: f32,
    _pad: [f32; 3],
}

/// What the graph is allowed to look at.
pub struct Scene<'a> {
    pub world: &'a World,
    pub assets: &'a [&'a AssetDef],
}

impl Scene<'_> {
    fn asset(&self, name: &str) -> Option<&[u8]> {
        self.assets
            .iter()
            .find(|a| a.name == name)
            .map(|a| a.bytes.as_slice())
    }
}

struct Instanced {
    component: String,
    mesh: Mesh,
    layout: Instancing,
    buffer: wgpu::Buffer,
    capacity: u64,
}

struct Compiled {
    pipeline: RenderPipeline,
    reads_layout: Option<BindGroupLayout>,
    instanced: Option<Instanced>,
    uniform: Option<(String, Uniform, wgpu::Buffer, wgpu::BindGroup)>,
}

pub struct Graph {
    def: GraphDef,
    order: Vec<usize>,
    compiled: Vec<Compiled>,
    globals_layout: BindGroupLayout,
    uniform_layout: BindGroupLayout,
    globals_buf: wgpu::Buffer,
    globals_bind: wgpu::BindGroup,
    /// Bind groups are positional: a pass with a uniform but no reads still
    /// has a group 1 in its layout, and leaving it unbound is a validation
    /// error rather than a no-op.
    empty_bind: wgpu::BindGroup,
}

/// Kahn's algorithm over the declared edges. Passes nobody ordered keep their
/// declaration order, so a graph with no edges behaves the obvious way.
fn topo(def: &GraphDef) -> Result<Vec<usize>> {
    let index: HashMap<&str, usize> =
        def.passes.iter().enumerate().map(|(i, p)| (p.name.as_str(), i)).collect();
    let n = def.passes.len();
    let mut adj = vec![Vec::new(); n];
    let mut indeg = vec![0usize; n];
    for (from, to) in &def.edges {
        let (&a, &b) = match (index.get(from.as_str()), index.get(to.as_str())) {
            (Some(a), Some(b)) => (a, b),
            _ => bail!("edge `{from}` -> `{to}` names a pass this graph does not declare"),
        };
        adj[a].push(b);
        indeg[b] += 1;
    }
    let mut ready: Vec<usize> = (0..n).filter(|&i| indeg[i] == 0).collect();
    ready.sort_unstable();
    let mut out = Vec::with_capacity(n);
    while !ready.is_empty() {
        let i = ready.remove(0);
        out.push(i);
        for &j in &adj[i] {
            indeg[j] -= 1;
            if indeg[j] == 0 {
                ready.push(j);
                ready.sort_unstable();
            }
        }
    }
    if out.len() != n {
        bail!("render graph `{}` has a cycle in its edges", def.name);
    }
    Ok(out)
}

impl Graph {
    pub fn build(device: &Device, def: &GraphDef, targets: &Targets, scene: &Scene) -> Result<Graph> {
        if def.passes.is_empty() {
            bail!("render graph `{}` declares no passes", def.name);
        }
        targets.get(&def.present)?;
        let order = topo(def)?;

        let globals_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("se.globals"),
            entries: &[uniform_entry(0, wgpu::ShaderStages::VERTEX_FRAGMENT)],
        });
        let uniform_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("se.uniform"),
            entries: &[uniform_entry(0, wgpu::ShaderStages::VERTEX_FRAGMENT)],
        });
        let globals_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("se.globals"),
            size: std::mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let globals_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("se.globals"),
            layout: &globals_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: globals_buf.as_entire_binding() }],
        });

        let empty_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("se.empty"),
            entries: &[],
        });
        let empty_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("se.empty"),
            layout: &empty_layout,
            entries: &[],
        });

        let mut compiled = Vec::with_capacity(def.passes.len());
        for p in &def.passes {
            compiled.push(
                compile(device, p, targets, scene, &globals_layout, &uniform_layout)
                    .with_context(|| format!("pass `{}`", p.name))?,
            );
        }

        Ok(Graph {
            def: def.clone(),
            order,
            compiled,
            globals_layout,
            uniform_layout,
            globals_buf,
            globals_bind,
            empty_bind,
        })
    }

    pub fn def(&self) -> &GraphDef {
        &self.def
    }

    /// Passes in the order they will run — what the IDE lists.
    pub fn order(&self) -> impl Iterator<Item = &PassDef> {
        self.order.iter().map(|&i| &self.def.passes[i])
    }

    pub fn recompile(&mut self, device: &Device, targets: &Targets, scene: &Scene) -> Result<()> {
        self.compiled.clear();
        for p in &self.def.passes {
            self.compiled.push(compile(
                device,
                p,
                targets,
                scene,
                &self.globals_layout,
                &self.uniform_layout,
            )?);
        }
        Ok(())
    }

    pub fn render(
        &mut self,
        device: &Device,
        queue: &Queue,
        targets: &Targets,
        scene: &Scene,
        time: f64,
        dt: f32,
        frame: u64,
        pixel_aspect: f32,
    ) -> Result<()> {
        let (w, h) = targets.screen();
        // Report the apparent size, not the buffer size: a shader asking for
        // aspect wants to know the shape of what the viewer sees.
        let apparent_h = h as f32 * pixel_aspect.max(0.0001);
        queue.write_buffer(
            &self.globals_buf,
            0,
            bytemuck::bytes_of(&Globals {
                resolution: [w as f32, apparent_h],
                time: time as f32,
                dt,
                frame: frame as f32,
                _pad: [0.0; 3],
            }),
        );

        // Instance and uniform data are refreshed before anything is encoded,
        // so a pass never reads a buffer another pass is still filling.
        let mut counts: Vec<u32> = vec![0; self.compiled.len()];
        for (i, c) in self.compiled.iter_mut().enumerate() {
            if let Some(inst) = &mut c.instanced {
                counts[i] = upload_instances(device, queue, inst, scene)?;
            }
            if let Some((name, u, buf, _)) = &c.uniform {
                if let Some(bytes) = scene.world.column(name).and_then(|col| col.row(0)) {
                    queue.write_buffer(buf, 0, &u.pack(bytes));
                }
            }
        }

        let mut enc = device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("se.frame") });

        for &i in &self.order {
            let p = &self.def.passes[i];
            let c = &self.compiled[i];

            let reads_bind = match &c.reads_layout {
                None => None,
                Some(layout) => {
                    let views: Vec<_> = p
                        .reads
                        .iter()
                        .map(|n| targets.get(n).map(|t| t.read_view()))
                        .collect::<Result<_>>()?;
                    let mut entries = vec![wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Sampler(&targets.sampler),
                    }];
                    for (k, v) in views.iter().enumerate() {
                        entries.push(wgpu::BindGroupEntry {
                            binding: k as u32 + 1,
                            resource: wgpu::BindingResource::TextureView(v),
                        });
                    }
                    Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some(&p.name),
                        layout,
                        entries: &entries,
                    }))
                }
            };

            let color_views: Vec<_> = p
                .color
                .iter()
                .map(|n| targets.get(n).map(|t| t.write_view()))
                .collect::<Result<_>>()?;
            let ops = wgpu::Operations {
                load: if p.load {
                    wgpu::LoadOp::Load
                } else {
                    wgpu::LoadOp::Clear(wgpu::Color {
                        r: p.clear[0] as f64,
                        g: p.clear[1] as f64,
                        b: p.clear[2] as f64,
                        a: p.clear[3] as f64,
                    })
                },
                store: wgpu::StoreOp::Store,
            };
            let attachments: Vec<Option<wgpu::RenderPassColorAttachment>> = color_views
                .iter()
                .map(|v| {
                    Some(wgpu::RenderPassColorAttachment {
                        view: v,
                        depth_slice: None,
                        resolve_target: None,
                        ops,
                    })
                })
                .collect();

            let depth = match &p.depth {
                Some(n) => Some(targets.get(n)?),
                None => None,
            };
            let depth_attachment = depth.map(|t| wgpu::RenderPassDepthStencilAttachment {
                view: t.write_view(),
                depth_ops: Some(wgpu::Operations {
                    load: if p.load { wgpu::LoadOp::Load } else { wgpu::LoadOp::Clear(1.0) },
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            });

            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(&p.name),
                color_attachments: &attachments,
                depth_stencil_attachment: depth_attachment,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            rp.set_pipeline(&c.pipeline);
            rp.set_bind_group(0, &self.globals_bind, &[]);
            match &reads_bind {
                Some(b) => rp.set_bind_group(1, b, &[]),
                // Reserved by the layout whenever a uniform follows it.
                None if c.uniform.is_some() => rp.set_bind_group(1, &self.empty_bind, &[]),
                None => {}
            }
            if let Some((_, _, _, bind)) = &c.uniform {
                rp.set_bind_group(2, bind, &[]);
            }
            match &c.instanced {
                None => rp.draw(0..3, 0..1),
                Some(inst) => {
                    let n = counts[i];
                    if n > 0 {
                        rp.set_vertex_buffer(0, inst.mesh.vertices.slice(..));
                        rp.set_vertex_buffer(1, inst.buffer.slice(..));
                        rp.set_index_buffer(
                            inst.mesh.indices.slice(..),
                            wgpu::IndexFormat::Uint32,
                        );
                        rp.draw_indexed(0..inst.mesh.index_count, 0, 0..n);
                    }
                }
            }
        }

        queue.submit([enc.finish()]);
        Ok(())
    }
}

fn uniform_entry(binding: u32, vis: wgpu::ShaderStages) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: vis,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

/// Copy the component column straight into the instance buffer. The column is
/// already dense and already strided the way the GPU wants, so this is a memcpy.
fn upload_instances(
    device: &Device,
    queue: &Queue,
    inst: &mut Instanced,
    scene: &Scene,
) -> Result<u32> {
    let Some(col) = scene.world.column(&inst.component) else { return Ok(0) };
    let n = col.len() as u32;
    if n == 0 {
        return Ok(0);
    }
    let bytes = col.bytes();
    let need = bytes.len() as u64;
    if need > inst.capacity {
        let cap = need.next_power_of_two().max(1024);
        inst.buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("se.instances"),
            size: cap,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        inst.capacity = cap;
    }
    queue.write_buffer(&inst.buffer, 0, bytes);
    Ok(n)
}

fn compile(
    device: &Device,
    p: &PassDef,
    targets: &Targets,
    scene: &Scene,
    globals: &BindGroupLayout,
    uniform_layout: &BindGroupLayout,
) -> Result<Compiled> {
    if p.color.is_empty() {
        bail!("writes no colour buffer");
    }

    // Instancing, if this pass draws entities.
    let instanced = match p.draw.kind {
        se_abi::DrawKind::Fullscreen => None,
        se_abi::DrawKind::Instanced => {
            let component = p
                .draw
                .instance_of
                .clone()
                .ok_or_else(|| anyhow!("an instanced draw must name the component it draws"))?;
            let layout = scene
                .world
                .layout(&component)
                .ok_or_else(|| anyhow!("draws `{component}`, which the bundle does not define"))?;
            let name = p.draw.mesh.clone().unwrap_or_default();
            let m = Mesh::load(device, &name, scene.asset(&name))?;
            Some(Instanced {
                component,
                mesh: m,
                layout: attrs::instancing(layout)?,
                buffer: device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("se.instances"),
                    size: 1024,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }),
                capacity: 1024,
            })
        }
    };

    // A component read as a uniform — a camera, most of the time.
    let uniform = match &p.uniform_of {
        None => None,
        Some(name) => {
            let l = scene
                .world
                .layout(name)
                .ok_or_else(|| anyhow!("reads `{name}` as a uniform, which the bundle does not define"))?;
            let u = attrs::uniform(l)?;
            let buf = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(&format!("{}.uniform", p.name)),
                size: u.size as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(&format!("{}.uniform", p.name)),
                layout: uniform_layout,
                entries: &[wgpu::BindGroupEntry { binding: 0, resource: buf.as_entire_binding() }],
            });
            Some((name.clone(), u, buf, bind))
        }
    };

    let src = compose(
        p,
        instanced.as_ref().map(|i| &i.layout),
        uniform.as_ref().map(|(_, u, _, _)| u),
    );
    // A pass whose shader will not compile shows up as a black frame and a
    // validation error with no source attached, so make the source reachable.
    if let Some(dir) = std::env::var_os("SE_DUMP_WGSL") {
        let dir = std::path::PathBuf::from(dir);
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(dir.join(format!("{}.wgsl", p.name)), &src);
    }
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(&p.name),
        source: wgpu::ShaderSource::Wgsl(src.into()),
    });

    let reads_layout = if p.reads.is_empty() {
        None
    } else {
        let mut entries = vec![wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        }];
        for (i, name) in p.reads.iter().enumerate() {
            let t = targets.get(name)?;
            if !t.def.sampled {
                bail!("reads `{name}`, which buffer.rs did not mark sampled");
            }
            entries.push(wgpu::BindGroupLayoutEntry {
                binding: i as u32 + 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            });
        }
        Some(device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some(&format!("{}.reads", p.name)),
            entries: &entries,
        }))
    };

    // Groups are positional, so an absent group still occupies its slot.
    let empty = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("se.empty"),
        entries: &[],
    });
    let mut layouts: Vec<&BindGroupLayout> = vec![globals];
    if reads_layout.is_some() || uniform.is_some() {
        layouts.push(reads_layout.as_ref().unwrap_or(&empty));
    }
    if uniform.is_some() {
        layouts.push(uniform_layout);
    }
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(&p.name),
        bind_group_layouts: &layouts,
        push_constant_ranges: &[],
    });

    let color_targets: Vec<Option<wgpu::ColorTargetState>> = p
        .color
        .iter()
        .map(|n| {
            targets.get(n).map(|t| {
                Some(wgpu::ColorTargetState {
                    format: t.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })
            })
        })
        .collect::<Result<_>>()?;

    let depth_stencil = match &p.depth {
        Some(n) => Some(wgpu::DepthStencilState {
            format: targets.get(n)?.format,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::LessEqual,
            stencil: Default::default(),
            bias: Default::default(),
        }),
        None => None,
    };

    let instance_layout = instanced.as_ref().map(|i| wgpu::VertexBufferLayout {
        array_stride: i.layout.stride,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &i.layout.attributes,
    });
    let buffers: Vec<wgpu::VertexBufferLayout> = match &instance_layout {
        None => vec![],
        Some(il) => vec![mesh::vertex_layout(), il.clone()],
    };

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(&p.name),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some(if instanced.is_some() { VS_MAIN } else { VS_FULLSCREEN }),
            compilation_options: Default::default(),
            buffers: &buffers,
        },
        primitive: wgpu::PrimitiveState {
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some(FS_ENTRY),
            compilation_options: Default::default(),
            targets: &color_targets,
        }),
        multiview: None,
        cache: None,
    });

    Ok(Compiled { pipeline, reads_layout, instanced, uniform })
}
