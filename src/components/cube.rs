use std::{mem, slice};

use wgpu::{
    util::DeviceExt, BindGroup, BindGroupLayout, Buffer, ColorTargetState, Queue, RenderPipeline,
    ShaderModule, TextureFormat,
};

use crate::{
    core::{
        model::{Model, ModelVertex, Vertex},
        texture,
    },
    resources::{load_model, load_texture},
    Camera, CameraUniform,
};

use super::pentagon::Renderable;

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

pub struct Cube {
    camera_uniform: CameraUniform,
    render_pipeline: RenderPipeline,
    diffuse_bind_group: BindGroup,
    camera_buffer: Buffer,
    camera_bind_group: BindGroup,
    elapsed_time_buffer: Buffer,
    elapsed_time_bind_group: BindGroup,
    instance_buffer: Buffer,
    instances: Vec<Instance>,
    model: Model,
}

impl Cube {
    pub async fn new(
        device: &wgpu::Device,
        shader: &ShaderModule,
        swapchain_format: &TextureFormat,
        camera: &Camera,
        queue: &Queue,
    ) -> Self {
        let diffuse_texture = load_texture("cube-diffuse.jpg", &device, &queue)
            .await
            .unwrap();

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
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
            ModelVertex::desc(),
            wgpu::VertexBufferLayout {
                // TODO: move out
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

        // load model
        let model = load_model("cube.obj", device, queue, &texture_bind_group_layout)
            .await
            .unwrap();

        camera_uniform.update_view_proj(&camera);

        let instances = Cube::create_instances(10);

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
            depth_stencil: Some(wgpu::DepthStencilState {
                format: crate::core::texture::Texture::DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let diffuse_texture_view = diffuse_texture.view;
        let diffuse_sampler = diffuse_texture.sampler;

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

        Cube {
            camera_uniform,
            render_pipeline,
            diffuse_bind_group,
            camera_bind_group,
            camera_buffer,
            elapsed_time_buffer,
            elapsed_time_bind_group,
            instances,
            instance_buffer,
            model,
        }
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
                        x: (x * 3) as f32,
                        y: -2.0,
                        z: (z * 3) as f32,
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

impl Renderable for Cube {
    fn prepare(&mut self, queue: &wgpu::Queue, camera: Option<&crate::Camera>, elapsed_time: f32) {
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
    }

    fn render<'rpass>(&'rpass self, render_pass: &mut wgpu::RenderPass<'rpass>) {
        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(0, &self.diffuse_bind_group, &[]);

        render_pass.set_bind_group(1, &self.camera_bind_group, &[]);

        render_pass.set_bind_group(2, &self.elapsed_time_bind_group, &[]);

        render_pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
        for mesh in &self.model.meshes {
            render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            render_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..mesh.num_elements, 0, 0..self.instances.len() as _);
        }
    }
}
