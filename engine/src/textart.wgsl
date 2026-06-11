// Text-art cell pass: collapse the rendered frame into one u32 per character
// cell. Each cell covers 2x4 subpixels (braille layout). A subpixel is "lit"
// when its linear luminance clears the threshold; lit subpixels set their
// braille dot bit and contribute to the cell's average color.
//
// Output packing: bits << 24 | r << 16 | g << 8 | b  (color sRGB-encoded).
// The CPU side (textart::packed_to_cells) only maps ints to glyphs.

struct Params {
    cols: u32,
    rows: u32,
    threshold: f32, // linear-space luminance
    _pad: u32,
};

@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var<storage, read_write> cells: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

fn luminance(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

// Braille dot bit for subpixel (dx, dy): dots 1-3/7 left column, 4-6/8 right.
fn braille_bit(dx: u32, dy: u32) -> u32 {
    if (dy == 3u) {
        return 0x40u << dx;
    }
    return 1u << (dy + dx * 3u);
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= params.cols || gid.y >= params.rows) {
        return;
    }

    var bits: u32 = 0u;
    var sum = vec3<f32>(0.0);
    var lit: u32 = 0u;
    for (var dy = 0u; dy < 4u; dy = dy + 1u) {
        for (var dx = 0u; dx < 2u; dx = dx + 1u) {
            let px = vec2<i32>(i32(gid.x * 2u + dx), i32(gid.y * 4u + dy));
            let c = textureLoad(src, px, 0).rgb; // sRGB texture -> linear values
            if (luminance(c) > params.threshold) {
                bits = bits | braille_bit(dx, dy);
                sum = sum + c;
                lit = lit + 1u;
            }
        }
    }

    var rgb = vec3<u32>(0u, 0u, 0u);
    if (lit > 0u) {
        // Approximate linear -> sRGB so terminal colors match the frame.
        let avg = pow(sum / f32(lit), vec3<f32>(1.0 / 2.2));
        rgb = vec3<u32>(clamp(avg, vec3<f32>(0.0), vec3<f32>(1.0)) * 255.0);
    }
    cells[gid.y * params.cols + gid.x] = (bits << 24u) | (rgb.x << 16u) | (rgb.y << 8u) | rgb.z;
}
