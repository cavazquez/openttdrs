//! Captured binary-alpha pair: GPU ownership vs the original blitter oracle.
//! This isolates depth and nearest sampling; it is not a whole-scene oracle.
#[allow(dead_code)]
#[path = "probe_projected_mask_depth.rs"]
mod depth_probe;
use bevy::math::Mat4;
use bevy::tasks::block_on;
use std::path::Path;
use wgpu::util::DeviceExt;
const WIDTH: u32 = 132;
const HEIGHT: u32 = 104;
const SHADER: &str = r"
struct Params { clip_from_world: mat4x4<f32>, world_position: vec4<f32>, color: vec4<f32>, rank: vec4<u32> }
@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var image: texture_2d<f32>;
@group(0) @binding(2) var nearest: sampler;
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}
@vertex fn vertex(@builtin(vertex_index) index: u32) -> VertexOutput {
    let xy = array<vec2<f32>, 6>(vec2(-1.0,-1.0),vec2(1.0,-1.0),vec2(1.0,1.0),vec2(-1.0,-1.0),vec2(1.0,1.0),vec2(-1.0,1.0));
    let uv = array<vec2<f32>, 6>(vec2(0.0,1.0),vec2(1.0,1.0),vec2(1.0,0.0),vec2(0.0,1.0),vec2(1.0,0.0),vec2(0.0,0.0));
    let projected = params.clip_from_world * params.world_position;
    var output: VertexOutput;
    output.position = mask_ranked_clip_position(vec4(xy[index], projected.z, projected.w), params.rank.x, true);
    output.uv = uv[index];
    return output;
}
@fragment fn fragment(input: VertexOutput) -> @location(0) vec4<f32> {
    var color = textureSample(image, nearest, input.uv) * params.color;
    if color.a < 0.5 { discard; }
    color.a = 1.0;
    return color;
}
";
fn render(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    pipeline: &wgpu::RenderPipeline,
    matrix: Mat4,
    images: &[Vec<u8>; 2],
    order: &[usize],
    depths: [f32; 2],
    tags: [u32; 2],
) -> (Vec<u8>, Vec<u8>) {
    let size = wgpu::Extent3d {
        width: WIDTH,
        height: HEIGHT,
        depth_or_array_layers: 1,
    };
    let texture = |format, size, usage| {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some("isolated captured pair"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        })
    };
    let usage = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC;
    let color = texture(wgpu::TextureFormat::Rgba8Unorm, size, usage);
    let depth = texture(wgpu::TextureFormat::Depth32Float, size, usage);
    let color_view = color.create_view(&Default::default());
    let depth_view = depth.create_view(&Default::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("nearest"),
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    });
    let layout = pipeline.get_bind_group_layout(0);
    let bindings: Vec<_> = images
        .iter()
        .enumerate()
        .map(|(index, rgba)| {
            assert_eq!(rgba.len(), 66 * 52 * 4);
            let source = texture(
                wgpu::TextureFormat::Rgba8UnormSrgb,
                wgpu::Extent3d {
                    width: 66,
                    height: 52,
                    depth_or_array_layers: 1,
                },
                wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            );
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &source,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                rgba,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(66 * 4),
                    rows_per_image: Some(52),
                },
                source.size(),
            );
            let source_view = source.create_view(&Default::default());
            let mut values = matrix.to_cols_array().to_vec();
            values.extend([34.0, -3926.0, depths[index], 1.0]);
            values.extend(if index == 0 {
                [0.0, 0.0, 0.0, 1.0]
            } else {
                [1.0, 0.0, 0.0, 1.0]
            });
            let mut bytes: Vec<_> = values.into_iter().flat_map(f32::to_ne_bytes).collect();
            bytes.extend(
                [tags[index], 0, 0, 0]
                    .into_iter()
                    .flat_map(u32::to_ne_bytes),
            );
            let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("captured matrix"),
                contents: &bytes,
                usage: wgpu::BufferUsages::UNIFORM,
            });
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("textured plane"),
                layout: &layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: uniform.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&source_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                ],
            })
        })
        .collect();
    // Copy rows are padded to 256 bytes, as required by WebGPU.
    let pitch = (WIDTH * 4).div_ceil(256) * 256;
    let output = || {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pair readback"),
            size: u64::from(pitch * HEIGHT),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        })
    };
    let color_output = output();
    let depth_output = output();
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("pair order"),
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
            pass.draw(0..6, 0..1);
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
                    bytes_per_row: Some(pitch),
                    rows_per_image: Some(HEIGHT),
                },
            },
            size,
        );
    }
    queue.submit([encoder.finish()]);
    let strip = |bytes: Vec<u8>| {
        bytes
            .chunks_exact(pitch as usize)
            .flat_map(|row| row[..(WIDTH * 4) as usize].iter().copied())
            .collect()
    };
    (
        strip(depth_probe::read_buffer(device, &color_output)),
        strip(depth_probe::read_buffer(device, &depth_output)),
    )
}
fn main() {
    let argument = std::env::args()
        .nth(1)
        .expect("directory with captured pair and native oracle");
    let directory = Path::new(&argument);
    let images = [
        std::fs::read(directory.join("parent.rgba")).expect("parent"),
        std::fs::read(directory.join("child.rgba")).expect("child"),
    ];
    let native =
        std::fs::read(directory.join("native-parent-child.bin")).expect("run native oracle first");
    assert_eq!(native.len(), (WIDTH * HEIGHT) as usize);
    let expected: Vec<u8> = native
        .iter()
        .map(|&tag| match tag {
            0 => 0,
            1 => 1,
            2 | 3 => 2,
            _ => panic!("native ownership tag"),
        })
        .collect();
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::VULKAN,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface: None,
    }))
    .expect("Vulkan hardware adapter");
    assert_ne!(adapter.get_info().device_type, wgpu::DeviceType::Cpu);
    eprintln!("adapter: {:?}", adapter.get_info());
    let (device, queue) =
        block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).expect("device");
    let production = include_str!("probe_mask_rank_mapping.wgsl");
    let mapping = production
        .split("// BEGIN_MASK_DEPTH_MAPPING\n")
        .nth(1)
        .expect("mapping start")
        .split("// END_MASK_DEPTH_MAPPING")
        .next()
        .expect("mapping end");
    let shader_source = format!("{mapping}\n{SHADER}");
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("textured mask pair"),
        source: wgpu::ShaderSource::Wgsl(shader_source.into()),
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("same Mask depth rule"),
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
    let current = depth_probe::captured_clip_from_world();
    let mut precise = current;
    precise.z_axis.z = 1.0 / 4000.0;
    precise.w_axis.z = 0.0;
    println!(
        "projection,order,pixels,ownership_differences_native,background,parent,glass,min_nonzero_depth_bits,max_nonzero_depth_bits"
    );
    let captured = [f32::from_bits(1075726681), f32::from_bits(1075726728)];
    for (name, matrix, depths, tags, clipped, tied) in [
        ("captured", current, captured, [0, 0], false, true),
        ("zero_depth_offset", precise, captured, [0, 0], false, false),
        (
            "ranked_depth",
            current,
            captured,
            [0x80000001, 0x80000002],
            false,
            false,
        ),
        (
            "ranked_negative",
            current,
            [-97.56, -97.5596],
            [0x80000001, 0x80000002],
            false,
            false,
        ),
        (
            "ranked_max",
            current,
            captured,
            [0x807ffffe, 0x807fffff],
            false,
            false,
        ),
        (
            "ranked_equal_world",
            current,
            [captured[0]; 2],
            [0x80000001; 2],
            false,
            true,
        ),
        (
            "ranked_below_clip",
            current,
            [-1001.0, -1000.5],
            [0x80000001, 0x80000002],
            true,
            false,
        ),
        (
            "ranked_above_clip",
            current,
            [3001.0, 3002.0],
            [0x80000001, 0x80000002],
            true,
            false,
        ),
    ] {
        for (label, order) in [("parent_child", [0, 1]), ("child_parent", [1, 0])] {
            let (colors, depths) = render(
                &device, &queue, &pipeline, matrix, &images, &order, depths, tags,
            );
            let actual: Vec<u8> = colors
                .chunks_exact(4)
                .map(|rgba| {
                    if rgba[3] == 0 {
                        0
                    } else if rgba[0] > 127 {
                        2
                    } else {
                        1
                    }
                })
                .collect();
            let differences = actual.iter().zip(&expected).filter(|(a, b)| a != b).count();
            let mut counts = [0; 3];
            for &tag in &actual {
                counts[tag as usize] += 1;
            }
            let written: Vec<u32> = depths
                .chunks_exact(4)
                .map(|b| u32::from_ne_bytes(b.try_into().expect("depth bits")))
                .filter(|&b| b != 0)
                .collect();
            assert_eq!(
                counts[0],
                if clipped {
                    (WIDTH * HEIGHT) as usize
                } else {
                    9616
                }
            );
            if clipped {
                assert!(actual.iter().all(|&tag| tag == 0));
                assert!(written.is_empty());
            } else if tied {
                assert_eq!(differences, if order[1] == 1 { 0 } else { 68 });
            } else {
                assert_eq!(
                    differences, 0,
                    "separated projected depths must retain child ownership in either order"
                );
            }
            std::fs::write(directory.join(format!("{name}-{label}.rgba")), colors)
                .expect("save GPU readback");
            std::fs::write(directory.join(format!("{name}-{label}.depth")), depths)
                .expect("save depth readback");
            println!(
                "{name},{label},{},{differences},{},{},{},{},{}",
                actual.len(),
                counts[0],
                counts[1],
                counts[2],
                written.iter().min().unwrap_or(&0),
                written.iter().max().unwrap_or(&0)
            );
        }
    }
}
