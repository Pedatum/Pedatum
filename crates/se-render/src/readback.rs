//! Getting the presented buffer back to the CPU.
//!
//! The render side has no window, so "show" means handing pixels to whoever
//! asked — today a terminal. Copying every frame is not free, but at terminal
//! resolutions the frame is a few hundred kilobytes and the graph is the
//! expensive part.

use crate::targets::Target;
use anyhow::{bail, Result};
use wgpu::{Device, Queue};

/// One frame of 8-bit RGBA, tightly packed.
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl Frame {
    #[inline]
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * self.width + x) * 4) as usize;
        [self.rgba[i], self.rgba[i + 1], self.rgba[i + 2], self.rgba[i + 3]]
    }
}

const ALIGN: u32 = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;

pub fn read(device: &Device, queue: &Queue, target: &Target) -> Result<Frame> {
    let bpp = match target.format {
        wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Rgba8UnormSrgb => 4u32,
        other => bail!(
            "the presented buffer is {other:?}; present an Rgba8Unorm buffer, or add a pass that converts to one"
        ),
    };
    let (w, h) = target.size;
    let unpadded = w * bpp;
    let padded = unpadded.div_ceil(ALIGN) * ALIGN;

    let buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("se.readback"),
        size: (padded * h) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut enc =
        device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("se.readback") });
    enc.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: target.write_texture(),
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buf,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded),
                rows_per_image: Some(h),
            },
        },
        wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
    );
    queue.submit([enc.finish()]);

    let slice = buf.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    let _ = device.poll(wgpu::PollType::wait_indefinitely());
    match rx.recv() {
        Ok(Ok(())) => {}
        Ok(Err(e)) => bail!("mapping the readback buffer failed: {e}"),
        Err(_) => bail!("the readback buffer was never mapped"),
    }

    let data = slice.get_mapped_range();
    let mut rgba = Vec::with_capacity((unpadded * h) as usize);
    for row in 0..h {
        let off = (row * padded) as usize;
        rgba.extend_from_slice(&data[off..off + unpadded as usize]);
    }
    drop(data);
    buf.unmap();

    Ok(Frame { width: w, height: h, rgba })
}
