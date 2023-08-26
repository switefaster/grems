mod pml;

use wgpu::util::DeviceExt;

use self::pml::PMLBoundary;

pub type Component = SliceMode;

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub enum SliceMode {
    X = 2,
    Y = 1,
    Z = 0,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub enum FieldType {
    E,
    H,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum BoundaryCondition {
    PML { sigma: f32, alpha: f32, cells: u32 },
    PEC,
    PMC,
}

impl BoundaryCondition {
    pub fn get_extra_grid_extent(&self) -> u32 {
        match *self {
            BoundaryCondition::PML { cells, .. } => cells * 2,
            BoundaryCondition::PEC | BoundaryCondition::PMC => 0,
        }
    }

    pub fn use_pmc(&self) -> u32 {
        match *self {
            BoundaryCondition::PML { .. } | BoundaryCondition::PEC => 0,
            BoundaryCondition::PMC => 1,
        }
    }
}

pub struct VisualizeComponent {
    vertex_shader: wgpu::ShaderModule,
    render_pipeline_layout: wgpu::PipelineLayout,
    rect_vertices: wgpu::Buffer,
    electric_field_render_bind_group: wgpu::BindGroup,
    magnetic_field_render_bind_group: wgpu::BindGroup,
    render_pipeline: wgpu::RenderPipeline,
}

pub struct FDTD {
    workgroup_dispatch: crate::WorkgroupSettings,

    electric_field_bind_group: wgpu::BindGroup,
    electric_field_texture: [wgpu::Texture; 3],
    magnetic_field_bind_group: wgpu::BindGroup,
    magnetic_field_texture: [wgpu::Texture; 3],
    update_magnetic_field_pipeline: wgpu::ComputePipeline,
    update_electric_field_pipeline: wgpu::ComputePipeline,
    electric_field_excitation_bind_group: wgpu::BindGroup,
    magnetic_field_excitation_bind_group: wgpu::BindGroup,
    excite_field_volume_pipeline: wgpu::ComputePipeline,
    excite_field_mode_pipeline: wgpu::ComputePipeline,
    grid_dimension: [u32; 3],
    shift_vector: nalgebra::Vector3<f32>,
    spatial_step: f32,
    temporal_step: f32,
    boundary: BoundaryCondition,
    pml: Option<PMLBoundary>,

    slice_position: f32,
    slice_mode: SliceMode,
    field_view_mode: FieldType,
    scaling_factor: f32,

    // visualize
    visualization: Option<VisualizeComponent>,
}

impl FDTD {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        render_format: Option<wgpu::TextureFormat>,
        dx: f32,
        dt: f32,
        dimension: [[f32; 2]; 3],
        models: Vec<crate::ModelSettings>,
        boundary: BoundaryCondition,
        default_slice: crate::SliceSettings,
        default_shader: &str,
        default_scaling_factor: f32,
        workgroup_dispatch: crate::WorkgroupSettings,
        mode_source_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> anyhow::Result<Self> {
        let step_x = (dimension[0][1] - dimension[0][0]) / dx;
        let step_y = (dimension[1][1] - dimension[1][0]) / dx;
        let step_z = (dimension[2][1] - dimension[2][0]) / dx;

        let grid_x = step_x.ceil() as u32 + boundary.get_extra_grid_extent();
        let grid_y = step_y.ceil() as u32 + boundary.get_extra_grid_extent();
        let grid_z = step_z.ceil() as u32 + boundary.get_extra_grid_extent();

        let common_texture_descriptor = wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: grid_x,
                height: grid_y,
                depth_or_array_layers: grid_z,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D3,
            format: wgpu::TextureFormat::R32Float,
            usage: wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        };
        let electric_field_texture = [
            device.create_texture(&common_texture_descriptor),
            device.create_texture(&common_texture_descriptor),
            device.create_texture(&common_texture_descriptor),
        ];
        let electric_field_view = [
            electric_field_texture[0].create_view(&wgpu::TextureViewDescriptor::default()),
            electric_field_texture[1].create_view(&wgpu::TextureViewDescriptor::default()),
            electric_field_texture[2].create_view(&wgpu::TextureViewDescriptor::default()),
        ];
        let magnetic_field_texture = [
            device.create_texture(&common_texture_descriptor),
            device.create_texture(&common_texture_descriptor),
            device.create_texture(&common_texture_descriptor),
        ];
        let magnetic_field_view = [
            magnetic_field_texture[0].create_view(&wgpu::TextureViewDescriptor::default()),
            magnetic_field_texture[1].create_view(&wgpu::TextureViewDescriptor::default()),
            magnetic_field_texture[2].create_view(&wgpu::TextureViewDescriptor::default()),
        ];

        let mut importer = match boundary {
            BoundaryCondition::PML { sigma, alpha, .. } => gltf_importer::Importer::new(
                dimension,
                dt,
                dx,
                gltf_importer::MaterialConstants {
                    permittivity: 1.0,
                    permeability: 1.0,
                },
                boundary.get_extra_grid_extent(),
                sigma,
                alpha,
            ),
            BoundaryCondition::PEC | BoundaryCondition::PMC => gltf_importer::Importer::new(
                dimension,
                dt,
                dx,
                gltf_importer::MaterialConstants {
                    permittivity: 1.0,
                    permeability: 1.0,
                },
                boundary.get_extra_grid_extent(),
                0.,
                0.,
            ),
        };
        for model in models {
            importer.load_gltf(
                &model.path,
                model.scale,
                model.position,
                gltf_importer::MaterialConstants {
                    permittivity: model.refractive_index * model.refractive_index,
                    permeability: 1.0,
                },
            )?;
        }

        let (electric_constants_map, magnetic_constants_map, pml_constants) =
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
                            format: wgpu::TextureFormat::R32Float,
                            view_dimension: wgpu::TextureViewDimension::D3,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::StorageTexture {
                            access: wgpu::StorageTextureAccess::ReadWrite,
                            format: wgpu::TextureFormat::R32Float,
                            view_dimension: wgpu::TextureViewDimension::D3,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::StorageTexture {
                            access: wgpu::StorageTextureAccess::ReadWrite,
                            format: wgpu::TextureFormat::R32Float,
                            view_dimension: wgpu::TextureViewDimension::D3,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::StorageTexture {
                            access: wgpu::StorageTextureAccess::ReadOnly,
                            format: wgpu::TextureFormat::R32Float,
                            view_dimension: wgpu::TextureViewDimension::D3,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 4,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::StorageTexture {
                            access: wgpu::StorageTextureAccess::ReadOnly,
                            format: wgpu::TextureFormat::R32Float,
                            view_dimension: wgpu::TextureViewDimension::D3,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 5,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::StorageTexture {
                            access: wgpu::StorageTextureAccess::ReadOnly,
                            format: wgpu::TextureFormat::R32Float,
                            view_dimension: wgpu::TextureViewDimension::D3,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 6,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::StorageTexture {
                            access: wgpu::StorageTextureAccess::ReadOnly,
                            format: wgpu::TextureFormat::Rg32Float,
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
                    resource: wgpu::BindingResource::TextureView(&electric_field_view[0]),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&electric_field_view[1]),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&electric_field_view[2]),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&magnetic_field_view[0]),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&magnetic_field_view[1]),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(&magnetic_field_view[2]),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
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
                    resource: wgpu::BindingResource::TextureView(&magnetic_field_view[0]),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&magnetic_field_view[1]),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&magnetic_field_view[2]),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&electric_field_view[0]),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&electric_field_view[1]),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(&electric_field_view[2]),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(&magnetic_constants_map),
                },
            ],
        });

        let update_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[&field_bind_group_layout],
                push_constant_ranges: &[wgpu::PushConstantRange {
                    stages: wgpu::ShaderStages::COMPUTE,
                    range: 0..16,
                }],
            });

        let excite_field_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: None,
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::StorageTexture {
                            access: wgpu::StorageTextureAccess::ReadWrite,
                            format: wgpu::TextureFormat::R32Float,
                            view_dimension: wgpu::TextureViewDimension::D3,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::StorageTexture {
                            access: wgpu::StorageTextureAccess::ReadWrite,
                            format: wgpu::TextureFormat::R32Float,
                            view_dimension: wgpu::TextureViewDimension::D3,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::StorageTexture {
                            access: wgpu::StorageTextureAccess::ReadWrite,
                            format: wgpu::TextureFormat::R32Float,
                            view_dimension: wgpu::TextureViewDimension::D3,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::StorageTexture {
                            access: wgpu::StorageTextureAccess::ReadOnly,
                            format: wgpu::TextureFormat::Rg32Float,
                            view_dimension: wgpu::TextureViewDimension::D3,
                        },
                        count: None,
                    },
                ],
            });

        let electric_field_excitation_bind_group =
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &excite_field_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&electric_field_view[0]),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&electric_field_view[1]),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&electric_field_view[2]),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(&electric_constants_map),
                    },
                ],
            });

        let magnetic_field_excitation_bind_group =
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &excite_field_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&magnetic_field_view[0]),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&magnetic_field_view[1]),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&magnetic_field_view[2]),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(&magnetic_constants_map),
                    },
                ],
            });

        let excite_volume_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[&excite_field_bind_group_layout],
                push_constant_ranges: &[wgpu::PushConstantRange {
                    stages: wgpu::ShaderStages::COMPUTE,
                    range: 0..44,
                }],
            });

        let excite_mode_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[
                    mode_source_bind_group_layout,
                    &excite_field_bind_group_layout,
                ],
                push_constant_ranges: &[wgpu::PushConstantRange {
                    stages: wgpu::ShaderStages::COMPUTE,
                    range: 0..28,
                }],
            });

        // naive preprocess
        let macro_replaced = std::fs::read_to_string(
            std::env::current_dir()?
                .join("shader")
                .join("fdtd")
                .join("fdtd-3d.wgsl"),
        )?
        .replace("WORKGROUP_X", workgroup_dispatch.x.to_string().as_str())
        .replace("WORKGROUP_Y", workgroup_dispatch.y.to_string().as_str())
        .replace("WORKGROUP_Z", workgroup_dispatch.z.to_string().as_str());

        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("FDTD Shader"),
            source: wgpu::ShaderSource::Wgsl(macro_replaced.into()),
        });

        let update_magnetic_field_pipeline =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: None,
                layout: Some(&update_pipeline_layout),
                module: &shader_module,
                entry_point: "update_magnetic_field",
            });

        let update_electric_field_pipeline =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: None,
                layout: Some(&update_pipeline_layout),
                module: &shader_module,
                entry_point: "update_electric_field",
            });

        let volume_excitation_shader_module =
            device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("FDTD Volume Excitation Shader"),
                source: wgpu::ShaderSource::Wgsl(
                    std::fs::read_to_string(
                        std::env::current_dir()?
                            .join("shader")
                            .join("fdtd")
                            .join("excitation-volume.wgsl"),
                    )?
                    .replace("WORKGROUP_X", workgroup_dispatch.x.to_string().as_str())
                    .replace("WORKGROUP_Y", workgroup_dispatch.y.to_string().as_str())
                    .replace("WORKGROUP_Z", workgroup_dispatch.z.to_string().as_str())
                    .into(),
                ),
            });

        let mode_excitation_shader_module =
            device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("FDTD Mode Excitation Shader"),
                source: wgpu::ShaderSource::Wgsl(
                    std::fs::read_to_string(
                        std::env::current_dir()?
                            .join("shader")
                            .join("fdtd")
                            .join("excitation-mode.wgsl"),
                    )?
                    .replace("WORKGROUP_X", workgroup_dispatch.x.to_string().as_str())
                    .replace("WORKGROUP_Y", workgroup_dispatch.y.to_string().as_str())
                    .replace("WORKGROUP_Z", workgroup_dispatch.z.to_string().as_str())
                    .into(),
                ),
            });

        let excite_field_volume_pipeline =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: None,
                layout: Some(&excite_volume_pipeline_layout),
                module: &volume_excitation_shader_module,
                entry_point: "excite_field_volume",
            });

        let excite_field_mode_pipeline =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: None,
                layout: Some(&excite_mode_pipeline_layout),
                module: &mode_excitation_shader_module,
                entry_point: "excite_field_mode",
            });

        let visualization = render_format
            .map::<anyhow::Result<VisualizeComponent>, _>(|render_format| {
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

                let field_render_bind_group_layout =
                    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                        label: None,
                        entries: &[
                            wgpu::BindGroupLayoutEntry {
                                binding: 0,
                                visibility: wgpu::ShaderStages::FRAGMENT,
                                ty: wgpu::BindingType::Texture {
                                    sample_type: wgpu::TextureSampleType::Float {
                                        filterable: false,
                                    },
                                    view_dimension: wgpu::TextureViewDimension::D3,
                                    multisampled: false,
                                },
                                count: None,
                            },
                            wgpu::BindGroupLayoutEntry {
                                binding: 1,
                                visibility: wgpu::ShaderStages::FRAGMENT,
                                ty: wgpu::BindingType::Texture {
                                    sample_type: wgpu::TextureSampleType::Float {
                                        filterable: false,
                                    },
                                    view_dimension: wgpu::TextureViewDimension::D3,
                                    multisampled: false,
                                },
                                count: None,
                            },
                            wgpu::BindGroupLayoutEntry {
                                binding: 2,
                                visibility: wgpu::ShaderStages::FRAGMENT,
                                ty: wgpu::BindingType::Texture {
                                    sample_type: wgpu::TextureSampleType::Float {
                                        filterable: false,
                                    },
                                    view_dimension: wgpu::TextureViewDimension::D3,
                                    multisampled: false,
                                },
                                count: None,
                            },
                            wgpu::BindGroupLayoutEntry {
                                binding: 3,
                                visibility: wgpu::ShaderStages::FRAGMENT,
                                ty: wgpu::BindingType::Sampler(
                                    wgpu::SamplerBindingType::NonFiltering,
                                ),
                                count: None,
                            },
                        ],
                    });

                let electric_field_render_bind_group =
                    device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: None,
                        layout: &field_render_bind_group_layout,
                        entries: &[
                            wgpu::BindGroupEntry {
                                binding: 0,
                                resource: wgpu::BindingResource::TextureView(
                                    &electric_field_view[0],
                                ),
                            },
                            wgpu::BindGroupEntry {
                                binding: 1,
                                resource: wgpu::BindingResource::TextureView(
                                    &electric_field_view[1],
                                ),
                            },
                            wgpu::BindGroupEntry {
                                binding: 2,
                                resource: wgpu::BindingResource::TextureView(
                                    &electric_field_view[2],
                                ),
                            },
                            wgpu::BindGroupEntry {
                                binding: 3,
                                resource: wgpu::BindingResource::Sampler(
                                    &device.create_sampler(&wgpu::SamplerDescriptor::default()),
                                ),
                            },
                        ],
                    });

                let magnetic_field_render_bind_group =
                    device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: None,
                        layout: &field_render_bind_group_layout,
                        entries: &[
                            wgpu::BindGroupEntry {
                                binding: 0,
                                resource: wgpu::BindingResource::TextureView(
                                    &magnetic_field_view[0],
                                ),
                            },
                            wgpu::BindGroupEntry {
                                binding: 1,
                                resource: wgpu::BindingResource::TextureView(
                                    &magnetic_field_view[1],
                                ),
                            },
                            wgpu::BindGroupEntry {
                                binding: 2,
                                resource: wgpu::BindingResource::TextureView(
                                    &magnetic_field_view[2],
                                ),
                            },
                            wgpu::BindGroupEntry {
                                binding: 3,
                                resource: wgpu::BindingResource::Sampler(
                                    &device.create_sampler(&wgpu::SamplerDescriptor::default()),
                                ),
                            },
                        ],
                    });

                let render_pipeline_layout =
                    device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: None,
                        bind_group_layouts: &[&field_render_bind_group_layout],
                        push_constant_ranges: &[{
                            wgpu::PushConstantRange {
                                stages: wgpu::ShaderStages::FRAGMENT,
                                range: 0..12,
                            }
                        }],
                    });

                let vertex_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some(default_shader),
                    source: wgpu::ShaderSource::Wgsl(
                        std::fs::read_to_string(
                            std::env::current_dir()?.join("shader").join("vertex.wgsl"),
                        )?
                        .into(),
                    ),
                });

                let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some(default_shader),
                    source: wgpu::ShaderSource::Wgsl(
                        std::fs::read_to_string(default_shader)?.into(),
                    ),
                });

                let render_pipeline =
                    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                        label: None,
                        layout: Some(&render_pipeline_layout),
                        vertex: wgpu::VertexState {
                            module: &vertex_shader,
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
                                format: render_format,
                                blend: None,
                                write_mask: wgpu::ColorWrites::ALL,
                            })],
                        }),
                        multiview: None,
                    });

                Ok(VisualizeComponent {
                    vertex_shader,
                    render_pipeline_layout,
                    rect_vertices,
                    electric_field_render_bind_group,
                    magnetic_field_render_bind_group,
                    render_pipeline,
                })
            })
            .transpose()?;

        let shift_vector = -nalgebra::vector![
            dimension[0][0] + (step_x - step_x.floor()) * dx * 0.5
                - boundary.get_extra_grid_extent() as f32 * dx * 0.5,
            dimension[1][0] + (step_y - step_y.floor()) * dx * 0.5
                - boundary.get_extra_grid_extent() as f32 * dx * 0.5,
            dimension[2][0] + (step_z - step_z.floor()) * dx * 0.5
                - boundary.get_extra_grid_extent() as f32 * dx * 0.5
        ];

        let grid_dimension = [grid_x, grid_y, grid_z];
        let simulation_dimension = [
            grid_x - boundary.get_extra_grid_extent(),
            grid_y - boundary.get_extra_grid_extent(),
            grid_z - boundary.get_extra_grid_extent(),
        ];

        let pml = match boundary {
            BoundaryCondition::PML {
                sigma,
                alpha,
                cells,
            } => Some(PMLBoundary::new(
                &device,
                cells,
                alpha,
                sigma,
                dt,
                &electric_field_view,
                &magnetic_field_view,
                &electric_constants_map,
                &magnetic_constants_map,
                simulation_dimension,
                pml_constants.unwrap(),
            )),
            BoundaryCondition::PEC | BoundaryCondition::PMC => None,
        };

        Ok(Self {
            electric_field_bind_group,
            magnetic_field_bind_group,
            update_magnetic_field_pipeline,
            update_electric_field_pipeline,
            grid_dimension,
            shift_vector,
            spatial_step: dx,
            excite_field_volume_pipeline,
            slice_position: (default_slice.position
                + match default_slice.mode {
                    SliceMode::X => shift_vector[0],
                    SliceMode::Y => shift_vector[1],
                    SliceMode::Z => shift_vector[2],
                } as f32)
                / (match default_slice.mode {
                    SliceMode::X => grid_x,
                    SliceMode::Y => grid_y,
                    SliceMode::Z => grid_z,
                } as f32
                    - 1.0)
                / dx,
            slice_mode: default_slice.mode,
            field_view_mode: default_slice.field,
            scaling_factor: default_scaling_factor,
            electric_field_texture,
            magnetic_field_texture,
            boundary,
            pml,
            temporal_step: dt,
            workgroup_dispatch,
            visualization,
            electric_field_excitation_bind_group,
            magnetic_field_excitation_bind_group,
            excite_field_mode_pipeline,
        })
    }

    pub fn update_magnetic_field(&self, encoder: &mut wgpu::CommandEncoder) {
        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
        if let BoundaryCondition::PML { .. } = self.boundary {
            let pml = self.pml.as_ref().unwrap();
            pml.update_magnetic_field(&mut cpass);
        }
        cpass.set_pipeline(&self.update_magnetic_field_pipeline);
        cpass.set_bind_group(0, &self.magnetic_field_bind_group, &[]);
        cpass.set_push_constants(0, bytemuck::cast_slice(&self.grid_dimension));
        cpass.set_push_constants(12, bytemuck::cast_slice(&[self.boundary.use_pmc()]));
        cpass.dispatch_workgroups(
            (self.grid_dimension[0] as f32 / self.workgroup_dispatch.x as f32).ceil() as u32,
            (self.grid_dimension[1] as f32 / self.workgroup_dispatch.y as f32).ceil() as u32,
            (self.grid_dimension[2] as f32 / self.workgroup_dispatch.z as f32).ceil() as u32,
        );
    }

    pub fn excite_magnetic_field_volume(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        position: [u32; 3],
        size: [u32; 3],
        strength: [f32; 3],
    ) {
        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
        cpass.set_pipeline(&self.excite_field_volume_pipeline);
        cpass.set_bind_group(0, &self.magnetic_field_excitation_bind_group, &[]);
        cpass.set_push_constants(0, bytemuck::cast_slice(&size));
        cpass.set_push_constants(16, bytemuck::cast_slice(&strength));
        cpass.set_push_constants(32, bytemuck::cast_slice(&position));
        cpass.dispatch_workgroups(
            (size[0] as f32 / self.workgroup_dispatch.x as f32).ceil() as u32,
            (size[1] as f32 / self.workgroup_dispatch.y as f32).ceil() as u32,
            (size[2] as f32 / self.workgroup_dispatch.z as f32).ceil() as u32,
        );
    }

    pub fn excite_magnetic_field_mode(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        position: [u32; 3],
        (sin_t, cos_t): (f32, f32),
        envelope: f32,
        mode_bind_group: &wgpu::BindGroup,
    ) {
        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
        cpass.set_pipeline(&self.excite_field_mode_pipeline);
        cpass.set_bind_group(0, mode_bind_group, &[]);
        cpass.set_bind_group(1, &self.magnetic_field_excitation_bind_group, &[]);
        cpass.set_push_constants(0, bytemuck::cast_slice(&position));
        cpass.set_push_constants(
            12,
            bytemuck::cast_slice(&[cos_t, sin_t, envelope, self.temporal_step]),
        );
        cpass.dispatch_workgroups(
            ((self.grid_dimension[0] - self.boundary.get_extra_grid_extent()) as f32
                / self.workgroup_dispatch.x as f32)
                .ceil() as u32,
            ((self.grid_dimension[1] - self.boundary.get_extra_grid_extent()) as f32
                / self.workgroup_dispatch.y as f32)
                .ceil() as u32,
            1,
        );
    }

    pub fn update_electric_field(&self, encoder: &mut wgpu::CommandEncoder) {
        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
        if let BoundaryCondition::PML { .. } = self.boundary {
            let pml = self.pml.as_ref().unwrap();
            pml.update_electric_field(&mut cpass);
        }
        cpass.set_pipeline(&self.update_electric_field_pipeline);
        cpass.set_bind_group(0, &self.electric_field_bind_group, &[]);
        cpass.set_push_constants(0, bytemuck::cast_slice(&self.grid_dimension));
        cpass.set_push_constants(12, bytemuck::cast_slice(&[self.boundary.use_pmc()]));
        cpass.dispatch_workgroups(
            (self.grid_dimension[0] as f32 / self.workgroup_dispatch.x as f32).ceil() as u32,
            (self.grid_dimension[1] as f32 / self.workgroup_dispatch.y as f32).ceil() as u32,
            (self.grid_dimension[2] as f32 / self.workgroup_dispatch.z as f32).ceil() as u32,
        );
    }

    pub fn excite_electric_field_volume(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        position: [u32; 3],
        size: [u32; 3],
        strength: [f32; 3],
    ) {
        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
        cpass.set_pipeline(&self.excite_field_volume_pipeline);
        cpass.set_bind_group(0, &self.electric_field_excitation_bind_group, &[]);
        cpass.set_push_constants(0, bytemuck::cast_slice(&size));
        cpass.set_push_constants(16, bytemuck::cast_slice(&strength));
        cpass.set_push_constants(32, bytemuck::cast_slice(&position));
        cpass.dispatch_workgroups(
            (size[0] as f32 / self.workgroup_dispatch.x as f32).ceil() as u32,
            (size[1] as f32 / self.workgroup_dispatch.y as f32).ceil() as u32,
            (size[2] as f32 / self.workgroup_dispatch.z as f32).ceil() as u32,
        );
    }

    pub fn excite_electric_field_mode(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        position: [u32; 3],
        (sin_t, cos_t): (f32, f32),
        envelope: f32,
        mode_bind_group: &wgpu::BindGroup,
    ) {
        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
        cpass.set_pipeline(&self.excite_field_mode_pipeline);
        cpass.set_bind_group(0, mode_bind_group, &[]);
        cpass.set_bind_group(1, &self.electric_field_excitation_bind_group, &[]);
        cpass.set_push_constants(0, bytemuck::cast_slice(&position));
        cpass.set_push_constants(
            12,
            bytemuck::cast_slice(&[cos_t, sin_t, envelope, self.temporal_step]),
        );
        cpass.dispatch_workgroups(
            ((self.grid_dimension[0] - self.boundary.get_extra_grid_extent()) as f32
                / self.workgroup_dispatch.x as f32)
                .ceil() as u32,
            ((self.grid_dimension[1] - self.boundary.get_extra_grid_extent()) as f32
                / self.workgroup_dispatch.y as f32)
                .ceil() as u32,
            1,
        );
    }

    pub fn offset_slice_position(&mut self, row_delta: f32) {
        self.slice_position += -row_delta
            * (1.0
                / match self.slice_mode {
                    SliceMode::X => self.grid_dimension[0] - 1,
                    SliceMode::Y => self.grid_dimension[1] - 1,
                    SliceMode::Z => self.grid_dimension[2] - 1,
                } as f32);
        self.slice_position = self.slice_position.min(1.0).max(0.0);
    }

    pub fn set_slice_mode(&mut self, slice_mode: SliceMode) {
        self.slice_mode = slice_mode;
    }

    pub fn get_slice_position(&self) -> f32 {
        let shift = match self.slice_mode {
            SliceMode::X => self.shift_vector[0],
            SliceMode::Y => self.shift_vector[1],
            SliceMode::Z => self.shift_vector[2],
        };
        let dimension = match self.slice_mode {
            SliceMode::X => self.grid_dimension[0],
            SliceMode::Y => self.grid_dimension[1],
            SliceMode::Z => self.grid_dimension[2],
        } as f32;
        self.slice_position * (dimension - 1.0) * self.spatial_step - shift
    }

    pub fn get_slice_position_normalized(&self) -> f32 {
        self.slice_position
    }

    pub fn get_slice_mode(&self) -> SliceMode {
        self.slice_mode
    }

    pub fn set_field_view_mode(&mut self, field_view_mode: FieldType) {
        self.field_view_mode = field_view_mode;
    }

    pub fn get_field_view_mode(&self) -> FieldType {
        self.field_view_mode
    }

    pub fn get_scaling_factor(&self) -> f32 {
        self.scaling_factor
    }

    pub fn scale_linear(&mut self, delta: f32) {
        self.scaling_factor += delta;
        self.scaling_factor = self.scaling_factor.max(0.0);
    }

    pub fn scale_exponential(&mut self, delta_exp: i32) {
        self.scaling_factor *= 10f32.powi(delta_exp);
    }

    pub fn get_electric_field_textures<'a>(&'a self) -> &'a [wgpu::Texture; 3] {
        &self.electric_field_texture
    }

    pub fn get_magnetic_field_textures<'a>(&'a self) -> &'a [wgpu::Texture; 3] {
        &self.magnetic_field_texture
    }

    pub fn get_dimension(&self) -> [u32; 3] {
        self.grid_dimension
    }

    pub fn reload_shader<P: AsRef<std::path::Path>>(
        &mut self,
        path: P,
        device: &wgpu::Device,
        render_format: wgpu::TextureFormat,
    ) -> anyhow::Result<()> {
        if let Some(visualization) = &mut self.visualization {
            let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(path.as_ref().file_name().unwrap().to_str().unwrap()),
                source: wgpu::ShaderSource::Wgsl(std::fs::read_to_string(path.as_ref())?.into()),
            });

            let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: None,
                layout: Some(&visualization.render_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &visualization.vertex_shader,
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
                        format: render_format,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview: None,
            });

            visualization.render_pipeline = render_pipeline;
        }

        Ok(())
    }

    pub fn visualize<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        if let Some(visualization) = &self.visualization {
            render_pass.set_pipeline(&visualization.render_pipeline);
            render_pass.set_vertex_buffer(0, visualization.rect_vertices.slice(..));
            render_pass.set_bind_group(
                0,
                match self.field_view_mode {
                    FieldType::E => &visualization.electric_field_render_bind_group,
                    FieldType::H => &visualization.magnetic_field_render_bind_group,
                },
                &[],
            );
            render_pass.set_push_constants(
                wgpu::ShaderStages::FRAGMENT,
                0,
                bytemuck::cast_slice(&[self.get_slice_position_normalized()]),
            );
            render_pass.set_push_constants(
                wgpu::ShaderStages::FRAGMENT,
                4,
                bytemuck::cast_slice(&[self.slice_mode as u32]),
            );
            render_pass.set_push_constants(
                wgpu::ShaderStages::FRAGMENT,
                8,
                bytemuck::cast_slice(&[self.scaling_factor]),
            );
            render_pass.draw(0..6, 0..1);
        }
    }
}

