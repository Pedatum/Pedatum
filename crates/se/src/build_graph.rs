//! Describing a render graph.
//!
//! Passes are built into owned `String`s and only converted to ABI structs at
//! the moment they are handed over. That matters because the host deep-copies
//! inside `push`: a shader can therefore be assembled at registration time —
//! `format!` over shared WGSL fragments — instead of having to be one literal.

use se_abi::{Draw, DrawKind, Edge, GraphDesc, GraphSink, Pass, Slice, Str};

fn s(x: &str) -> Str {
    Str { ptr: x.as_ptr(), len: x.len() }
}

#[derive(Default)]
pub struct PassBuild {
    name: String,
    shader: String,
    color: Vec<String>,
    depth: Option<String>,
    reads: Vec<String>,
    uniform_of: Option<String>,
    clear: [f32; 4],
    load: bool,
    instanced: Option<(String, String)>,
}

impl PassBuild {
    /// WGSL for this pass. The engine prepends globals, read bindings, and the
    /// generated `SeVertex` / `SeInstance` / `SeUniform` structs.
    pub fn shader(mut self, src: impl Into<String>) -> Self {
        self.shader = src.into();
        self
    }
    /// Buffers this pass writes.
    pub fn color(mut self, names: &[&str]) -> Self {
        self.color = names.iter().map(|s| s.to_string()).collect();
        self
    }
    pub fn depth(mut self, name: &str) -> Self {
        self.depth = Some(name.to_string());
        self
    }
    /// Sampled buffers read as textures — always the previous frame.
    pub fn reads(mut self, names: &[&str]) -> Self {
        self.reads = names.iter().map(|s| s.to_string()).collect();
        self
    }
    /// A component whose first entity becomes this pass's uniform.
    pub fn uniform_of(mut self, component: &str) -> Self {
        self.uniform_of = Some(component.to_string());
        self
    }
    pub fn clear(mut self, rgba: [f32; 4]) -> Self {
        self.clear = rgba;
        self
    }
    /// Blend over what is already in the target instead of clearing it.
    pub fn load(mut self) -> Self {
        self.load = true;
        self
    }
    /// One instance per entity carrying `component`, drawn with `mesh`.
    /// An empty mesh name is the built-in unit quad.
    pub fn instanced(mut self, component: &str, mesh: &str) -> Self {
        self.instanced = Some((component.to_string(), mesh.to_string()));
        self
    }
}

pub struct GraphBuilder {
    name: String,
    present: String,
    passes: Vec<PassBuild>,
    edges: Vec<(String, String)>,
}

impl GraphBuilder {
    pub fn new(name: &str) -> GraphBuilder {
        GraphBuilder {
            name: name.to_string(),
            present: String::new(),
            passes: Vec::new(),
            edges: Vec::new(),
        }
    }

    /// The buffer whose contents get shown.
    pub fn present(mut self, buffer: &str) -> Self {
        self.present = buffer.to_string();
        self
    }

    pub fn pass(mut self, name: &str, f: impl FnOnce(PassBuild) -> PassBuild) -> Self {
        let p = f(PassBuild { name: name.to_string(), ..Default::default() });
        self.passes.push(p);
        self
    }

    /// Ordering only. Reads never need an edge, because a sampled buffer
    /// resolves to the previous frame and so can never form a cycle.
    pub fn edge(mut self, from: &str, to: &str) -> Self {
        self.edges.push((from.to_string(), to.to_string()));
        self
    }

    /// # Safety
    /// `sink` must be the one the host passed to `se_register_graph`.
    pub unsafe fn finish(&self, sink: &mut GraphSink) {
        // Every borrowed pointer below points into `self` or into these
        // locals, all of which outlive the `push` that copies them.
        let colors: Vec<Vec<Str>> = self
            .passes
            .iter()
            .map(|p| p.color.iter().map(|c| s(c)).collect())
            .collect();
        let reads: Vec<Vec<Str>> = self
            .passes
            .iter()
            .map(|p| p.reads.iter().map(|r| s(r)).collect())
            .collect();

        let passes: Vec<Pass> = self
            .passes
            .iter()
            .enumerate()
            .map(|(i, p)| Pass {
                name: s(&p.name),
                shader: s(&p.shader),
                color: Slice::from_raw(colors[i].as_ptr(), colors[i].len()),
                depth: p.depth.as_deref().map(s).unwrap_or(Str::EMPTY),
                reads: Slice::from_raw(reads[i].as_ptr(), reads[i].len()),
                uniform_of: p.uniform_of.as_deref().map(s).unwrap_or(Str::EMPTY),
                clear: p.clear,
                load: p.load,
                draw: match &p.instanced {
                    None => Draw::FULLSCREEN,
                    Some((c, m)) => Draw {
                        kind: DrawKind::Instanced,
                        instance_of: s(c),
                        mesh: s(m),
                    },
                },
            })
            .collect();

        let edges: Vec<Edge> = self
            .edges
            .iter()
            .map(|(a, b)| Edge { from: s(a), to: s(b) })
            .collect();

        let g = GraphDesc {
            name: s(&self.name),
            passes: Slice::from_raw(passes.as_ptr(), passes.len()),
            edges: Slice::from_raw(edges.as_ptr(), edges.len()),
            present: s(&self.present),
        };
        sink.push(&g);
    }
}
