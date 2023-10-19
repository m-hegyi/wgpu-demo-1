use std::{mem, slice};
use wgpu::{
    util::DeviceExt, BindGroup, Buffer, ColorTargetState, Device, Queue, RenderPass,
    RenderPipeline, ShaderModule, Texture, TextureFormat,
};

use crate::{Camera, CameraUniform};

#[repr(C)]
#[derive(Clone, Debug)]
struct Vertex {
    pos: [f32; 3],
    tex_coords: [f32; 2],
}

#[repr(C)]
#[derive(Clone)]
struct Instance {
    position: cgmath::Vector3<f32>,
    rotation: cgmath::Quaternion<f32>,
}

#[repr(C)]
struct InstanceRaw {
    model: [[f32; 4]; 4],
}

impl Instance {
    fn to_raw(&self) -> InstanceRaw {
        InstanceRaw {
            model: (cgmath::Matrix4::from_translation(self.position)
                * cgmath::Matrix4::from(self.rotation))
            .into(),
        }
    }
}

pub struct Pentagon {
    camera_uniform: CameraUniform,
    vertices: Vec<Vertex>,
    indices: Vec<i32>,
    render_pipeline: RenderPipeline,
    vertex_buffer: Buffer,
    index_buffer: Buffer,
    diffuse_bind_group: BindGroup,
    camera_buffer: Buffer,
    camera_bind_group: BindGroup,
    elapsed_time_buffer: Buffer,
    elapsed_time_bind_group: BindGroup,
    instance_buffer: Buffer,
    instances: Vec<Instance>,
}