pub mod gltf_importer {

    use std::path::Path;

    use ndarray::ShapeBuilder;
    use rayon::{
        iter::{IntoParallelIterator, ParallelIterator},
        prelude::ParallelBridge,
    };
    use wgpu::util::DeviceExt;

    #[derive(Clone, Copy)]
    pub struct MaterialConstants {
        pub permittivity: f32,
        pub permeability: f32,
    }

    #[derive(Clone, Copy)]
    struct FDTDConstants {
        pub ec2: f32,
        pub ec3: f32,
        pub hc2: f32,
        pub hc3: f32,
    }

    impl FDTDConstants {
        fn from_material(material: MaterialConstants, dt: f32, dx: f32) -> Self {
            let ec3 = dt / material.permittivity;
            let ec2 = ec3 / dx;
            let hc3 = dt / material.permeability;
            let hc2 = hc3 / dx;
            Self { ec2, ec3, hc2, hc3 }
        }
    }

    pub struct Importer {
        grid_dimension: [u32; 3],
        dt: f32,
        dx: f32,
        electric_constants: ndarray::Array3<std::sync::Mutex<nalgebra::Vector2<f32>>>,
        magnetic_constants: ndarray::Array3<std::sync::Mutex<nalgebra::Vector2<f32>>>,
        shift_vector: nalgebra::Vector3<f32>,
        extra_extent: u32,
        pml_sigma: f32,
        pml_alpha: f32,
    }

