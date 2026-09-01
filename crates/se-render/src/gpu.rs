//! Device acquisition.
//!
//! Headless by design. The render side never owns a window — it fills buffers,
//! and something else decides what "show" means. That is what lets the same
//! graph present to a terminal, and it is why a software adapter is a
//! first-class outcome here rather than a fallback to apologise for.

use anyhow::{Context, Result};
use wgpu::{Adapter, Device, DeviceDescriptor, Instance, InstanceDescriptor, Queue};

pub struct Gpu {
    pub device: Device,
    pub queue: Queue,
    pub adapter_name: String,
    pub is_software: bool,
}

impl Gpu {
    pub fn new() -> Result<Gpu> {
        pollster::block_on(Self::new_async())
    }

    pub async fn new_async() -> Result<Gpu> {
        let instance = Instance::new(&InstanceDescriptor::default());
        let adapter: Adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: None,
            })
            .await
            .context("no wgpu adapter — install a Vulkan driver, or mesa-vulkan-drivers for a software one")?;

        let info = adapter.get_info();
        let is_software = info.device_type == wgpu::DeviceType::Cpu;

        let (device, queue) = adapter
            .request_device(&DeviceDescriptor {
                label: Some("shinra"),
                required_features: wgpu::Features::empty(),
                // Whatever the adapter has. A software rasteriser reports
                // modest limits and the graph is expected to fit them.
                required_limits: adapter.limits(),
                ..Default::default()
            })
            .await
            .context("request_device")?;

        // Validation errors would otherwise surface as a silent black frame,
        // which is the worst possible failure mode for a hot-swapped shader.
        device.on_uncaptured_error(std::sync::Arc::new(|e| {
            eprintln!("[shinra:gpu] {e}");
        }));

        Ok(Gpu {
            device,
            queue,
            adapter_name: info.name,
            is_software,
        })
    }

    pub fn wait(&self) {
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
    }
}
