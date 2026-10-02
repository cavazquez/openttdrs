//! GPU reproducer for the captured Kale In2x occlusion projection.
//!
//! Reuses the built Bevy/wgpu dependencies; see the stage 43 evidence for
//! compilation and scope. This is a two-plane depth probe, not a scene oracle.
use bevy::math::{Mat4, Vec3};
use bevy::tasks::block_on;
use std::time::Duration;
use wgpu::util::DeviceExt;

// Captured camera from stage 42's failing In2x trace, without recalculating
// its projection from a hand-selected near/far interval.
const CLIP_FROM_VIEW: [u32; 16] = [
    994888909, 0, 0, 0, 0, 1001786209, 0, 0, 0, 0, 964891247, 0, 2147483648, 2147483648,
    1056964608, 1065353216,
];
const WORLD_FROM_VIEW: [u32; 16] = [
    1065353216, 0, 0, 0, 0, 1065353216, 0, 0, 0, 0, 1065353216, 0, 0, 3313516544, 1148844442,
    1065353216,
];
const PARENT_BITS: u32 = 1075726681;
const CHILD_BITS: u32 = 1075726728;
const SHADER: &str = r"
struct Params {
    clip_from_world: mat4x4<f32>,
    world_position: vec4<f32>,
    color: vec4<f32>,
}
@group(0) @binding(0) var<uniform> params: Params;
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
}
@vertex fn vertex(@builtin(vertex_index) index: u32) -> VertexOutput {
    let vertices = array<vec2<f32>, 3>(vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
    // Bevy mesh2d_position_world_to_clip applies this same multiplication.
    // XY is replaced by a fullscreen triangle so only depth is under test.
    let projected = params.clip_from_world * params.world_position;
    var output: VertexOutput;
    output.position = vec4(vertices[index], projected.z, projected.w);
    output.color = params.color;
    return output;
}
@fragment fn fragment(input: VertexOutput) -> @location(0) vec4<f32> {
    return input.color;
}
";

pub(crate) fn read_buffer(device: &wgpu::Device, buffer: &wgpu::Buffer) -> Vec<u8> {
    let (tx, rx) = std::sync::mpsc::channel();
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            tx.send(result).expect("map callback receiver");
        });
    device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(Duration::from_secs(10)),
        })
        .expect("GPU readback completed");
    rx.recv_timeout(Duration::from_secs(10))
        .expect("map callback")
        .expect("mapped buffer");
    let result = buffer.slice(..).get_mapped_range().to_vec();
    buffer.unmap();
    result
}

fn draw(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    pipeline: &wgpu::RenderPipeline,
    matrix: Mat4,
    depths: [f32; 2],
    order: &[usize],
) -> ([u8; 4], u32) {
    let size = wgpu::Extent3d {
        width: 8,
        height: 8,
        depth_or_array_layers: 1,
    };
    let texture = |format| {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some("mask depth probe target"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        })
    };
    let color = texture(wgpu::TextureFormat::Rgba8Unorm);
    let depth = texture(wgpu::TextureFormat::Depth32Float);
    let color_view = color.create_view(&Default::default());
    let depth_view = depth.create_view(&Default::default());
    let layout = pipeline.get_bind_group_layout(0);
    let bindings: Vec<_> = depths
        .into_iter()
        .enumerate()
        .map(|(index, z)| {
            let mut values = matrix.to_cols_array().to_vec();
            values.extend([0.0, 0.0, z, 1.0]);
            values.extend(if index == 0 {
                [0.0, 0.0, 0.0, 1.0]
            } else {
                [1.0, 0.0, 0.0, 1.0]
            });
            let bytes: Vec<_> = values.into_iter().flat_map(f32::to_ne_bytes).collect();
            let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("captured clip matrix and plane"),
                contents: &bytes,
                usage: wgpu::BufferUsages::UNIFORM,
            });
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("plane"),
                layout: &layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                }],
            })
        })
        .collect();
    let output = || {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: 256 * 8,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        })
    };
    let color_output = output();
    let depth_output = output();
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("ordered planes"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &color_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(0.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        pass.set_pipeline(pipeline);
        for &index in order {
            pass.set_bind_group(0, &bindings[index], &[]);
            pass.draw(0..3, 0..1);
        }
    }
    for (source, destination, aspect) in [
        (&color, &color_output, wgpu::TextureAspect::All),
        (&depth, &depth_output, wgpu::TextureAspect::DepthOnly),
    ] {
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: source,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: destination,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(256),
                    rows_per_image: Some(8),
                },
            },
            size,
        );
    }
    queue.submit([encoder.finish()]);
    let colors = read_buffer(device, &color_output);
    let depths = read_buffer(device, &depth_output);
    let offset = 4 * 256 + 4 * 4;
    (
        colors[offset..offset + 4].try_into().expect("RGBA"),
        u32::from_ne_bytes(depths[offset..offset + 4].try_into().expect("depth f32")),
    )
}

