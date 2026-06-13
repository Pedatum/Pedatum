// Sprite pipeline: textured quads cut from a sprite sheet. Shares the camera
// (group 0) and per-object model (group 1) bind group layouts with the mesh
// pipeline; group 2 adds the sheet texture + sampler.

struct Camera { view_proj: mat4x4<f32>, };
struct Object { model: mat4x4<f32>, };

@group(0) @binding(0) var<uniform> camera: Camera;
@group(1) @binding(0) var<uniform> object: Object;
@group(2) @binding(0) var sheet: texture_2d<f32>;
@group(2) @binding(1) var sheet_sampler: sampler;

struct VsIn  { @location(0) pos: vec3<f32>, @location(1) uv: vec2<f32> };
struct VsOut { @builtin(position) clip: vec4<f32>, @location(0) uv: vec2<f32> };

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    out.clip = camera.view_proj * object.model * vec4<f32>(in.pos, 1.0);
    out.uv = in.uv;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let c = textureSample(sheet, sheet_sampler, in.uv);
    // Cut-out transparency for sheets with an alpha channel; RGB sheets
    // (a = 1 everywhere) draw the full cell.
    if (c.a < 0.5) {
        discard;
    }
    return vec4<f32>(c.rgb, 1.0);
}
