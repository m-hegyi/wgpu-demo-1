use std::{borrow::Cow, mem, slice};
use wgpu::{util::DeviceExt, BufferUsages, VertexBufferLayout, COPY_BUFFER_ALIGNMENT};
use winit::{
    event::{ElementState, Event, KeyboardInput, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::Window,
};

#[repr(C)]
#[derive(Clone, Debug)]
struct Vertex {
    pos: [f32; 2],
    color: [f32; 3],
    has_texture: [f32; 1],
    tex_coords: [f32; 2],
}

async fn run(event_loop: EventLoop<()>, window: Window) {
    //
    // LOAD IMAGE
    //
    let diffuse_bytes = include_bytes!("happy-tree.png");
    let diffuse_image = image::load_from_memory(diffuse_bytes).unwrap();
    let diffuse_rgba = diffuse_image.to_rgba8();

    use image::GenericImageView;
    let dimensions = diffuse_image.dimensions();

    let size = window.inner_size();

    let instance = wgpu::Instance::default();

    let surface = unsafe { instance.create_surface(&window) }.unwrap();
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            force_fallback_adapter: false,
            // Request an adapter which can render to our surface
            compatible_surface: Some(&surface),
        })
        .await
        .expect("Failed to find an appropriate adapter");

    // Create the logical device and command queue
    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                features: wgpu::Features::empty(),
                // Make sure we use the texture resolution limits from the adapter, so we can support images the size of the swapchain.
                limits: wgpu::Limits::downlevel_webgl2_defaults()
                    .using_resolution(adapter.limits()),
            },
            None,
        )
        .await
        .expect("Failed to create device");

    // Load the shaders from disk
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("shader.wgsl"))),
    });

    let vertex_buffers = [VertexBufferLayout {
        array_stride: mem::size_of::<Vertex>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 0,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x3,
                offset: mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                shader_location: 1,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32,
                offset: mem::size_of::<[f32; 5]>() as wgpu::BufferAddress,
                shader_location: 2,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: mem::size_of::<[f32; 6]>() as wgpu::BufferAddress,
                shader_location: 3,
            },
        ],
    }];

    let texture_size = wgpu::Extent3d {
        width: diffuse_image.width(),
        height: diffuse_image.height(),
        ..Default::default()
    };

    let diffuse_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Diffuse texture"),
        size: texture_size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    let diffuse_texture_view = diffuse_texture.create_view(&wgpu::TextureViewDescriptor::default());
    let diffuse_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Nearest,
        mipmap_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    });

    let texture_bind_group_layout =
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    // This should match the filterable field of the
                    // corresponding Texture entry above.
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
            label: Some("texture_bind_group_layout"),
        });

    let diffuse_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout: &texture_bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&diffuse_texture_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&diffuse_sampler),
            },
        ],
        label: Some("diffuse_bind_group"),
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[&texture_bind_group_layout],
        push_constant_ranges: &[],
    });

    let swapchain_capabilities = surface.get_capabilities(&adapter);
    let swapchain_format = swapchain_capabilities.formats[0];

    let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None,
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "vs_main",
            buffers: &vertex_buffers,
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: "fs_main",
            targets: &[Some(swapchain_format.into())],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: Some(wgpu::Face::Back),
            ..Default::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
    });

    let mut config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: swapchain_format,
        width: size.width,
        height: size.height,
        present_mode: wgpu::PresentMode::Fifo,
        alpha_mode: swapchain_capabilities.alpha_modes[0],
        view_formats: vec![],
    };

    surface.configure(&device, &config);

    let pentagon = [
        Vertex {
            pos: [-0.0868241, 0.49240386],
            color: [0.5028864580325687, 0.0, 0.5028864580325687],
            has_texture: [1.0],
            tex_coords: [0.4131759, 0.00759614],
        }, // A
        Vertex {
            pos: [-0.49513406, 0.06958647],
            color: [0.5028864580325687, 0.0, 0.5028864580325687],
            has_texture: [1.0],
            tex_coords: [0.0048659444, 0.43041354],
        }, // B
        Vertex {
            pos: [-0.21918549, -0.44939706],
            color: [0.5028864580325687, 0.0, 0.5028864580325687],
            has_texture: [1.0],
            tex_coords: [0.28081453, 0.949397],
        }, // C
        Vertex {
            pos: [0.35966998, -0.3473291],
            color: [0.5028864580325687, 0.0, 0.5028864580325687],
            has_texture: [1.0],
            tex_coords: [0.85967, 0.84732914],
        }, // D
        Vertex {
            pos: [0.44147372, 0.2347359],
            color: [0.5028864580325687, 0.0, 0.5028864580325687],
            has_texture: [1.0],
            tex_coords: [0.9414737, 0.2652641],
        }, // E
    ]
    .to_vec();

    let triangle = [
        Vertex {
            pos: [-1.0, -1.0],
            color: [1.0, 0.0, 0.0],
            has_texture: [1.0],
            tex_coords: [0.0, 1.0],
        },
        Vertex {
            pos: [-0.5, -0.5],
            color: [0.0, 1.0, 0.0],
            has_texture: [1.0],
            tex_coords: [0.0, 1.0],
        },
        Vertex {
            pos: [-1.0, 0.0],
            color: [0.0, 0.0, 1.0],
            has_texture: [1.0],
            tex_coords: [0.0, 1.0],
        },
    ]
    .to_vec();

    let vertices_raw = unsafe {
        slice::from_raw_parts(
            pentagon.as_ptr() as *const u8,
            mem::size_of::<Vertex>() * pentagon.len(),
        )
    };

    let traingle_vertices_raw = unsafe {
        slice::from_raw_parts(
            triangle.as_slice() as *const _ as *const u8,
            mem::size_of::<Vertex>() * triangle.len(),
        )
    };

    let vertex_buffer_size = next_copy_buffer_size(4096);
    let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Test vertex non initialized buffer"),
        usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
        mapped_at_creation: false,
        size: vertex_buffer_size,
    });

    let vertex_buffer2 = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Test triangle vertex buffer"),
        usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
        mapped_at_creation: false,
        size: vertex_buffer_size,
    });

    let indices = [0, 1, 4, 1, 2, 4, 2, 3, 4].to_vec();

    let triangle_indices = [0, 1, 2].to_vec();

    let indices_raw = unsafe {
        slice::from_raw_parts(
            indices.as_slice() as *const _ as *const u8,
            mem::size_of::<Vertex>() * indices.len(),
        )
    };

    let triangle_indices_raw = unsafe {
        slice::from_raw_parts(
            triangle_indices.as_slice() as *const _ as *const u8,
            mem::size_of::<Vertex>() * triangle_indices.len(),
        )
    };

    let index_buffer_size = next_copy_buffer_size(4096);
    let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Test indices non initialize buffer"),
        usage: BufferUsages::INDEX | BufferUsages::COPY_DST,
        mapped_at_creation: false,
        size: index_buffer_size,
    });

    let index_buffer2 = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Test indices buffer2"),
        usage: BufferUsages::INDEX | BufferUsages::COPY_DST,
        mapped_at_creation: false,
        size: index_buffer_size,
    });

    queue.write_buffer(&vertex_buffer, 0, vertices_raw);
    queue.write_buffer(&index_buffer, 0, indices_raw);

    // queue.write_buffer(&vertex_buffer2, 0, traingle_vertices_raw);
    // queue.write_buffer(&index_buffer2, 0, triangle_indices_raw);

    queue.write_texture(
        wgpu::ImageCopyTexture {
            texture: &diffuse_texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &diffuse_rgba,
        wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(4 * dimensions.0),
            rows_per_image: Some(dimensions.1),
        },
        texture_size,
    );
    // let index_buffer_init = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
    //     label: Some("Test index buffer"),
    //     contents: INDICES,
    //     usage: BufferUsages::INDEX,
    // });

    event_loop.run(move |event, _, control_flow| {
        // Have the closure take ownership of the resources.
        // `event_loop.run` never returns, therefore we must do this to ensure
        // the resources are properly cleaned up.
        let _ = (&instance, &adapter, &shader, &pipeline_layout);

        *control_flow = ControlFlow::Wait;
        match event {
            Event::WindowEvent {
                event: WindowEvent::Resized(size),
                ..
            } => {
                // Reconfigure the surface with the new size
                config.width = size.width;
                config.height = size.height;
                surface.configure(&device, &config);
                // On macos the window needs to be redrawn manually after resizing
                window.request_redraw();
            }
            Event::RedrawRequested(_) => {
                let frame = surface
                    .get_current_texture()
                    .expect("Failed to acquire next swap chain texture");
                let view = frame
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());
                let mut encoder =
                    device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
                {
                    let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: None,
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                                store: true,
                            },
                        })],
                        depth_stencil_attachment: None,
                    });
                    rpass.set_pipeline(&render_pipeline);
                    rpass.set_bind_group(0, &diffuse_bind_group, &[]);
                    rpass.set_vertex_buffer(0, vertex_buffer.slice(..));
                    rpass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    rpass.draw_indexed(0..indices.len() as u32, 0, 0..1);
                }

                // {
                //     let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                //         label: None,
                //         color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                //             view: &view,
                //             resolve_target: None,
                //             ops: wgpu::Operations {
                //                 load: wgpu::LoadOp::Load,
                //                 store: true,
                //             },
                //         })],
                //         depth_stencil_attachment: None,
                //     });
                //     rpass.set_pipeline(&render_pipeline);
                //     rpass.set_vertex_buffer(0, vertex_buffer2.slice(..));
                //     rpass.set_index_buffer(index_buffer2.slice(..), wgpu::IndexFormat::Uint32);
                //     rpass.draw_indexed(0..triangle_indices.len() as u32, 0, 0..1);
                // }

                queue.submit(Some(encoder.finish()));
                frame.present();
            }
            Event::WindowEvent {
                event:
                    WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
                                state: ElementState::Pressed,
                                virtual_keycode: Some(winit::event::VirtualKeyCode::Escape),
                                ..
                            },
                        ..
                    },
                ..
            } => *control_flow = ControlFlow::Exit,

            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *control_flow = ControlFlow::Exit,
            Event::MainEventsCleared => {
                window.request_redraw();
            }
            _ => {}
        }
    });
}