    impl Importer {
        pub fn new(
            dimension: [[f32; 2]; 3],
            dt: f32,
            dx: f32,
            background: MaterialConstants,
            extra_extent: u32,
            pml_sigma: f32,
            pml_alpha: f32,
        ) -> Self {
            let step_x = (dimension[0][1] - dimension[0][0]) / dx;
            let step_y = (dimension[1][1] - dimension[1][0]) / dx;
            let step_z = (dimension[2][1] - dimension[2][0]) / dx;
            let grid_x = step_x.ceil() as u32 + extra_extent;
            let grid_y = step_y.ceil() as u32 + extra_extent;
            let grid_z = step_z.ceil() as u32 + extra_extent;

            Self {
                electric_constants: ndarray::Array3::from_shape_simple_fn(
                    (grid_x as usize, grid_y as usize, grid_z as usize).f(),
                    || {
                        std::sync::Mutex::new(nalgebra::vector![
                            dt / (dx * background.permittivity),
                            dt / background.permittivity
                        ])
                    },
                ),
                magnetic_constants: ndarray::Array3::from_shape_simple_fn(
                    (grid_x as usize, grid_y as usize, grid_z as usize).f(),
                    || {
                        std::sync::Mutex::new(nalgebra::vector![
                            dt / (dx * background.permeability),
                            dt / background.permeability
                        ])
                    },
                ),
                grid_dimension: [grid_x, grid_y, grid_z],
                dt,
                dx,
                shift_vector: -nalgebra::vector![
                    dimension[0][0] + (step_x - step_x.floor()) * dx * 0.5
                        - extra_extent as f32 * dx * 0.5,
                    dimension[1][0] + (step_y - step_y.floor()) * dx * 0.5
                        - extra_extent as f32 * dx * 0.5,
                    dimension[2][0] + (step_z - step_z.floor()) * dx * 0.5
                        - extra_extent as f32 * dx * 0.5
                ],
                extra_extent,
                pml_sigma,
                pml_alpha,
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
                    nalgebra::Matrix4::new_translation(&(self.shift_vector / self.dx))
                        * nalgebra::Matrix4::new_translation(
                            &(nalgebra::Vector3::from(position) / self.dx),
                        )
                        * nalgebra::Matrix4::new_nonuniform_scaling(
                            &(nalgebra::Vector3::from(scale) / self.dx),
                        ),
                    &buffers,
                    FDTDConstants::from_material(constants, self.dt, self.dx),
                );
            }
            Ok(())
        }

