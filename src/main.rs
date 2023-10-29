use cgmath::SquareMatrix;
use components::cube::Cube;
use std::{borrow::Cow, time::SystemTime};
use winit::{
    event::{ElementState, Event, KeyEvent, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::Window,
};

use crate::{
    components::pentagon::{Pentagon, Renderable},
    core::texture::Texture,
};

mod components;
mod core;
mod resources;

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

    let mut camera = Camera::default();

    let mut pentagon_model = Pentagon::new(&device, &shader, &swapchain_format, &camera, &queue);

    let mut cube_model = Cube::new(&device, &shader, &swapchain_format, &camera, &queue).await;

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

    let mut depth_texture: Texture =
        Texture::create_depth_texture(&device, &config, "Depth Texture");

    surface.configure(&device, &config);

    pentagon_model.prepare(&queue, None, 0.0);

    cube_model.prepare(&queue, None, 0.0);

    // let mut hello_text = Text::new(&device, &queue, swapchain_format);

    // hello_text.buffer.set_size(
    //     &mut hello_text.font_system,
    //     config.width as f32,
    //     config.height as f32,
    // );
    // hello_text.set_text("Hello world! 👋\nThis is rendered with 🦅 glyphon 🦁\nThe text below should be partially clipped.\na b c d e f g h i j k l m n o p q r s t u v w x y z");
    // hello_text
    //     .buffer
    //     .shape_until_scroll(&mut hello_text.font_system);

    let now = SystemTime::now();

    // let mut modifiers = ModifiersState::default();

    event_loop.set_control_flow(ControlFlow::Poll);

    event_loop.run(move |event, elwt| {
        // Have the closure take ownership of the resources.
        // `event_loop.run` never returns, therefore we must do this to ensure
        // the resources are properly cleaned up.
        let _ = (&instance, &adapter, &shader, &pipeline_layout);

        match event {
            Event::WindowEvent {
                event: WindowEvent::Resized(size),
                ..
            } => {
                // Reconfigure the surface with the new size
                config.width = size.width;
                config.height = size.height;
                surface.configure(&device, &config);

                depth_texture.recreate_texture(Texture::create_depth_texture(
                    &device,
                    &config,
                    "Depth texture",
                ));

                camera.update_aspect(size.width as f32 / size.height as f32);
                // On macos the window needs to be redrawn manually after resizing
                window.request_redraw();
            }
            Event::WindowEvent {
                event: WindowEvent::RedrawRequested,
                ..
            } => {
                let elapsed_time: f32 = now.elapsed().unwrap().as_secs_f32();

                pentagon_model.prepare(&queue, Some(&camera), elapsed_time);
                cube_model.prepare(&queue, Some(&camera), elapsed_time);

                // hello_text.prepare(
                //     &device,
                //     &queue,
                //     glyphon::Resolution {
                //         width: size.width,
                //         height: size.height,
                //     },
                // );

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
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                            view: depth_texture.get_view(),
                            depth_ops: Some(wgpu::Operations {
                                load: wgpu::LoadOp::Clear(1.0),
                                store: wgpu::StoreOp::Store,
                            }),
                            stencil_ops: None,
                        }),
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });
                    pentagon_model.render(&mut rpass);
                    cube_model.render(&mut rpass);
                }

                {
                    let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: None,
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });
                    // hello_text.render(&mut rpass);
                }

                queue.submit(Some(encoder.finish()));
                frame.present();
            }
            Event::WindowEvent {
                event:
                    WindowEvent::KeyboardInput {
                        event:
                            KeyEvent {
                                physical_key: PhysicalKey::Code(KeyCode::Escape),
                                ..
                            },
                        ..
                    },
                ..
            } => elwt.exit(),

            // Event::WindowEvent {
            //     event:
            //         WindowEvent::KeyboardInput {
            //             input:
            //                 KeyboardInput {
            //                     state: ElementState::Pressed,
            //                     virtual_keycode,
            //                     ..
            //                 },
            //             ..
            //         },
            //     ..
            // } => match virtual_keycode {
            //     Some(VirtualKeyCode::Left) => {
            //         camera.update_eye((camera.eye.x + (-0.1), camera.eye.y, camera.eye.z).into());
            //     }
            //     Some(VirtualKeyCode::Right) => {
            //         camera.update_eye((camera.eye.x + (0.1), camera.eye.y, camera.eye.z).into());
            //     }
            //     Some(VirtualKeyCode::Up) => {
            //         camera.update_eye((camera.eye.x, camera.eye.y, camera.eye.z + (-0.02)).into());
            //     }
            //     Some(VirtualKeyCode::Down) => {
            //         camera.update_eye((camera.eye.x, camera.eye.y, camera.eye.z + (0.02)).into());
            //     }

            //     _ => {}
            // },
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => elwt.exit(),
            // Event::WindowEvent {
            //     event: WindowEvent::ModifiersChanged(new),
            //     ..
            // } => modifiers = new,

            // Event::WindowEvent { event, .. } => match event {
            //     WindowEvent::KeyboardInput { .. } => {
            //         println!("{modifiers:?}");
            //     }
            //     _ => {}
            // },
            // Event::MainEventsCleared => {
            //     window.request_redraw();
            // }
            _ => {}
        }
    });
}

fn main() {
    env_logger::init();

    let event_loop = EventLoop::new().unwrap();
    // let window = winit::window::WindowBuilder::new()
    //     .with_title("demo-1")
    //     .build(&event_loop)
    //     .unwrap();

    let window = winit::window::Window::new(&event_loop).unwrap();
    pollster::block_on(run(event_loop, window));
}
