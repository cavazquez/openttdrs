//! Captured binary-alpha sampling against pristine native Draw at six zooms.
//! This probe isolates aligned geometry/nearest sampling; it is not scene parity.
#[allow(dead_code)]
#[path = "probe_projected_mask_depth.rs"]
mod depth_probe;

use bevy::tasks::block_on;
use std::path::Path;
use wgpu::util::DeviceExt;

const WIDTH: u32 = 288;
const HEIGHT: u32 = 232;
const SOURCE_WIDTH: u32 = 66;
const SOURCE_HEIGHT: u32 = 52;
const SHADER: &str = r"
struct Params { quad: vec4<f32>, sample: vec4<f32> }
@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var source: texture_2d<f32>;
@group(0) @binding(2) var nearest: sampler;
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}
@vertex fn vertex(@builtin(vertex_index) index: u32) -> VertexOutput {
    let corners = array<vec2<f32>, 6>(vec2(0.0,0.0),vec2(1.0,0.0),vec2(1.0,1.0),vec2(0.0,0.0),vec2(1.0,1.0),vec2(0.0,1.0));
    let screen = params.quad.xy + corners[index] * params.quad.zw;
    var output: VertexOutput;
    output.position = vec4(screen.x / 144.0 - 1.0, 1.0 - screen.y / 116.0, 0.5, 1.0);
    output.uv = corners[index] * params.quad.zw * params.sample.x / vec2(66.0,52.0);
    return output;
}
@fragment fn fragment(input: VertexOutput) -> @location(0) vec4<f32> {
    let uv = input.uv + vec2(params.sample.y) / vec2(66.0,52.0);
    if textureSample(source, nearest, uv).a < 0.5 { discard; }
    return vec4(1.0);
}
";

fn render(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    pipeline: &wgpu::RenderPipeline,
    source_view: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
    scale: f32,
    variant: u32,
) -> Vec<u8> {
    let mut dimensions = [SOURCE_WIDTH as f32 / scale, SOURCE_HEIGHT as f32 / scale];
    if variant == 2 {
        dimensions = dimensions.map(f32::ceil);
    }
    let shift = if variant == 0 {
        0.0
    } else {
        ((1.0 - scale) * 0.5).min(0.0)
    };
    let values = [
        8.0,
        8.0,
        dimensions[0],
        dimensions[1],
        scale,
        shift,
        0.0,
        0.0,
    ];
    let bytes: Vec<_> = values.into_iter().flat_map(f32::to_ne_bytes).collect();
    let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("aligned native sprite bounds"),
        contents: &bytes,
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let binding = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("sampling variant"),
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(source_view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    });
    let size = wgpu::Extent3d {
        width: WIDTH,
        height: HEIGHT,
        depth_or_array_layers: 1,
    };
    let color = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("sampling readback target"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = color.create_view(&Default::default());
    let pitch = (WIDTH * 4).div_ceil(256) * 256;
    let output = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("sampling readback"),
        size: u64::from(pitch * HEIGHT),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("isolated alpha sampler"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &binding, &[]);
        pass.draw(0..6, 0..1);
    }
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &color,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &output,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(pitch),
                rows_per_image: Some(HEIGHT),
            },
        },
        size,
    );
    queue.submit([encoder.finish()]);
    depth_probe::read_buffer(device, &output)
        .chunks_exact(pitch as usize)
        .flat_map(|row| row[..WIDTH as usize * 4].to_vec())
        .collect()
}

fn main() {
    let directory = std::env::args().nth(1).expect("captured pair directory");
    let work = Path::new(&directory);
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::VULKAN,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface: None,
    }))
    .expect("hardware Vulkan adapter");
    let info = adapter.get_info();
    assert_ne!(info.device_type, wgpu::DeviceType::Cpu);
    eprintln!("adapter: {info:?}");
    let (device, queue) =
        block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).expect("device");
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("nearest-vs-native sampling"),
        source: wgpu::ShaderSource::Wgsl(SHADER.into()),
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("alpha-mask sampling only"),
        layout: None,
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vertex"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fragment"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba8Unorm,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    });
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("same nearest filtering"),
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    });
    println!("source,zoom_index,scale,variant,pixels,opaque,alpha_differences_native");
    for name in ["parent", "child"] {
        let rgba = std::fs::read(work.join(format!("{name}.rgba"))).expect("captured RGBA");
        assert_eq!(rgba.len(), (SOURCE_WIDTH * SOURCE_HEIGHT * 4) as usize);
        let source = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("captured 66x52 RGBA"),
            size: wgpu::Extent3d {
                width: SOURCE_WIDTH,
                height: SOURCE_HEIGHT,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &source,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(SOURCE_WIDTH * 4),
                rows_per_image: Some(SOURCE_HEIGHT),
            },
            source.size(),
        );
        let view = source.create_view(&Default::default());
        for (zoom, scale) in [0.25, 0.5, 1.0, 2.0, 4.0, 8.0].into_iter().enumerate() {
            let native = std::fs::read(work.join(format!("native-sampling-{name}-{zoom}.bin")))
                .expect("native Draw ownership");
            assert_eq!(native.len(), (WIDTH * HEIGHT) as usize);
            for (variant, label) in ["nearest", "native_centres", "native_centres_ceil"]
                .into_iter()
                .enumerate()
            {
                let path = work.join(format!("gpu-sampling-{name}-{zoom}-{label}.rgba"));
                assert!(!path.exists(), "preserve a previous run");
                let colors = render(
                    &device,
                    &queue,
                    &pipeline,
                    &view,
                    &sampler,
                    scale,
                    variant as u32,
                );
                let occupancy: Vec<_> = colors
                    .chunks_exact(4)
                    .map(|rgba| u8::from(rgba[3] != 0))
                    .collect();
                let differences = occupancy
                    .iter()
                    .zip(&native)
                    .filter(|(a, b)| a != b)
                    .count();
                let opaque: usize = occupancy.iter().map(|&a| a as usize).sum();
                std::fs::write(path, &colors).expect("preserve GPU readback");
                println!(
                    "{name},{zoom},{scale},{label},{},{opaque},{differences}",
                    occupancy.len()
                );
                if scale <= 1.0 || variant == 2 {
                    assert_eq!(
                        differences, 0,
                        "aligned sampling contract: {name}/{zoom}/{label}"
                    );
                }
            }
        }
    }
}
