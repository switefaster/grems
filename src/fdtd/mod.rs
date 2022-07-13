use wgpu::util::DeviceExt;

pub struct FDTD {
    electric_field_bind_group: wgpu::BindGroup,
    magnetic_field_bind_group: wgpu::BindGroup,
    electric_additive_source_bind_group: wgpu::BindGroup,
    magnetic_additive_source_bind_group: wgpu::BindGroup,
    electric_additive_source_map: wgpu::Texture,
    magnetic_additive_source_map: wgpu::Texture,
    update_magnetic_field_pipeline: wgpu::ComputePipeline,
    update_electric_field_pipeline: wgpu::ComputePipeline,
    grid_dimension: [u32; 3],

    // visualize
    rect_vertices: wgpu::Buffer,
    electric_field_render_bind_group: wgpu::BindGroup,
    render_pipeline: wgpu::RenderPipeline,
}

impl FDTD {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        dx: f32, // micrometer
        dt: f32, // seconds
        dimension: [f32; 3],
        gltfs: Vec<crate::GLTFSettings>,
    ) -> anyhow::Result<Self> {
        let grid_x = (dimension[0] / dx).ceil() as u32;
        let grid_y = (dimension[1] / dx).ceil() as u32;
        let grid_z = (dimension[2] / dx).ceil() as u32;

        let mut common_texture_descriptor = wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: grid_x,
                height: grid_y,
                depth_or_array_layers: grid_z,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D3,
            format: wgpu::TextureFormat::Rgba32Float,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
        };
        let electric_field_texture = device.create_texture(&common_texture_descriptor);
        let electric_field_view =
            electric_field_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let magnetic_field_texture = device.create_texture(&common_texture_descriptor);
        let magnetic_field_view =
            magnetic_field_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut importer = gltf_importer::Importer::new(
            dimension,
            dt,
            dx,
            gltf_importer::MaterialConstants {
                permittivity: physical_constants::VACUUM_ELECTRIC_PERMITTIVITY as _,
                permeability: physical_constants::VACUUM_MAG_PERMEABILITY as _,
                electric_conductivity: 0.0,
                magnetic_conductivity: 0.0,
            },
        );
        for gltf in gltfs {
            importer.load_gltf(
                &gltf.path,
                gltf.scale,
                gltf.position,
                gltf_importer::MaterialConstants {
                    permittivity: physical_constants::VACUUM_ELECTRIC_PERMITTIVITY as f32
                        * (gltf.refractive_index * gltf.refractive_index),
                    permeability: physical_constants::VACUUM_MAG_PERMEABILITY as _,
                    electric_conductivity: 0.0,
                    magnetic_conductivity: 0.0,
                },
            )?;
        }

        let (electric_constants_map, magnetic_constants_map) =
            importer.into_constants_map(device, queue);

        let field_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: None,
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::StorageTexture {
                            access: wgpu::StorageTextureAccess::ReadWrite,
                            format: wgpu::TextureFormat::Rgba32Float,
                            view_dimension: wgpu::TextureViewDimension::D3,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::StorageTexture {
                            access: wgpu::StorageTextureAccess::ReadOnly,
                            format: wgpu::TextureFormat::Rgba32Float,
                            view_dimension: wgpu::TextureViewDimension::D3,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::StorageTexture {
                            access: wgpu::StorageTextureAccess::ReadOnly,
                            format: wgpu::TextureFormat::Rgba32Float,
                            view_dimension: wgpu::TextureViewDimension::D3,
                        },
                        count: None,
                    },
                ],
            });

        let electric_field_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &field_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&electric_field_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&magnetic_field_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&electric_constants_map),
                },
            ],
        });

        let magnetic_field_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &field_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&magnetic_field_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&electric_field_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&magnetic_constants_map),
                },
            ],
        });

        common_texture_descriptor.usage =
            wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::STORAGE_BINDING;

        let electric_additive_source_map = device.create_texture(&common_texture_descriptor);
        let electric_additive_source_view =
            electric_additive_source_map.create_view(&wgpu::TextureViewDescriptor::default());

        let magnetic_additive_source_map = device.create_texture(&common_texture_descriptor);
        let magnetic_additive_source_view =
            magnetic_additive_source_map.create_view(&wgpu::TextureViewDescriptor::default());

        let additive_source_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: None,
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::ReadOnly,
                        format: wgpu::TextureFormat::Rgba32Float,
                        view_dimension: wgpu::TextureViewDimension::D3,
                    },
                    count: None,
                }],
            });

        let electric_additive_source_bind_group =
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &additive_source_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&electric_additive_source_view),
                }],
            });

        let magnetic_additive_source_bind_group =
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &additive_source_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&magnetic_additive_source_view),
                }],
            });

        let common_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[&field_bind_group_layout, &additive_source_layout],
                push_constant_ranges: &[wgpu::PushConstantRange {
                    stages: wgpu::ShaderStages::COMPUTE,
                    range: 0..12,
                }],
            });

        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("FDTD Shader"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::from(std::fs::read_to_string(
                std::env::current_dir()?
                    .join("shader")
                    .join("fdtd")
                    .join("fdtd-3d.wgsl"),
            )?)),
        });

        let update_magnetic_field_pipeline =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: None,
                layout: Some(&common_pipeline_layout),
                module: &shader_module,
                entry_point: "update_magnetic_field",
            });

        let update_electric_field_pipeline =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: None,
                layout: Some(&common_pipeline_layout),
                module: &shader_module,
                entry_point: "update_electric_field",
            });

        let rect = [
            crate::Vertex {
                pos: [-1.0, 1.0],
                tex_coord: [0.0, 0.0],
            },
            crate::Vertex {
                pos: [1.0, 1.0],
                tex_coord: [1.0, 0.0],
            },
            crate::Vertex {
                pos: [-1.0, -1.0],
                tex_coord: [0.0, 1.0],
            },
            crate::Vertex {
                pos: [1.0, 1.0],
                tex_coord: [1.0, 0.0],
            },
            crate::Vertex {
                pos: [-1.0, -1.0],
                tex_coord: [0.0, 1.0],
            },
            crate::Vertex {
                pos: [1.0, -1.0],
                tex_coord: [1.0, 1.0],
            },
        ];

        let rect_vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&rect),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let electric_field_render_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: None,
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D3,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                        count: None,
                    },
                ],
            });

        let electric_field_render_bind_group =
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &electric_field_render_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&electric_field_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(
                            &device.create_sampler(&wgpu::SamplerDescriptor::default()),
                        ),
                    },
                ],
            });

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[&electric_field_render_bind_group_layout],
                push_constant_ranges: &[],
            });

        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Blit Shader"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::from(std::fs::read_to_string(
                std::env::current_dir()?.join("shader").join("blit.wgsl"),
            )?)),
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader_module,
                entry_point: "vs_main",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<crate::Vertex>() as _,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x2,
                        1 => Float32x2
                    ],
                }],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader_module,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Bgra8UnormSrgb,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview: None,
        });

        Ok(Self {
            electric_field_bind_group,
            magnetic_field_bind_group,
            electric_additive_source_map,
            magnetic_additive_source_map,
            electric_additive_source_bind_group,
            magnetic_additive_source_bind_group,
            update_magnetic_field_pipeline,
            update_electric_field_pipeline,
            grid_dimension: [grid_x, grid_y, grid_z],
            rect_vertices,
            electric_field_render_bind_group,
            render_pipeline,
        })
    }

    pub fn step(&self, encoder: &mut wgpu::CommandEncoder) {
        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
        cpass.set_pipeline(&self.update_magnetic_field_pipeline);
        cpass.set_bind_group(0, &self.magnetic_field_bind_group, &[]);
        cpass.set_bind_group(1, &self.magnetic_additive_source_bind_group, &[]);
        cpass.set_push_constants(0, bytemuck::cast_slice(&self.grid_dimension));
        cpass.dispatch_workgroups(
            (self.grid_dimension[0] as f32 / 8.0).ceil() as u32,
            (self.grid_dimension[1] as f32 / 8.0).ceil() as u32,
            (self.grid_dimension[2] as f32 / 8.0).ceil() as u32,
        );
        cpass.set_pipeline(&self.update_electric_field_pipeline);
        cpass.set_bind_group(0, &self.electric_field_bind_group, &[]);
        cpass.set_bind_group(1, &self.electric_additive_source_bind_group, &[]);
        cpass.dispatch_workgroups(
            (self.grid_dimension[0] as f32 / 8.0).ceil() as u32,
            (self.grid_dimension[1] as f32 / 8.0).ceil() as u32,
            (self.grid_dimension[2] as f32 / 8.0).ceil() as u32,
        );
    }

    pub fn visualize<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_vertex_buffer(0, self.rect_vertices.slice(..));
        render_pass.set_bind_group(0, &self.electric_field_render_bind_group, &[]);
        render_pass.draw(0..6, 0..1);
    }

    pub fn get_electric_additive_source_map<'a>(&'a self) -> &'a wgpu::Texture {
        &self.electric_additive_source_map
    }
}