fn next_copy_buffer_size(size: u64) -> u64 {
    let align_mask = COPY_BUFFER_ALIGNMENT - 1;
    ((size.next_power_of_two() + align_mask) & !align_mask).max(COPY_BUFFER_ALIGNMENT)
}

fn main() {
    env_logger::init();

    let event_loop = EventLoop::new();
    let window = winit::window::Window::new(&event_loop).unwrap();
    pollster::block_on(run(event_loop, window));
}

// use glyphon::{
//     Attrs, Buffer, Color, Family, FontSystem, Metrics, Resolution, Shaping, SwashCache, TextArea,
//     TextAtlas, TextBounds, TextRenderer,
// };
// use wgpu::{
//     CommandEncoderDescriptor, CompositeAlphaMode, DeviceDescriptor, Features, Instance,
//     InstanceDescriptor, Limits, LoadOp, MultisampleState, Operations, PresentMode,
//     RenderPassColorAttachment, RenderPassDescriptor, RequestAdapterOptions, SurfaceConfiguration,
//     TextureFormat, TextureUsages, TextureViewDescriptor,
// };
// use winit::{
//     dpi::LogicalSize,
//     event::{Event, WindowEvent},
//     event_loop::{ControlFlow, EventLoop},
//     window::WindowBuilder,
// };

