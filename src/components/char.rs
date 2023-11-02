use wgpu::util::DeviceExt;

use crate::core::{
    model::ModelVertex,
    model::{Material, Mesh, Model, Vertex},
    texture::Texture,
};

use super::pentagon::Renderable;

pub struct Char {
    pub render_pipeline: wgpu::RenderPipeline,
    pub diffuse_bind_group: wgpu::BindGroup,
    pub model: Model,
}

impl Char {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        shader: &wgpu::ShaderModule,
        pipeline_layout: &wgpu::PipelineLayout,
        texture_bind_group_layout: &wgpu::BindGroupLayout,
        config: &wgpu::SurfaceConfiguration,
    ) -> Self {
        let model = Char::prepare_model(device, queue);
        let (render_pipeline, diffuse_bind_group) = Char::prepare_pipeline_and_bind(
            device,
            shader,
            &model.materials[0].diffuse_texture.view,
            &model.materials[0].diffuse_texture.sampler,
            pipeline_layout,
            texture_bind_group_layout,
            config,
        );

        Char {
            model,
            render_pipeline,
            diffuse_bind_group,
        }
    }
}

impl Renderable for Char {
    fn prepare(&mut self, queue: &wgpu::Queue, camera: Option<&crate::Camera>, elapsed_time: f32) {}

    fn render<'rpass>(&'rpass self, render_pass: &mut wgpu::RenderPass<'rpass>) {
        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(0, &self.diffuse_bind_group, &[]);

        for mesh in &self.model.meshes {
            render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            render_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..mesh.num_elements, 0, 0..1);
        }
    }
}

impl Char {
    fn prepare_model(device: &wgpu::Device, queue: &wgpu::Queue) -> Model {
        let data = [128, 128, 128, 255].to_vec();

        let vertices = [
            ModelVertex {
                pos: [-0.9, 0.9, 0.0],
                tex_coords: [0.0, 0.0],
            }, // A
            ModelVertex {
                pos: [-0.9, -0.9, 0.0],
                tex_coords: [0.0, 1.0],
            }, // B
            ModelVertex {
                pos: [0.9, -0.9, 0.0],
                tex_coords: [1.0, 1.0],
            }, // C
            ModelVertex {
                pos: [0.9, 0.9, 0.0],
                tex_coords: [1.0, 0.0],
            }, // D
        ]
        .to_vec();

        let mut buffer = image::RgbaImage::new(5, 5);

        for x in 0..buffer.width() {
            for y in 0..buffer.height() {
                // if (x % 2 == 0 && y % 2 == 0) || ((x + 1) % 2 == 0 && (y + 1) % 2 == 0) {
                //     buffer.put_pixel(x, y, image::Rgba([230, 230, 230, 255]));
                // } else {
                // }
                buffer.put_pixel(x, y, image::Rgba([0, 0, 0, 210]));
            }
        }

        let white = image::Rgba([255, 255, 255, 255]);

        // A
        let pixels = [
            (1, 1),
            (1, 2),
            (1, 3),
            (1, 4),
            (2, 0),
            (2, 2),
            (3, 1),
            (3, 2),
            (3, 3),
            (3, 4),
        ];

        pixels.into_iter().for_each(|(x, y)| {
            buffer.put_pixel(x, y, white);
        });

        let text_data = include_bytes!("../happy-tree.png").to_vec();

        let diffuse_texture = Texture::from_bytes(
            device,
            queue,
            text_data,
            "char",
            Some(buffer),
            Texture::create_sampler(device, Some(wgpu::FilterMode::Nearest)),
        )
        .unwrap();

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("texture_bind_group_layout"),
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
            });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &texture_bind_group_layout,
            label: None,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&diffuse_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&diffuse_texture.sampler),
                },
            ],
        });

        let mut materials = Vec::with_capacity(1);

        materials.push(Material {
            name: String::from("Char"),
            diffuse_texture,
            bind_group,
        });

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Char vertex buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

        // let indices = [0, 1, 4, 1, 2, 4, 2, 3, 4, /* padding */ 0];

        let indices = [0, 1, 3, 1, 2, 3];

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Char index buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
        });

        let mut meshes = Vec::with_capacity(1);

        meshes.push(Mesh {
            name: String::from("char"),
            vertex_buffer,
            index_buffer,
            num_elements: indices.len() as u32,
            materials: materials.len() as usize,
        });

        Model { meshes, materials }
    }

    fn prepare_pipeline_and_bind(
        device: &wgpu::Device,
        shader: &wgpu::ShaderModule,
        texture_view: &wgpu::TextureView,
        texture_sampler: &wgpu::Sampler,
        pipeline_layout: &wgpu::PipelineLayout,
        texture_bind_group_layout: &wgpu::BindGroupLayout,
        config: &wgpu::SurfaceConfiguration,
    ) -> (wgpu::RenderPipeline, wgpu::BindGroup) {
        let vertex_buffers = [ModelVertex::desc()];

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
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent::OVER,
                        // color: wgpu::BlendComponent {
                        //     src_factor: wgpu::BlendFactor::SrcAlpha,
                        //     dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                        //     operation: wgpu::BlendOperation::Add,
                        // },
                        alpha: wgpu::BlendComponent::REPLACE,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
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

        let diffuse_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(texture_sampler),
                },
            ],
            label: Some("diffuse_bind_group"),
        });

        return (render_pipeline, diffuse_bind_group);
    }
}