pub mod gltf_importer {

    use std::path::Path;

    use ndarray::ShapeBuilder;
    use rayon::iter::{IntoParallelIterator, ParallelIterator};
    use wgpu::util::DeviceExt;

    #[derive(Clone, Copy)]
    pub struct MaterialConstants {
        pub permittivity: f32,
        pub permeability: f32,
        pub electric_conductivity: f32,
        pub magnetic_conductivity: f32,
    }

    #[derive(Clone, Copy)]
    struct FDTDConstants {
        pub ec1: f32,
        pub ec2: f32,
        pub ec3: f32,
        pub hc1: f32,
        pub hc2: f32,
        pub hc3: f32,
    }

    impl FDTDConstants {
        fn from_material(material: MaterialConstants, dt: f32, dx: f32) -> Self {
            let ec1 = (2.0 * material.permittivity - dt * material.electric_conductivity)
                / (2.0 * material.permittivity + dt * material.electric_conductivity);
            let ec3 =
                -2.0 * dt / (2.0 * material.permittivity + dt * material.electric_conductivity);
            let ec2 = ec3 / dx;
            let hc1 = (2.0 * material.permeability - dt * material.magnetic_conductivity)
                / (2.0 * material.permeability + dt * material.magnetic_conductivity);
            let hc3 =
                -2.0 * dt / (2.0 * material.permeability + dt * material.magnetic_conductivity);
            let hc2 = hc3 / dx;
            Self {
                ec1,
                ec2,
                ec3,
                hc1,
                hc2,
                hc3,
            }
        }
    }

