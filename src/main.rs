use cgmath::SquareMatrix;
use std::{borrow::Cow, time::SystemTime};
use wgpu::COPY_BUFFER_ALIGNMENT;
use winit::{
    event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::Window,
};

use crate::components::pentagon::Pentagon;

mod components;

#[rustfmt::skip]
pub const OPENGL_TO_WGPU_MATRIX: cgmath::Matrix4<f32> = cgmath::Matrix4::new(
    1.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 0.5, 0.5,
    0.0, 0.0, 0.0, 1.0,
);

pub struct Camera {
    eye: cgmath::Point3<f32>,
    target: cgmath::Point3<f32>,
    up: cgmath::Vector3<f32>,
    aspect: f32,
    fovy: f32,
    znear: f32,
    zfar: f32,
}

impl Camera {
    fn build_view_projection_matrix(&self) -> cgmath::Matrix4<f32> {
        // 1.
        let view = cgmath::Matrix4::look_at_rh(self.eye, self.target, self.up);
        // 2.
        let proj = cgmath::perspective(cgmath::Deg(self.fovy), self.aspect, self.znear, self.zfar);

        // 3.
        return OPENGL_TO_WGPU_MATRIX * proj * view;
    }

    fn update_aspect(&mut self, aspect: f32) {
        self.aspect = aspect;
    }

    // TODO: replace it with a more complex control
    fn update_eye(&mut self, new_eye_pos: cgmath::Point3<f32>) {
        self.eye = new_eye_pos;
    }
}

impl Default for Camera {
    fn default() -> Self {
        Camera {
            eye: (0.0, 1.3, 6.0).into(),
            target: (0.0, 0.0, 0.0).into(),
            up: cgmath::Vector3::unit_y(),
            aspect: 4.0 as f32 / 3.0 as f32,
            fovy: 45.0,
            znear: 0.1,
            zfar: 100.0,
        }
    }
}

// TODO: Move this to somewhere else
#[repr(C)]
#[derive(Clone, Debug)]
struct Vertex {
    pos: [f32; 2],
    color: [f32; 3],
    has_texture: [f32; 1],
    tex_coords: [f32; 2],
}

#[repr(C)]
struct CameraUniform {
    view_proj: [[f32; 4]; 4],
}

impl CameraUniform {
    fn new() -> Self {
        CameraUniform {
            view_proj: cgmath::Matrix4::identity().into(),
        }
    }

    fn update_view_proj(&mut self, camera: &Camera) {
        self.view_proj = camera.build_view_projection_matrix().into();
    }
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

    let swapchain_capabilities = surface.get_capabilities(&adapter);
    let swapchain_format = swapchain_capabilities.formats[0];

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

    let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Depth texture"),
        size: wgpu::Extent3d {
            width: size.width,
            height: size.height,
            ..Default::default()
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });

    let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());
    // let depth_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
    //     address_mode_u: wgpu::AddressMode::ClampToEdge,
    //     address_mode_v: wgpu::AddressMode::ClampToEdge,
    //     address_mode_w: wgpu::AddressMode::ClampToEdge,
    //     mag_filter: wgpu::FilterMode::Linear,
    //     min_filter: wgpu::FilterMode::Linear,
    //     mipmap_filter: wgpu::FilterMode::Nearest,
    //     compare: Some(wgpu::CompareFunction::LessEqual), // 5.
    //     lod_min_clamp: 0.0,
    //     lod_max_clamp: 100.0,
    //     ..Default::default()
    // });

    let mut camera = Camera::default();

    let mut pentagon_model = Pentagon::new(
        &device,
        &shader,
        &swapchain_format,
        &diffuse_texture,
        &camera,
    );

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

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[&texture_bind_group_layout],
        push_constant_ranges: &[],
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

    pentagon_model.prepare(&queue, None, 0.0);

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

    let now = SystemTime::now();

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
                camera.update_aspect(size.width as f32 / size.height as f32);
                // On macos the window needs to be redrawn manually after resizing
                window.request_redraw();
            }
            Event::RedrawRequested(_) => {
                let elapsed_time: f32 = now.elapsed().unwrap().as_secs_f32();

                pentagon_model.prepare(&queue, Some(&camera), elapsed_time);
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
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                            view: &depth_view,
                            depth_ops: Some(wgpu::Operations {
                                load: wgpu::LoadOp::Clear(1.0),
                                store: true,
                            }),
                            stencil_ops: None,
                        }),
                    });
                    pentagon_model.render(&mut rpass);
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
                event:
                    WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
                                state: ElementState::Pressed,
                                virtual_keycode,
                                ..
                            },
                        ..
                    },
                ..
            } => match virtual_keycode {
                Some(VirtualKeyCode::Left) => {
                    camera.update_eye((camera.eye.x + (-0.1), camera.eye.y, camera.eye.z).into());
                }
                Some(VirtualKeyCode::Right) => {
                    camera.update_eye((camera.eye.x + (0.1), camera.eye.y, camera.eye.z).into());
                }
                Some(VirtualKeyCode::Up) => {
                    camera.update_eye((camera.eye.x, camera.eye.y, camera.eye.z + (-0.02)).into());
                }
                Some(VirtualKeyCode::Down) => {
                    camera.update_eye((camera.eye.x, camera.eye.y, camera.eye.z + (0.02)).into());
                }

                _ => {}
            },

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
