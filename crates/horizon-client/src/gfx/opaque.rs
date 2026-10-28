//! The browser build's last pass: alpha 1 over the whole frame. The frame's alpha only marks
//! what HDR output may lift (0 on the sun, the moon and light-emitting blocks, `hdr.wgsl`),
//! and the browser has no HDR output; but Gecko composites the canvas with that alpha, though
//! wgpu configures it opaque, so those pixels showed the page behind it (black).

const SHADER: &str = "
@vertex fn vs_opaque(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4(p * 2.0 - 1.0, 0.0, 1.0);
}
@fragment fn fs_opaque() -> @location(0) vec4<f32> {
    return vec4(0.0, 0.0, 0.0, 1.0);
}
";

pub struct Opaque {
    pipeline: wgpu::RenderPipeline,
}

impl Opaque {
    /// For views of `format` (the surface's, without its sRGB suffix).
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Opaque frame"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Opaque frame"),
            ..Default::default()
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Opaque frame"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_opaque"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_opaque"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    // The colour stays as drawn.
                    write_mask: wgpu::ColorWrites::ALPHA,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        Self { pipeline }
    }

    pub fn apply(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
        let attachments = [Some(wgpu::RenderPassColorAttachment {
            view,
            resolve_target: None,
            depth_slice: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Store,
            },
        })];
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Opaque frame"),
            color_attachments: &attachments,
            ..Default::default()
        });
        pass.set_pipeline(&self.pipeline);
        pass.draw(0..3, 0..1);
    }
}