    pub struct Importer {
        grid_dimension: [u32; 3],
        dt: f32,
        dx: f32,
        electric_constants: std::sync::Mutex<ndarray::Array3<nalgebra::Vector4<f32>>>,
        magnetic_constants: std::sync::Mutex<ndarray::Array3<nalgebra::Vector4<f32>>>,
    }
    impl Importer {
        pub fn new(dimension: [f32; 3], dt: f32, dx: f32, background: MaterialConstants) -> Self {
            let grid_x = (dimension[0] / dx).ceil() as u32;
            let grid_y = (dimension[1] / dx).ceil() as u32;
            let grid_z = (dimension[2] / dx).ceil() as u32;

            Self {
                electric_constants: std::sync::Mutex::new(ndarray::Array3::from_elem(
                    (grid_x as usize, grid_y as usize, grid_z as usize).f(),
                    nalgebra::vector![
                        (2.0 * background.permittivity - dt * background.electric_conductivity)
                            / (2.0 * background.permittivity
                                + dt * background.electric_conductivity),
                        -2.0 * dt
                            / (dx
                                * 1e-6
                                * (2.0 * background.permittivity
                                    + dt * background.electric_conductivity)),
                        -2.0 * dt
                            / (2.0 * background.permittivity
                                + dt * background.electric_conductivity),
                        1.0
                    ],
                )),
                magnetic_constants: std::sync::Mutex::new(ndarray::Array3::from_elem(
                    (grid_x as usize, grid_y as usize, grid_z as usize).f(),
                    nalgebra::vector![
                        (2.0 * background.permeability - dt * background.magnetic_conductivity)
                            / (2.0 * background.permeability
                                + dt * background.magnetic_conductivity),
                        -2.0 * dt
                            / (dx
                                * 1e-6
                                * (2.0 * background.permeability
                                    + dt * background.magnetic_conductivity)),
                        -2.0 * dt
                            / (2.0 * background.permeability
                                + dt * background.magnetic_conductivity),
                        1.0
                    ],
                )),
                grid_dimension: [grid_x, grid_y, grid_z],
                dt,
                dx,
            }
        }