// fn main() {
//     pollster::block_on(run());
// }

// async fn run() {
//     env_logger::init();
//     // Set up window
//     let (width, height) = (800, 600);
//     let event_loop = EventLoop::new();
//     let window = WindowBuilder::new()
//         .with_inner_size(LogicalSize::new(width as f64, height as f64))
//         .with_title("glyphon hello world")
//         .build(&event_loop)
//         .unwrap();
//     let size = window.inner_size();
//     let scale_factor = window.scale_factor();

//     // Set up surface
//     let instance = Instance::new(InstanceDescriptor::default());
//     let adapter = instance
//         .request_adapter(&RequestAdapterOptions::default())
//         .await
//         .unwrap();
//     let (device, queue) = adapter
//         .request_device(
//             &DeviceDescriptor {
//                 label: None,
//                 features: Features::empty(),
//                 limits: Limits::downlevel_defaults(),
//             },
//             None,
//         )
//         .await
//         .unwrap();
//     let surface = unsafe { instance.create_surface(&window) }.expect("Create surface");
//     let swapchain_format = TextureFormat::Bgra8UnormSrgb;
//     let mut config = SurfaceConfiguration {
//         usage: TextureUsages::RENDER_ATTACHMENT,
//         format: swapchain_format,
//         width: size.width,
//         height: size.height,
//         present_mode: PresentMode::Fifo,
//         alpha_mode: CompositeAlphaMode::Opaque,
//         view_formats: vec![],
//     };
//     surface.configure(&device, &config);

