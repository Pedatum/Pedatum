//! Meshes, from asset bytes.
//!
//! `asset/*.so` hands over bytes and says nothing about what they mean. The
//! render side decides — here, that a name ending `.obj` is Wavefront OBJ. An
//! asset module can therefore change what a model *is* without the engine
//! learning a new asset type.

use anyhow::{bail, Result};
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
}

pub const VERTEX_ATTRS: [wgpu::VertexAttribute; 3] = wgpu::vertex_attr_array![
    0 => Float32x3,  // position
    1 => Float32x3,  // normal
    2 => Float32x2,  // uv
];

pub fn vertex_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<Vertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &VERTEX_ATTRS,
    }
}

pub struct Mesh {
    pub vertices: wgpu::Buffer,
    pub indices: wgpu::Buffer,
    pub index_count: u32,
}

/// A unit quad on XY, for 2D games and for anything textured.
pub fn quad() -> (Vec<Vertex>, Vec<u32>) {
    let v = vec![
        Vertex { pos: [-0.5, -0.5, 0.0], normal: [0.0, 0.0, 1.0], uv: [0.0, 1.0] },
        Vertex { pos: [0.5, -0.5, 0.0], normal: [0.0, 0.0, 1.0], uv: [1.0, 1.0] },
        Vertex { pos: [0.5, 0.5, 0.0], normal: [0.0, 0.0, 1.0], uv: [1.0, 0.0] },
        Vertex { pos: [-0.5, 0.5, 0.0], normal: [0.0, 0.0, 1.0], uv: [0.0, 0.0] },
    ];
    (v, vec![0, 1, 2, 0, 2, 3])
}

/// Wavefront OBJ: `v`, `vn`, `vt`, and `f` with triangle-fan polygons.
///
/// Faces index the three arrays independently, so a unique `v/vt/vn` triple is
/// one output vertex — which is why this deduplicates rather than expanding.
pub fn parse_obj(bytes: &[u8]) -> Result<(Vec<Vertex>, Vec<u32>)> {
    let text = std::str::from_utf8(bytes)?;
    let mut pos: Vec<[f32; 3]> = Vec::new();
    let mut nrm: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut verts: Vec<Vertex> = Vec::new();
    let mut index: Vec<u32> = Vec::new();
    let mut seen: std::collections::HashMap<(i64, i64, i64), u32> = Default::default();

    let f3 = |it: &mut std::str::SplitWhitespace| -> [f32; 3] {
        let mut v = [0.0; 3];
        for slot in v.iter_mut() {
            *slot = it.next().and_then(|s| s.parse().ok()).unwrap_or(0.0);
        }
        v
    };

    for line in text.lines() {
        let line = line.trim();
        let mut it = line.split_whitespace();
        match it.next() {
            Some("v") => pos.push(f3(&mut it)),
            Some("vn") => nrm.push(f3(&mut it)),
            Some("vt") => {
                let a = it.next().and_then(|s| s.parse().ok()).unwrap_or(0.0);
                let b: f32 = it.next().and_then(|s| s.parse().ok()).unwrap_or(0.0);
                // OBJ's V axis points up; texture space points down.
                uvs.push([a, 1.0 - b]);
            }
            Some("f") => {
                let corners: Vec<&str> = it.collect();
                if corners.len() < 3 {
                    continue;
                }
                let mut resolve = |c: &str| -> Result<u32> {
                    let mut parts = c.split('/');
                    let vi = idx(parts.next(), pos.len())?;
                    let ti = idx(parts.next(), uvs.len()).unwrap_or(-1);
                    let ni = idx(parts.next(), nrm.len()).unwrap_or(-1);
                    let key = (vi, ti, ni);
                    if let Some(&e) = seen.get(&key) {
                        return Ok(e);
                    }
                    let v = Vertex {
                        pos: *pos.get(vi as usize).unwrap_or(&[0.0; 3]),
                        normal: nrm.get(ni.max(0) as usize).copied().unwrap_or([0.0, 1.0, 0.0]),
                        uv: uvs.get(ti.max(0) as usize).copied().unwrap_or([0.0, 0.0]),
                    };
                    verts.push(v);
                    let e = (verts.len() - 1) as u32;
                    seen.insert(key, e);
                    Ok(e)
                };
                let a = resolve(corners[0])?;
                for w in corners[1..].windows(2) {
                    let b = resolve(w[0])?;
                    let c = resolve(w[1])?;
                    index.extend_from_slice(&[a, b, c]);
                }
            }
            _ => {}
        }
    }

    if verts.is_empty() || index.is_empty() {
        bail!("no triangles in this OBJ");
    }
    // A model with no normals still has to shade, so derive them.
    if nrm.is_empty() {
        derive_normals(&mut verts, &index);
    }
    normalize(&mut verts);
    Ok((verts, index))
}