        pub fn load_gltf<P: AsRef<Path>>(
            &mut self,
            path: P,
            scale: [f32; 3],
            position: [f32; 3],
            constants: MaterialConstants,
        ) -> anyhow::Result<()> {
            let (document, buffers, _) = gltf::import(path)?;
            let scene = document
                .default_scene()
                .ok_or(anyhow::anyhow!("Default scene required!"))?;
            for node in scene.nodes() {
                self.process_node(
                    node,
                    nalgebra::Matrix4::new_translation(
                        &(nalgebra::Vector3::from(position) / self.dx),
                    ) * nalgebra::Matrix4::new_nonuniform_scaling(
                        &(nalgebra::Vector3::from(scale) / self.dx),
                    ),
                    &buffers,
                    FDTDConstants::from_material(constants, self.dt, self.dx * 1e-6),
                );
            }
            Ok(())
        }

        pub fn into_constants_map(
            self,
            device: &wgpu::Device,
            queue: &wgpu::Queue,
        ) -> (wgpu::TextureView, wgpu::TextureView) {
            let common_desc = wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d {
                    width: self.grid_dimension[0],
                    height: self.grid_dimension[1],
                    depth_or_array_layers: self.grid_dimension[2],
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D3,
                format: wgpu::TextureFormat::Rgba32Float,
                usage: wgpu::TextureUsages::STORAGE_BINDING,
            };

            let electric_constants_map = device
                .create_texture_with_data(
                    queue,
                    &common_desc,
                    bytemuck::cast_slice(
                        self.electric_constants
                            .lock()
                            .unwrap()
                            .as_slice_memory_order()
                            .unwrap(),
                    ),
                )
                .create_view(&wgpu::TextureViewDescriptor::default());
            let magnetic_constants_map = device
                .create_texture_with_data(
                    queue,
                    &common_desc,
                    bytemuck::cast_slice(
                        self.magnetic_constants
                            .lock()
                            .unwrap()
                            .as_slice_memory_order()
                            .unwrap(),
                    ),
                )
                .create_view(&wgpu::TextureViewDescriptor::default());
            (electric_constants_map, magnetic_constants_map)
        }