pub(crate) fn captured_clip_from_world() -> Mat4 {
    Mat4::from_cols_array(&CLIP_FROM_VIEW.map(f32::from_bits))
        * Mat4::from_cols_array(&WORLD_FROM_VIEW.map(f32::from_bits)).inverse()
}

fn main() {
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
    assert_ne!(
        info.device_type,
        wgpu::DeviceType::Cpu,
        "software adapter is outside this probe's scope"
    );
    eprintln!("adapter: {info:?}");
    let (device, queue) =
        block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).expect("device");
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("mask projection probe"),
        source: wgpu::ShaderSource::Wgsl(SHADER.into()),
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("same depth comparison as Bevy mask mesh"),
        layout: None,
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vertex"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        primitive: Default::default(),
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::GreaterEqual),
            stencil: Default::default(),
            bias: Default::default(),
        }),
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
    let matrix = captured_clip_from_world();
    let parent = f32::from_bits(PARENT_BITS);
    let child = f32::from_bits(CHILD_BITS);
    assert!(parent < child);
    println!(
        "case,order,parent_world_bits,child_world_bits,parent_cpu_clip_bits,child_cpu_clip_bits,rgba,stored_depth_bits"
    );
    for (name, depths) in [
        ("captured_distinct_depths", [parent, child]),
        ("equal_depth_control", [parent, parent]),
        ("separated_depth_control", [parent, parent + 0.01]),
    ] {
        let cpu = depths.map(|z| matrix.transform_point3(Vec3::new(0.0, 0.0, z)).z.to_bits());
        for (label, order) in [
            ("parent_only", vec![0]),
            ("child_only", vec![1]),
            ("parent_child", vec![0, 1]),
            ("child_parent", vec![1, 0]),
        ] {
            let (color, depth) = draw(&device, &queue, &pipeline, matrix, depths, &order);
            assert!(
                color == [0, 0, 0, 255] || color == [255, 0, 0, 255],
                "plane must cover the sampled pixel"
            );
            if name == "separated_depth_control" && order.len() == 2 {
                assert_eq!(color, [255, 0, 0, 255]);
            }
            if name == "captured_distinct_depths" || name == "equal_depth_control" {
                assert_eq!(
                    depth, 1048597585,
                    "captured projection stores the tied depth"
                );
                let last = *order.last().expect("nonempty draw order");
                assert_eq!(
                    color,
                    if last == 0 {
                        [0, 0, 0, 255]
                    } else {
                        [255, 0, 0, 255]
                    }
                );
            }
            println!(
                "{name},{label},{},{},{},{},{};{};{};{},{depth}",
                depths[0].to_bits(),
                depths[1].to_bits(),
                cpu[0],
                cpu[1],
                color[0],
                color[1],
                color[2],
                color[3]
            );
        }
    }
}