impl Pentagon {
    pub fn new(
        device: &Device,
        shader: &ShaderModule,
        swapchain_format: &TextureFormat,
        diffuse_texture: &Texture,
        camera: &Camera,
    ) -> Self {
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

        let vertex_buffers = [
            wgpu::VertexBufferLayout {
                array_stride: mem::size_of::<Vertex>() as wgpu::BufferAddress,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &[
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x3,
                        offset: 0,
                        shader_location: 0,
                    },
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x2,
                        offset: mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                        shader_location: 1,
                    },
                ],
            },
            wgpu::VertexBufferLayout {
                array_stride: mem::size_of::<InstanceRaw>() as wgpu::BufferAddress,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &[
                    wgpu::VertexAttribute {
                        offset: 0,
                        shader_location: 5,
                        format: wgpu::VertexFormat::Float32x4,
                    },
                    wgpu::VertexAttribute {
                        offset: mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                        shader_location: 6,
                        format: wgpu::VertexFormat::Float32x4,
                    },
                    wgpu::VertexAttribute {
                        offset: mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                        shader_location: 7,
                        format: wgpu::VertexFormat::Float32x4,
                    },
                    wgpu::VertexAttribute {
                        offset: mem::size_of::<[f32; 12]>() as wgpu::BufferAddress,
                        shader_location: 8,
                        format: wgpu::VertexFormat::Float32x4,
                    },
                ],
            },
        ];

        let mut camera_uniform = CameraUniform::new();

        camera_uniform.update_view_proj(&camera);

        let instances = Pentagon::create_instances(10);

        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("camera_bind_group_layout"),
            });

        let elapsed_time_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("elapsed_time_bind_group_layout"),
            });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[
                &texture_bind_group_layout,
                &camera_bind_group_layout,
                &elapsed_time_bind_group_layout,
            ],
            push_constant_ranges: &[],
        });

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
                targets: &[Some(ColorTargetState::from(*swapchain_format))],
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
        // index buffer
        //

        let vertices = [
            Vertex {
                pos: [-0.0868241, 0.49240386, 0.0],
                tex_coords: [0.4131759, 0.00759614],
            }, // A
            Vertex {
                pos: [-0.49513406, 0.06958647, 0.0],
                tex_coords: [0.0048659444, 0.43041354],
            }, // B
            Vertex {
                pos: [-0.21918549, -0.44939706, 0.0],
                tex_coords: [0.28081453, 0.949397],
            }, // C
            Vertex {
                pos: [0.35966998, -0.3473291, 0.0],
                tex_coords: [0.85967, 0.84732914],
            }, // D
            Vertex {
                pos: [0.44147372, 0.2347359, 0.0],
                tex_coords: [0.9414737, 0.2652641],
            }, // E
        ]
        .to_vec();

        let vertex_buffer_size = crate::next_copy_buffer_size(4096);
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Test vertex non initialized buffer"),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
            size: vertex_buffer_size,
        });

        let indices = [0, 1, 4, 1, 2, 4, 2, 3, 4].to_vec();

        let index_buffer_size = crate::next_copy_buffer_size(4096);
        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Test indices non initialize buffer"),
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
            size: index_buffer_size,
        });

        let diffuse_texture_view =
            diffuse_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let diffuse_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
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

        // TODO: it can be a serious issue
        let camera_raw = unsafe {
            slice::from_raw_parts(
                camera_uniform.view_proj.as_ptr() as *const u8,
                mem::size_of::<CameraUniform>(),
            )
        };

        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera buffer"),
            contents: camera_raw,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
            label: Some("camera_bind_group"),
        });

        let start_time: [u8; 4] = [0, 0, 0, 0];

        let elapsed_time_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Elapsed time buffer"),
            contents: &start_time,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let elapsed_time_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &elapsed_time_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: elapsed_time_buffer.as_entire_binding(),
            }],
            label: Some("Elapsed time bind group"),
        });

        let instance_raw = instances.iter().map(Instance::to_raw).collect::<Vec<_>>();

        let instance_raw = unsafe {
            slice::from_raw_parts(
                instance_raw.as_ptr() as *const u8,
                mem::size_of::<InstanceRaw>() * instance_raw.len(),
            )
        };

        let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Insatnce buffer"),
            contents: instance_raw,
            usage: wgpu::BufferUsages::VERTEX,
        });

        // write queue

        Pentagon {
            camera_uniform,
            vertices,
            indices,
            render_pipeline,
            vertex_buffer,
            index_buffer,
            diffuse_bind_group,
            camera_bind_group,
            camera_buffer,
            elapsed_time_buffer,
            elapsed_time_bind_group,
            instances,
            instance_buffer,
        }
    }

    pub fn prepare(&mut self, queue: &Queue, camera: Option<&Camera>, elapsed_time: f32) {
        let vertices_raw = unsafe {
            slice::from_raw_parts(
                self.vertices.as_ptr() as *const u8,
                mem::size_of::<Vertex>() * self.vertices.len(),
            )
        };

        let indices_raw = unsafe {
            slice::from_raw_parts(
                self.indices.as_slice() as *const _ as *const u8,
                mem::size_of::<Vertex>() * self.indices.len(),
            )
        };

        match camera {
            Some(camera) => {
                let _ = &self.camera_uniform.update_view_proj(camera);

                let camera_raw = unsafe {
                    slice::from_raw_parts(
                        self.camera_uniform.view_proj.as_ptr() as *const u8,
                        mem::size_of::<CameraUniform>(),
                    )
                };

                queue.write_buffer(&self.camera_buffer, 0, camera_raw);
            }
            _ => {}
        }

        // TODO: elapsed writing
        queue.write_buffer(&self.elapsed_time_buffer, 0, &elapsed_time.to_ne_bytes());

        queue.write_buffer(&self.vertex_buffer, 0, vertices_raw);
        queue.write_buffer(&self.index_buffer, 0, indices_raw);
    }

    pub fn render<'rpass>(&'rpass self, render_pass: &mut RenderPass<'rpass>) {
        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(0, &self.diffuse_bind_group, &[]);

        render_pass.set_bind_group(1, &self.camera_bind_group, &[]);

        render_pass.set_bind_group(2, &self.elapsed_time_bind_group, &[]);

        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.draw_indexed(
            0..self.indices.len() as u32,
            0,
            0..self.instances.len() as _,
        );
    }

    fn create_instances(num_row_instances: usize) -> Vec<Instance> {
        use cgmath::prelude::*;

        const NUM_INSTANCES_PER_ROW: u32 = 10;

        const INSTANCE_DISPLACEMENT: cgmath::Vector3<f32> = cgmath::Vector3::new(
            NUM_INSTANCES_PER_ROW as f32 * 0.5,
            0.0,
            NUM_INSTANCES_PER_ROW as f32 * 0.5,
        );

        (0..num_row_instances)
            .flat_map(|z| {
                (0..NUM_INSTANCES_PER_ROW).map(move |x| {
                    let position = cgmath::Vector3 {
                        x: x as f32,
                        y: 0.0,
                        z: z as f32,
                    } - INSTANCE_DISPLACEMENT;

                    let rotation = if position.is_zero() {
                        // this is needed so an object at (0, 0, 0) won't get scaled to zero
                        // as Quaternions can effect scale if they're not created correctly
                        cgmath::Quaternion::from_axis_angle(
                            cgmath::Vector3::unit_z(),
                            cgmath::Deg(0.0),
                        )
                    } else {
                        cgmath::Quaternion::from_axis_angle(position.normalize(), cgmath::Deg(45.0))
                    };

                    Instance { position, rotation }
                })
            })
            .collect()
    }
}