        fn process_node(
            &mut self,
            node: gltf::Node,
            transform: nalgebra::Matrix4<f32>,
            buffers: &Vec<gltf::buffer::Data>,
            constants: FDTDConstants,
        ) {
            let transform = transform
                * nalgebra::Matrix4::from_iterator(node.transform().matrix().into_iter().flatten());
            if let Some(mesh) = node.mesh() {
                for primitive in mesh.primitives() {
                    let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));
                    let indices: Vec<u32> = match reader.read_indices().unwrap() {
                        gltf::mesh::util::ReadIndices::U8(iter) => iter.map(|d| d as u32).collect(),
                        gltf::mesh::util::ReadIndices::U16(iter) => {
                            iter.map(|d| d as u32).collect()
                        }
                        gltf::mesh::util::ReadIndices::U32(iter) => iter.collect(),
                    };

                    let vertices: Vec<nalgebra::Vector3<f32>> = reader
                        .read_positions()
                        .unwrap()
                        .map(|vertex| {
                            (transform * nalgebra::vector![vertex[0], vertex[1], vertex[2], 1.0])
                                .xyz()
                        })
                        .collect();

                    let mut flag_map: ndarray::Array3<u8> = ndarray::Array3::zeros((
                        self.grid_dimension[0] as usize,
                        self.grid_dimension[1] as usize,
                        self.grid_dimension[2] as usize,
                    ));
                    indices.chunks(3).for_each(|triangle| {
                        let v0 = vertices[triangle[0] as usize];
                        let v1 = vertices[triangle[1] as usize];
                        let v2 = vertices[triangle[2] as usize];
                        let edge1 = v1 - v0;
                        let edge2 = v2 - v0;
                        let ray = nalgebra::vector![0.0f32, 0.0, 1.0];
                        let min_x = v0.x.min(v1.x.min(v2.x)).floor() as u32;
                        let max_x = v0.x.max(v1.x.max(v2.x)).ceil() as u32;
                        let min_y = v0.y.min(v1.y.min(v2.y)).floor() as u32;
                        let max_y = v0.y.max(v1.y.max(v2.y)).ceil() as u32;
                        (min_x..=max_x).for_each(|x| {
                            (min_y..=max_y).for_each(|y| {
                                let p = nalgebra::vector![x as f32, y as f32, 0.0];
                                let denominator =
                                    nalgebra::Matrix3::from_columns(&[edge1, edge2, -ray])
                                        .determinant();
                                let nominator_u =
                                    nalgebra::Matrix3::from_columns(&[p - v0, edge2, -ray])
                                        .determinant();
                                let nominator_v =
                                    nalgebra::Matrix3::from_columns(&[edge1, p - v0, -ray])
                                        .determinant();
                                let nominator_t =
                                    nalgebra::Matrix3::from_columns(&[edge1, edge2, p - v0])
                                        .determinant();
                                if denominator != 0.0 {
                                    let u = nominator_u / denominator;
                                    let v = nominator_v / denominator;
                                    let t = nominator_t / denominator;
                                    if u >= 0.0 && u <= 1.0 && v >= 0.0 && u + v <= 1.0 && t >= 0.0
                                    {
                                        let h = p + ray * t;
                                        let x = h.x.round() as usize;
                                        let y = h.y.round() as usize;
                                        let z = h.z.round() as usize;
                                        flag_map[[x, y, z]] = 1;
                                    }
                                }
                            })
                        });
                    });

                    let accumulator: std::sync::Mutex<ndarray::Array3<u8>> =
                        std::sync::Mutex::new(ndarray::Array3::zeros((
                            self.grid_dimension[0] as usize,
                            self.grid_dimension[1] as usize,
                            self.grid_dimension[2] as usize,
                        )));

                    (0..self.grid_dimension[2]).for_each(|z| {
                        (0..self.grid_dimension[0]).into_par_iter().for_each(|x| {
                            (0..self.grid_dimension[1]).into_par_iter().for_each(|y| {
                                let mut acc_lock = accumulator.lock().unwrap();
                                let idx_x = x as usize;
                                let idx_y = y as usize;
                                let idx_z = z as usize;
                                acc_lock[[idx_x, idx_y, idx_z]] = flag_map[[idx_x, idx_y, idx_z]];
                                if z > 0 {
                                    acc_lock[[idx_x, idx_y, idx_z]] +=
                                        acc_lock[[idx_x, idx_y, idx_z - 1]];
                                }
                                if acc_lock[[idx_x, idx_y, idx_z]] % 2 == 1 {
                                    self.electric_constants.lock().unwrap()
                                        [[idx_x, idx_y, idx_z]] = nalgebra::vector![
                                        constants.ec1,
                                        constants.ec2,
                                        constants.ec3,
                                        1.0
                                    ];
                                    self.magnetic_constants.lock().unwrap()
                                        [[idx_x, idx_y, idx_z]] = nalgebra::vector![
                                        constants.hc1,
                                        constants.hc2,
                                        constants.hc3,
                                        1.0
                                    ];
                                }
                            });
                        })
                    });
                }
            }
            for node in node.children() {
                self.process_node(node, transform, buffers, constants);
            }
        }
    }
}