        pub fn into_constants_map(
            self,
            device: &wgpu::Device,
            queue: &wgpu::Queue,
        ) -> (
            wgpu::TextureView,
            wgpu::TextureView,
            Option<([wgpu::TextureView; 6], [wgpu::TextureView; 6])>,
        ) {
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
                format: wgpu::TextureFormat::Rg32Float,
                usage: wgpu::TextureUsages::STORAGE_BINDING,
                view_formats: &[],
            };

            let mut ec_map = ndarray::Zip::from(&self.electric_constants)
                .par_map_collect(|mutex| *mutex.lock().unwrap());

            let mut hc_map = ndarray::Zip::from(&self.magnetic_constants)
                .par_map_collect(|mutex| *mutex.lock().unwrap());

            let mut pml_constants = None;

            if self.extra_extent > 0 {
                let half_extent = (self.extra_extent / 2) as usize;
                let far_x = self.grid_dimension[0] as usize - half_extent;
                let far_y = self.grid_dimension[1] as usize - half_extent;
                let far_z = self.grid_dimension[2] as usize - half_extent;

                let simulation_x = (self.grid_dimension[0] - self.extra_extent) as usize;
                let simulation_y = (self.grid_dimension[1] - self.extra_extent) as usize;
                let simulation_z = (self.grid_dimension[2] - self.extra_extent) as usize;

                let x_near_plane_electric = ndarray::Array2::from_shape_vec(
                    (simulation_y, simulation_z),
                    ec_map
                        .slice(ndarray::s![
                            half_extent,
                            half_extent..far_y,
                            half_extent..far_z,
                        ])
                        .iter()
                        .cloned()
                        .collect(),
                )
                .unwrap();
                ec_map
                    .slice_mut(ndarray::s![
                        0..half_extent,
                        half_extent..far_y,
                        half_extent..far_z,
                    ])
                    .assign(&x_near_plane_electric);

                let x_far_plane_electric = ndarray::Array2::from_shape_vec(
                    (simulation_y, simulation_z),
                    ec_map
                        .slice(ndarray::s![
                            far_x - 1,
                            half_extent..far_y,
                            half_extent..far_z,
                        ])
                        .iter()
                        .cloned()
                        .collect(),
                )
                .unwrap();
                ec_map
                    .slice_mut(ndarray::s![
                        far_x..self.grid_dimension[0] as usize,
                        half_extent..far_y,
                        half_extent..far_z,
                    ])
                    .assign(&x_far_plane_electric);

                let y_near_plane_electric = ndarray::Array2::from_shape_vec(
                    (simulation_x, simulation_z),
                    ec_map
                        .slice(ndarray::s![
                            half_extent..far_x,
                            half_extent,
                            half_extent..far_z,
                        ])
                        .iter()
                        .cloned()
                        .collect(),
                )
                .unwrap();
                ec_map
                    .slice_mut(ndarray::s![
                        half_extent..far_x,
                        0..half_extent,
                        half_extent..far_z,
                    ])
                    .permuted_axes([1, 0, 2])
                    .assign(&y_near_plane_electric);

                let y_far_plane_electric = ndarray::Array2::from_shape_vec(
                    (simulation_x, simulation_z),
                    ec_map
                        .slice(ndarray::s![
                            half_extent..far_x,
                            far_y - 1,
                            half_extent..far_z,
                        ])
                        .iter()
                        .cloned()
                        .collect(),
                )
                .unwrap();
                ec_map
                    .slice_mut(ndarray::s![
                        half_extent..far_x,
                        far_y..self.grid_dimension[1] as usize,
                        half_extent..far_z,
                    ])
                    .permuted_axes([1, 0, 2])
                    .assign(&y_far_plane_electric);

                let mut z_near_plane_electric =
                    ndarray::Array2::default((simulation_x, simulation_y).f());
                z_near_plane_electric.assign(&ec_map.slice(ndarray::s![
                    half_extent..far_x,
                    half_extent..far_y,
                    half_extent,
                ]));
                ec_map
                    .slice_mut(ndarray::s![
                        half_extent..far_x,
                        half_extent..far_y,
                        0..half_extent,
                    ])
                    .permuted_axes([2, 0, 1])
                    .assign(&z_near_plane_electric);

                let mut z_far_plane_electric =
                    ndarray::Array2::default((simulation_x, simulation_y).f());
                z_far_plane_electric.assign(&ec_map.slice(ndarray::s![
                    half_extent..far_x,
                    half_extent..far_y,
                    far_z - 1,
                ]));
                ec_map
                    .slice_mut(ndarray::s![
                        half_extent..far_x,
                        half_extent..far_y,
                        far_z..self.grid_dimension[2] as usize,
                    ])
                    .permuted_axes([2, 0, 1])
                    .assign(&z_far_plane_electric);

                let pml_electric_views = [
                    &x_near_plane_electric,
                    &x_far_plane_electric,
                    &y_near_plane_electric,
                    &y_far_plane_electric,
                    &z_near_plane_electric,
                    &z_far_plane_electric,
                ]
                .map(|p| {
                    ndarray::Zip::from(p)
                        .par_map_collect(|c| (-(self.pml_sigma + self.pml_alpha) * c.y).exp())
                })
                .map(|c| {
                    device
                        .create_texture_with_data(
                            queue,
                            &wgpu::TextureDescriptor {
                                label: None,
                                size: wgpu::Extent3d {
                                    width: c.dim().0 as _,
                                    height: c.dim().1 as _,
                                    depth_or_array_layers: 1,
                                },
                                mip_level_count: 1,
                                sample_count: 1,
                                dimension: wgpu::TextureDimension::D2,
                                format: wgpu::TextureFormat::R32Float,
                                usage: wgpu::TextureUsages::STORAGE_BINDING,
                                view_formats: &[],
                            },
                            bytemuck::cast_slice(c.as_slice_memory_order().unwrap()),
                        )
                        .create_view(&wgpu::TextureViewDescriptor::default())
                });

                let x_near_plane_magnetic = ndarray::Array2::from_shape_vec(
                    (simulation_y, simulation_z),
                    hc_map
                        .slice(ndarray::s![
                            half_extent,
                            half_extent..far_y,
                            half_extent..far_z,
                        ])
                        .iter()
                        .cloned()
                        .collect(),
                )
                .unwrap();
                hc_map
                    .slice_mut(ndarray::s![
                        0..half_extent,
                        half_extent..far_y,
                        half_extent..far_z,
                    ])
                    .assign(&x_near_plane_magnetic);

                let x_far_plane_magnetic = ndarray::Array2::from_shape_vec(
                    (simulation_y, simulation_z),
                    hc_map
                        .slice(ndarray::s![
                            far_x - 1,
                            half_extent..far_y,
                            half_extent..far_z,
                        ])
                        .iter()
                        .cloned()
                        .collect(),
                )
                .unwrap();
                hc_map
                    .slice_mut(ndarray::s![
                        far_x..self.grid_dimension[0] as usize,
                        half_extent..far_y,
                        half_extent..far_z,
                    ])
                    .assign(&x_far_plane_magnetic);

                let y_near_plane_magnetic = ndarray::Array2::from_shape_vec(
                    (simulation_x, simulation_z),
                    hc_map
                        .slice(ndarray::s![
                            half_extent..far_x,
                            half_extent,
                            half_extent..far_z,
                        ])
                        .iter()
                        .cloned()
                        .collect(),
                )
                .unwrap();
                hc_map
                    .slice_mut(ndarray::s![
                        half_extent..far_x,
                        0..half_extent,
                        half_extent..far_z,
                    ])
                    .permuted_axes([1, 0, 2])
                    .assign(&y_near_plane_magnetic);

                let y_far_plane_magnetic = ndarray::Array2::from_shape_vec(
                    (simulation_x, simulation_z),
                    hc_map
                        .slice(ndarray::s![
                            half_extent..far_x,
                            far_y - 1,
                            half_extent..far_z,
                        ])
                        .iter()
                        .cloned()
                        .collect(),
                )
                .unwrap();
                hc_map
                    .slice_mut(ndarray::s![
                        half_extent..far_x,
                        far_y..self.grid_dimension[1] as usize,
                        half_extent..far_z,
                    ])
                    .permuted_axes([1, 0, 2])
                    .assign(&y_far_plane_magnetic);

                let mut z_near_plane_magnetic =
                    ndarray::Array2::default((simulation_x, simulation_y).f());
                z_near_plane_magnetic.assign(&hc_map.slice(ndarray::s![
                    half_extent..far_x,
                    half_extent..far_y,
                    half_extent,
                ]));
                hc_map
                    .slice_mut(ndarray::s![
                        half_extent..far_x,
                        half_extent..far_y,
                        0..half_extent,
                    ])
                    .permuted_axes([2, 0, 1])
                    .assign(&z_near_plane_magnetic);

                let mut z_far_plane_magnetic =
                    ndarray::Array2::default((simulation_x, simulation_y).f());
                z_far_plane_magnetic.assign(&hc_map.slice(ndarray::s![
                    half_extent..far_x,
                    half_extent..far_y,
                    far_z - 1,
                ]));
                hc_map
                    .slice_mut(ndarray::s![
                        half_extent..far_x,
                        half_extent..far_y,
                        far_z..self.grid_dimension[2] as usize,
                    ])
                    .permuted_axes([2, 0, 1])
                    .assign(&z_far_plane_magnetic);

                let pml_magnetic_views = [
                    (x_near_plane_magnetic, x_near_plane_electric),
                    (x_far_plane_magnetic, x_far_plane_electric),
                    (y_near_plane_magnetic, y_near_plane_electric),
                    (y_far_plane_magnetic, y_far_plane_electric),
                    (z_near_plane_magnetic, z_near_plane_electric),
                    (z_far_plane_magnetic, z_far_plane_electric),
                ]
                .map(|(h, e)| {
                    ndarray::Zip::from(&h).and(&e).par_map_collect(|h, e| {
                        (-(self.pml_sigma + self.pml_alpha) * e.y / h.y * self.dt).exp()
                    })
                })
                .map(|c| {
                    device
                        .create_texture_with_data(
                            queue,
                            &wgpu::TextureDescriptor {
                                label: None,
                                size: wgpu::Extent3d {
                                    width: c.dim().0 as _,
                                    height: c.dim().1 as _,
                                    depth_or_array_layers: 1,
                                },
                                mip_level_count: 1,
                                sample_count: 1,
                                dimension: wgpu::TextureDimension::D2,
                                format: wgpu::TextureFormat::R32Float,
                                usage: wgpu::TextureUsages::STORAGE_BINDING,
                                view_formats: &[],
                            },
                            bytemuck::cast_slice(c.as_slice_memory_order().unwrap()),
                        )
                        .create_view(&wgpu::TextureViewDescriptor::default())
                });

                pml_constants = Some((pml_electric_views, pml_magnetic_views));
            }