/// Centre the mesh and scale its longest axis to 1.
///
/// Models arrive in whatever units their author used — the Stanford bunny is
/// about 0.15 tall, a teapot about 3. Without this, swapping `asset/bunny.so`
/// for `asset/teapot.so` would change the *size* of the world as well as its
/// contents, and the two modules would not really be interchangeable. Scale
/// belongs to `Transform`, so this is the engine making that true.
fn normalize(verts: &mut [Vertex]) {
    let mut lo = [f32::MAX; 3];
    let mut hi = [f32::MIN; 3];
    for v in verts.iter() {
        for k in 0..3 {
            lo[k] = lo[k].min(v.pos[k]);
            hi[k] = hi[k].max(v.pos[k]);
        }
    }
    let extent = (0..3).fold(0.0f32, |m, k| m.max(hi[k] - lo[k]));
    if !extent.is_finite() || extent <= 1e-9 {
        return;
    }
    let mid = [
        (lo[0] + hi[0]) * 0.5,
        (lo[1] + hi[1]) * 0.5,
        (lo[2] + hi[2]) * 0.5,
    ];
    let k = 1.0 / extent;
    for v in verts.iter_mut() {
        for a in 0..3 {
            v.pos[a] = (v.pos[a] - mid[a]) * k;
        }
    }
}

/// OBJ indices are 1-based and may be negative (relative to the end).
fn idx(s: Option<&str>, len: usize) -> Result<i64> {
    let s = s.unwrap_or("").trim();
    if s.is_empty() {
        bail!("missing index");
    }
    let n: i64 = s.parse()?;
    Ok(if n < 0 { len as i64 + n } else { n - 1 })
}

fn derive_normals(verts: &mut [Vertex], index: &[u32]) {
    for v in verts.iter_mut() {
        v.normal = [0.0; 3];
    }
    for t in index.chunks_exact(3) {
        let (a, b, c) = (t[0] as usize, t[1] as usize, t[2] as usize);
        let p = |i: usize| verts[i].pos;
        let (u, v) = (sub(p(b), p(a)), sub(p(c), p(a)));
        let n = cross(u, v);
        for &i in &[a, b, c] {
            for k in 0..3 {
                verts[i].normal[k] += n[k];
            }
        }
    }
    for v in verts.iter_mut() {
        v.normal = norm(v.normal);
    }
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn norm(a: [f32; 3]) -> [f32; 3] {
    let l = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
    if l > 1e-8 {
        [a[0] / l, a[1] / l, a[2] / l]
    } else {
        [0.0, 1.0, 0.0]
    }
}

impl Mesh {
    pub fn upload(device: &wgpu::Device, name: &str, v: &[Vertex], i: &[u32]) -> Mesh {
        Mesh {
            vertices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("{name}.v")),
                contents: bytemuck::cast_slice(v),
                usage: wgpu::BufferUsages::VERTEX,
            }),
            indices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("{name}.i")),
                contents: bytemuck::cast_slice(i),
                usage: wgpu::BufferUsages::INDEX,
            }),
            index_count: i.len() as u32,
        }
    }

    /// `name` is an asset name; an empty name is the built-in unit quad.
    pub fn load(device: &wgpu::Device, name: &str, bytes: Option<&[u8]>) -> Result<Mesh> {
        if name.is_empty() {
            let (v, i) = quad();
            return Ok(Mesh::upload(device, "quad", &v, &i));
        }
        let Some(bytes) = bytes else {
            bail!("no asset named `{name}` — is the asset module in the bundle?")
        };
        if name.ends_with(".obj") {
            let (v, i) = parse_obj(bytes)?;
            Ok(Mesh::upload(device, name, &v, &i))
        } else {
            bail!("`{name}` is not a mesh this build knows how to read (expected .obj)")
        }
    }
}
