use std::{borrow::Cow, mem, slice};
use wgpu::{util::DeviceExt, BufferUsages, VertexBufferLayout, COPY_BUFFER_ALIGNMENT};
use winit::{
    event::{ElementState, Event, KeyboardInput, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::Window,
};

#[derive(Clone)]
struct Vertex {
    pos: [f32; 2],
    color: [f32; 3],
    hasTexture: f32,
}

async fn run(event_loop: EventLoop<()>, window: Window) {
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
                offset: mem::size_of::<f32>() as wgpu::BufferAddress,
                shader_location: 2,
            },
        ],
    }];

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[],
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
            hasTexture: 0.0,
        }, // A
        Vertex {
            pos: [-0.49513406, 0.06958647],
            color: [0.5028864580325687, 0.0, 0.5028864580325687],
            hasTexture: 0.0,
        }, // B
        Vertex {
            pos: [-0.21918549, -0.44939706],
            color: [0.5028864580325687, 0.0, 0.5028864580325687],
            hasTexture: 0.0,
        }, // C
        Vertex {
            pos: [0.35966998, -0.3473291],
            color: [0.5028864580325687, 0.0, 0.5028864580325687],
            hasTexture: 0.0,
        }, // D
        Vertex {
            pos: [0.44147372, 0.2347359],
            color: [0.5028864580325687, 0.0, 0.5028864580325687],
            hasTexture: 0.0,
        }, // E
    ]
    .to_vec();

    let triangle = [
        Vertex {
            pos: [-1.0, -1.0],
            color: [1.0, 0.0, 0.0],
            hasTexture: 0.0,
        },
        Vertex {
            pos: [-0.5, -0.5],
            color: [0.0, 1.0, 0.0],
            hasTexture: 0.0,
        },
        Vertex {
            pos: [-1.0, 0.0],
            color: [0.0, 0.0, 1.0],
            hasTexture: 0.0,
        },
    ]
    .to_vec();

    let vertices_raw = unsafe {
        slice::from_raw_parts(
            pentagon.as_slice() as *const _ as *const u8,
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

    queue.write_buffer(&vertex_buffer2, 0, traingle_vertices_raw);
    queue.write_buffer(&index_buffer2, 0, triangle_indices_raw);
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
                    rpass.set_vertex_buffer(0, vertex_buffer.slice(..));
                    rpass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    rpass.draw_indexed(0..indices.len() as u32, 0, 0..1);
                }

                {
                    let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: None,
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: true,
                            },
                        })],
                        depth_stencil_attachment: None,
                    });
                    rpass.set_pipeline(&render_pipeline);
                    rpass.set_vertex_buffer(0, vertex_buffer2.slice(..));
                    rpass.set_index_buffer(index_buffer2.slice(..), wgpu::IndexFormat::Uint32);
                    rpass.draw_indexed(0..triangle_indices.len() as u32, 0, 0..1);
                }

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