            let electric_constants_map = device
                .create_texture_with_data(
                    queue,
                    &common_desc,
                    bytemuck::cast_slice(ec_map.as_slice_memory_order().unwrap()),
                )
                .create_view(&wgpu::TextureViewDescriptor::default());

            let magnetic_constants_map = device
                .create_texture_with_data(
                    queue,
                    &common_desc,
                    bytemuck::cast_slice(hc_map.as_slice_memory_order().unwrap()),
                )
                .create_view(&wgpu::TextureViewDescriptor::default());

            (
                electric_constants_map,
                magnetic_constants_map,
                pml_constants,
            )
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

                    let simulation_x = self.grid_dimension[0] - self.extra_extent;
                    let simulation_y = self.grid_dimension[1] - self.extra_extent;
                    let simulation_z = self.grid_dimension[2] - self.extra_extent;

                    let flag_map: ndarray::Array3<std::sync::Mutex<u8>> =
                        ndarray::Array3::default((
                            simulation_x as usize,
                            simulation_y as usize,
                            simulation_z as usize,
                        ));

                    let half_extent = self.extra_extent / 2;
                    indices.chunks(3).par_bridge().for_each(|triangle| {
                        let v0 = vertices[triangle[0] as usize];
                        let v1 = vertices[triangle[1] as usize];
                        let v2 = vertices[triangle[2] as usize];
                        let edge1 = v1 - v0;
                        let edge2 = v2 - v0;
                        let ray = nalgebra::vector![0.0f32, 0.0, 1.0];
                        let min_x = v0.x.min(v1.x.min(v2.x)).floor().max(0.) as u32;
                        let max_x = v0.x.max(v1.x.max(v2.x)).ceil().max(0.) as u32;
                        let min_y = v0.y.min(v1.y.min(v2.y)).floor().max(0.) as u32;
                        let max_y = v0.y.max(v1.y.max(v2.y)).ceil().max(0.) as u32;
                        (min_x..=max_x).into_par_iter().for_each(|x| {
                            if x < half_extent || x >= self.grid_dimension[0] - half_extent {
                                return;
                            }
                            (min_y..=max_y).into_par_iter().for_each(|y| {
                                if y < half_extent || y >= self.grid_dimension[1] - half_extent {
                                    return;
                                }
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
                                    if u >= 0.0 && v >= 0.0 && u + v <= 1.0 {
                                        let h = p + ray * t;
                                        let x = h.x.round() as u32 - half_extent;
                                        let y = h.y.round() as u32 - half_extent;
                                        let z = (h.z.max(0.).round() as u32).max(half_extent)
                                            - half_extent;

                                        if z < simulation_z - 1 {
                                            let x = x as usize;
                                            let y = y as usize;
                                            let z = z as usize;
                                            *flag_map[[x, y, z]].lock().unwrap() = 1;
                                        }
                                    }
                                }
                            })
                        });
                    });

                    let accumulator: ndarray::Array3<std::sync::Mutex<u8>> =
                        ndarray::Array3::default((
                            simulation_x as usize,
                            simulation_y as usize,
                            simulation_z as usize,
                        ));

                    (0..simulation_z).for_each(|z| {
                        (0..simulation_x).into_par_iter().for_each(|x| {
                            (0..simulation_y).into_par_iter().for_each(|y| {
                                let idx_x = x as usize;
                                let idx_y = y as usize;
                                let idx_z = z as usize;

                                let grid_x = (x + half_extent) as usize;
                                let grid_y = (y + half_extent) as usize;
                                let grid_z = (z + half_extent) as usize;
                                let mut acc_write =
                                    accumulator[[idx_x, idx_y, idx_z]].lock().unwrap();
                                *acc_write = *flag_map[[idx_x, idx_y, idx_z]].lock().unwrap();
                                if z > 0 {
                                    *acc_write +=
                                        *accumulator[[idx_x, idx_y, idx_z - 1]].lock().unwrap();
                                }
                                if *acc_write % 2 == 1 {
                                    *self.electric_constants[[grid_x, grid_y, grid_z]]
                                        .lock()
                                        .unwrap() = nalgebra::vector![constants.ec2, constants.ec3];
                                    *self.magnetic_constants[[grid_x, grid_y, grid_z]]
                                        .lock()
                                        .unwrap() = nalgebra::vector![constants.hc2, constants.hc3];
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