//     // Set up text renderer
//     let mut font_system = FontSystem::new();
//     let mut cache = SwashCache::new();
//     let mut atlas = TextAtlas::new(&device, &queue, swapchain_format);
//     let mut text_renderer =
//         TextRenderer::new(&mut atlas, &device, MultisampleState::default(), None);
//     let mut buffer = Buffer::new(&mut font_system, Metrics::new(30.0, 42.0));

//     let physical_width = (width as f64 * scale_factor) as f32;
//     let physical_height = (height as f64 * scale_factor) as f32;

//     buffer.set_size(&mut font_system, physical_width, physical_height);
//     buffer.set_text(&mut font_system, "Hello world! 👋\nThis is rendered with 🦅 glyphon 🦁\nThe text below should be partially clipped.\na b c d e f g h i j k l m n o p q r s t u v w x y z", Attrs::new().family(Family::SansSerif), Shaping::Advanced);
//     buffer.shape_until_scroll(&mut font_system);

//     event_loop.run(move |event, _, control_flow| {
//         let _ = (&instance, &adapter);

//         *control_flow = ControlFlow::Poll;
//         match event {
//             Event::WindowEvent {
//                 event: WindowEvent::Resized(size),
//                 ..
//             } => {
//                 config.width = size.width;
//                 config.height = size.height;
//                 surface.configure(&device, &config);
//                 window.request_redraw();
//             }
//             Event::RedrawRequested(_) => {
//                 text_renderer
//                     .prepare(
//                         &device,
//                         &queue,
//                         &mut font_system,
//                         &mut atlas,
//                         Resolution {
//                             width: config.width,
//                             height: config.height,
//                         },
//                         [TextArea {
//                             buffer: &buffer,
//                             left: 10.0,
//                             top: 10.0,
//                             scale: 1.0,
//                             bounds: TextBounds {
//                                 left: 0,
//                                 top: 0,
//                                 right: 600,
//                                 bottom: 160,
//                             },
//                             default_color: Color::rgb(255, 255, 255),
//                         }],
//                         &mut cache,
//                     )
//                     .unwrap();

//                 let frame = surface.get_current_texture().unwrap();
//                 let view = frame.texture.create_view(&TextureViewDescriptor::default());
//                 let mut encoder =
//                     device.create_command_encoder(&CommandEncoderDescriptor { label: None });
//                 {
//                     let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
//                         label: None,
//                         color_attachments: &[Some(RenderPassColorAttachment {
//                             view: &view,
//                             resolve_target: None,
//                             ops: Operations {
//                                 load: LoadOp::Clear(wgpu::Color::BLACK),
//                                 store: true,
//                             },
//                         })],
//                         depth_stencil_attachment: None,
//                     });

//                     text_renderer.render(&atlas, &mut pass).unwrap();
//                 }

//                 queue.submit(Some(encoder.finish()));
//                 frame.present();

//                 atlas.trim();
//             }
//             Event::WindowEvent {
//                 event: WindowEvent::CloseRequested,
//                 ..
//             } => *control_flow = ControlFlow::Exit,
//             Event::MainEventsCleared => {
//                 window.request_redraw();
//             }
//             _ => {}
//         }
//     });
// }

// use winit::{
//     event::{Event, WindowEvent},
//     event_loop::EventLoop,
//     window::WindowBuilder,
// };

// fn main() {
//     let event_loop = EventLoop::new();
//     let window = WindowBuilder::new()
//         .with_title("Test Window")
//         .build(&event_loop)
//         .unwrap();

//     event_loop.run(move |event, _, control_flow| {
//         control_flow.set_wait();

//         match event {
//             Event::WindowEvent {
//                 event: WindowEvent::CloseRequested,
//                 ..
//             } => {
//                 control_flow.set_exit();
//             }
//             _ => (),
//         }
//     });
// }