pub struct PMLCorner {
    pub(crate) psi_self_update_bind_group: wgpu::BindGroup,
    pub(crate) psi_field_update_bind_group: wgpu::BindGroup,
}

impl PMLCorner {
    pub fn new(
        device: &wgpu::Device,
        cells: u32,
        field_view: &[wgpu::TextureView; 3],
        constant_map: &wgpu::TextureView,
        psi_self_update_bind_group_layout: &wgpu::BindGroupLayout,
        psi_field_update_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let common_texture_descriptor = wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: cells,
                height: cells,
                depth_or_array_layers: cells,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D3,
            format: wgpu::TextureFormat::R32Float,
            usage: wgpu::TextureUsages::STORAGE_BINDING,
        };
        let psi_textures = [
            device.create_texture(&common_texture_descriptor), // Ex/Hx - y
            device.create_texture(&common_texture_descriptor), // Ex/Hx - z
            device.create_texture(&common_texture_descriptor), // Ey/Hy - x
            device.create_texture(&common_texture_descriptor), // Ey/Hy - z
            device.create_texture(&common_texture_descriptor), // Ez/Hz - x
            device.create_texture(&common_texture_descriptor), // Ez/Hz - y
        ];
        let psi_texture_views = [
            psi_textures[0].create_view(&wgpu::TextureViewDescriptor::default()),
            psi_textures[1].create_view(&wgpu::TextureViewDescriptor::default()),
            psi_textures[2].create_view(&wgpu::TextureViewDescriptor::default()),
            psi_textures[3].create_view(&wgpu::TextureViewDescriptor::default()),
            psi_textures[4].create_view(&wgpu::TextureViewDescriptor::default()),
            psi_textures[5].create_view(&wgpu::TextureViewDescriptor::default()),
        ];

        let psi_self_update_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: psi_self_update_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&psi_texture_views[0]),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&psi_texture_views[1]),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&psi_texture_views[2]),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&psi_texture_views[3]),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&psi_texture_views[4]),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(&psi_texture_views[5]),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(&field_view[0]),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::TextureView(&field_view[1]),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: wgpu::BindingResource::TextureView(&field_view[2]),
                },
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: wgpu::BindingResource::TextureView(constant_map),
                },
            ],
        });

        let psi_field_update_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &psi_field_update_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&psi_texture_views[0]),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&psi_texture_views[1]),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&psi_texture_views[2]),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&psi_texture_views[3]),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&psi_texture_views[4]),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(&psi_texture_views[5]),
                },
            ],
        });
        Self {
            psi_self_update_bind_group,
            psi_field_update_bind_group,
        }
    }
}

pub struct PMLSurfaceX {
    pub(crate) psi_self_update_bind_group: wgpu::BindGroup,
    pub(crate) psi_field_update_bind_group: wgpu::BindGroup,
}

impl PMLSurfaceX {
    pub fn new(
        device: &wgpu::Device,
        cells: u32,
        simulation_dimension: [u32; 3],
        field_view: &[wgpu::TextureView; 3],
        constant_map: &wgpu::TextureView,
        psi_self_update_bind_group_layout: &wgpu::BindGroupLayout,
        psi_field_update_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let common_texture_descriptor = wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: cells,
                height: simulation_dimension[1],
                depth_or_array_layers: simulation_dimension[2],
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D3,
            format: wgpu::TextureFormat::R32Float,
            usage: wgpu::TextureUsages::STORAGE_BINDING,
        };
        let psi_textures = [
            device.create_texture(&common_texture_descriptor), // Ex/Hx - y
            device.create_texture(&common_texture_descriptor), // Ex/Hx - z
        ];
        let psi_texture_views = [
            psi_textures[0].create_view(&wgpu::TextureViewDescriptor::default()),
            psi_textures[1].create_view(&wgpu::TextureViewDescriptor::default()),
        ];

        let psi_self_update_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: psi_self_update_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&psi_texture_views[0]),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&psi_texture_views[1]),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&field_view[1]),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&field_view[2]),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(constant_map),
                },
            ],
        });

        let psi_field_update_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &psi_field_update_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&psi_texture_views[0]),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&psi_texture_views[1]),
                },
            ],
        });
        Self {
            psi_self_update_bind_group,
            psi_field_update_bind_group,
        }
    }
}

pub struct PMLBoundary {
    cells: u32,
    grid_dimension: [u32; 3],
    simulation_dimension: [u32; 3],
    corner_magnetic: [PMLCorner; 8],
    corner_electric: [PMLCorner; 8],
    corner_self_update_pipeline_magnetic: wgpu::ComputePipeline,
    corner_self_update_pipeline_electric: wgpu::ComputePipeline,
    corner_field_update_pipeline_magnetic: wgpu::ComputePipeline,
    corner_field_update_pipeline_electric: wgpu::ComputePipeline,
    surface_x_magnetic: [PMLSurfaceX; 2],
    surface_x_electric: [PMLSurfaceX; 2],
    surface_x_self_update_pipeline_magnetic: wgpu::ComputePipeline,
    surface_x_self_update_pipeline_electric: wgpu::ComputePipeline,
    surface_x_field_update_pipeline_magnetic: wgpu::ComputePipeline,
    surface_x_field_update_pipeline_electric: wgpu::ComputePipeline,
}

impl PMLBoundary {
    pub fn new(
        device: &wgpu::Device,
        cells: u32,
        electric_field_view: &[wgpu::TextureView; 3],
        magnetic_field_view: &[wgpu::TextureView; 3],
        electric_constant_map: &wgpu::TextureView,
        magnetic_constant_map: &wgpu::TextureView,
        field_update_bind_group_layout: &wgpu::BindGroupLayout,
        grid_dimension: [u32; 3],
        simulation_dimension: [u32; 3],
    ) -> Self {
        let psi_corner_self_update_bind_group_layout =
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
                            access: wgpu::StorageTextureAccess::ReadWrite,
                            format: wgpu::TextureFormat::R32Float,
                            view_dimension: wgpu::TextureViewDimension::D3,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 4,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::StorageTexture {
                            access: wgpu::StorageTextureAccess::ReadWrite,
                            format: wgpu::TextureFormat::R32Float,
                            view_dimension: wgpu::TextureViewDimension::D3,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 5,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::StorageTexture {
                            access: wgpu::StorageTextureAccess::ReadWrite,
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
                            format: wgpu::TextureFormat::R32Float,
                            view_dimension: wgpu::TextureViewDimension::D3,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 7,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::StorageTexture {
                            access: wgpu::StorageTextureAccess::ReadOnly,
                            format: wgpu::TextureFormat::R32Float,
                            view_dimension: wgpu::TextureViewDimension::D3,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 8,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::StorageTexture {
                            access: wgpu::StorageTextureAccess::ReadOnly,
                            format: wgpu::TextureFormat::R32Float,
                            view_dimension: wgpu::TextureViewDimension::D3,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 9,
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

        let psi_corner_field_update_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: None,
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::StorageTexture {
                            access: wgpu::StorageTextureAccess::ReadOnly,
                            format: wgpu::TextureFormat::R32Float,
                            view_dimension: wgpu::TextureViewDimension::D3,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::StorageTexture {
                            access: wgpu::StorageTextureAccess::ReadOnly,
                            format: wgpu::TextureFormat::R32Float,
                            view_dimension: wgpu::TextureViewDimension::D3,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::StorageTexture {
                            access: wgpu::StorageTextureAccess::ReadOnly,
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
                ],
            });
        let corner_electric = [(); 8].map(|_| {
            PMLCorner::new(
                device,
                cells,
                magnetic_field_view,
                electric_constant_map,
                &psi_corner_self_update_bind_group_layout,
                &psi_corner_field_update_bind_group_layout,
            )
        });

        let corner_magnetic = [(); 8].map(|_| {
            PMLCorner::new(
                device,
                cells,
                electric_field_view,
                magnetic_constant_map,
                &psi_corner_self_update_bind_group_layout,
                &psi_corner_field_update_bind_group_layout,
            )
        });

        let corner_self_update_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[&psi_corner_self_update_bind_group_layout],
                push_constant_ranges: &[wgpu::PushConstantRange {
                    stages: wgpu::ShaderStages::COMPUTE,
                    range: 0..48,
                }],
            });

        let corner_self_update_shader_module = device
            .create_shader_module(wgpu::include_wgsl!("../../shader/fdtd/pml_corner_psi.wgsl"));

        let corner_self_update_pipeline_magnetic =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: None,
                layout: Some(&corner_self_update_pipeline_layout),
                module: &corner_self_update_shader_module,
                entry_point: "update_magnetic_psi_corner",
            });

        let corner_self_update_pipeline_electric =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: None,
                layout: Some(&corner_self_update_pipeline_layout),
                module: &corner_self_update_shader_module,
                entry_point: "update_electric_psi_corner",
            });

        let corner_field_update_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[
                    field_update_bind_group_layout,
                    &psi_corner_field_update_bind_group_layout,
                ],
                push_constant_ranges: &[wgpu::PushConstantRange {
                    stages: wgpu::ShaderStages::COMPUTE,
                    range: 0..44,
                }],
            });
        let corner_field_update_shader_module = device.create_shader_module(wgpu::include_wgsl!(
            "../../shader/fdtd/pml_corner_field.wgsl"
        ));

        let corner_field_update_pipeline_magnetic =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: None,
                layout: Some(&corner_field_update_pipeline_layout),
                module: &corner_field_update_shader_module,
                entry_point: "update_magnetic_field",
            });

        let corner_field_update_pipeline_electric =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: None,
                layout: Some(&corner_field_update_pipeline_layout),
                module: &corner_field_update_shader_module,
                entry_point: "update_electric_field",
            });

        // ------------- PML SURFACE X ----------------

        let psi_surface_self_update_bind_group_layout =
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
                            access: wgpu::StorageTextureAccess::ReadOnly,
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
                            format: wgpu::TextureFormat::Rg32Float,
                            view_dimension: wgpu::TextureViewDimension::D3,
                        },
                        count: None,
                    },
                ],
            });

        let psi_surface_field_update_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: None,
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::StorageTexture {
                            access: wgpu::StorageTextureAccess::ReadOnly,
                            format: wgpu::TextureFormat::R32Float,
                            view_dimension: wgpu::TextureViewDimension::D3,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::StorageTexture {
                            access: wgpu::StorageTextureAccess::ReadOnly,
                            format: wgpu::TextureFormat::R32Float,
                            view_dimension: wgpu::TextureViewDimension::D3,
                        },
                        count: None,
                    },
                ],
            });
        let surface_x_electric = [(); 2].map(|_| {
            PMLSurfaceX::new(
                device,
                cells,
                simulation_dimension,
                magnetic_field_view,
                electric_constant_map,
                &psi_surface_self_update_bind_group_layout,
                &psi_surface_field_update_bind_group_layout,
            )
        });

        let surface_x_magnetic = [(); 2].map(|_| {
            PMLSurfaceX::new(
                device,
                cells,
                simulation_dimension,
                electric_field_view,
                magnetic_constant_map,
                &psi_surface_self_update_bind_group_layout,
                &psi_surface_field_update_bind_group_layout,
            )
        });

        let surface_self_update_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[&psi_surface_self_update_bind_group_layout],
                push_constant_ranges: &[wgpu::PushConstantRange {
                    stages: wgpu::ShaderStages::COMPUTE,
                    range: 0..48,
                }],
            });

        let surface_x_self_update_shader_module = device.create_shader_module(wgpu::include_wgsl!(
            "../../shader/fdtd/pml_surface_x_psi.wgsl"
        ));

        let surface_x_self_update_pipeline_magnetic =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: None,
                layout: Some(&surface_self_update_pipeline_layout),
                module: &surface_x_self_update_shader_module,
                entry_point: "update_magnetic_psi_surface",
            });

        let surface_x_self_update_pipeline_electric =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: None,
                layout: Some(&surface_self_update_pipeline_layout),
                module: &surface_x_self_update_shader_module,
                entry_point: "update_electric_psi_surface",
            });

        let surface_field_update_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[
                    field_update_bind_group_layout,
                    &psi_surface_field_update_bind_group_layout,
                ],
                push_constant_ranges: &[wgpu::PushConstantRange {
                    stages: wgpu::ShaderStages::COMPUTE,
                    range: 0..44,
                }],
            });
        let surface_x_field_update_shader_module = device.create_shader_module(
            wgpu::include_wgsl!("../../shader/fdtd/pml_surface_x_field.wgsl"),
        );

        let surface_x_field_update_pipeline_magnetic =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: None,
                layout: Some(&surface_field_update_pipeline_layout),
                module: &surface_x_field_update_shader_module,
                entry_point: "update_magnetic_field",
            });

        let surface_x_field_update_pipeline_electric =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: None,
                layout: Some(&surface_field_update_pipeline_layout),
                module: &surface_x_field_update_shader_module,
                entry_point: "update_electric_field",
            });

        Self {
            corner_self_update_pipeline_magnetic,
            corner_self_update_pipeline_electric,
            corner_field_update_pipeline_magnetic,
            corner_field_update_pipeline_electric,
            cells,
            corner_magnetic,
            corner_electric,
            grid_dimension,
            simulation_dimension,
            surface_x_magnetic,
            surface_x_electric,
            surface_x_self_update_pipeline_magnetic,
            surface_x_self_update_pipeline_electric,
            surface_x_field_update_pipeline_magnetic,
            surface_x_field_update_pipeline_electric,
        }
    }

    pub fn update_electric_field<'a>(
        &'a self,
        cpass: &mut wgpu::ComputePass<'a>,
        field_update_bind_group: &'a wgpu::BindGroup,
        dt: f32,
        sigma: f32,
    ) {
        let b = (-sigma * dt).exp();
        self.corner_electric
            .iter()
            .enumerate()
            .for_each(|(idx, corner)| {
                cpass.set_pipeline(&self.corner_self_update_pipeline_electric);
                cpass.set_bind_group(0, &corner.psi_self_update_bind_group, &[]);
                cpass.set_push_constants(0, bytemuck::cast_slice(&self.grid_dimension));
                cpass.set_push_constants(16, bytemuck::cast_slice(&[self.cells; 3]));
                let offset: [u32; 3] = match idx {
                    0 => [0; 3],
                    1 => [self.cells + self.simulation_dimension[0], 0, 0],
                    2 => [
                        self.cells + self.simulation_dimension[0],
                        self.cells + self.simulation_dimension[1],
                        0,
                    ],
                    3 => [0, self.cells + self.simulation_dimension[1], 0],
                    4 => [0, 0, self.cells + self.simulation_dimension[2]],
                    5 => [
                        self.cells + self.simulation_dimension[0],
                        0,
                        self.cells + self.simulation_dimension[2],
                    ],
                    6 => [
                        self.cells + self.simulation_dimension[0],
                        self.cells + self.simulation_dimension[1],
                        self.cells + self.simulation_dimension[2],
                    ],
                    7 => [
                        0,
                        self.cells + self.simulation_dimension[1],
                        self.cells + self.simulation_dimension[2],
                    ],
                    _ => unreachable!(),
                };
                cpass.set_push_constants(32, bytemuck::cast_slice(&offset));
                cpass.set_push_constants(44, bytemuck::cast_slice(&[b]));
                cpass.dispatch_workgroups(
                    (self.cells as f32 / 8.0).ceil() as u32,
                    (self.cells as f32 / 8.0).ceil() as u32,
                    (self.cells as f32 / 8.0).ceil() as u32,
                );
                cpass.set_pipeline(&self.corner_field_update_pipeline_electric);
                cpass.set_bind_group(0, field_update_bind_group, &[]);
                cpass.set_bind_group(1, &corner.psi_field_update_bind_group, &[]);
                cpass.set_push_constants(0, bytemuck::cast_slice(&self.grid_dimension));
                cpass.set_push_constants(16, bytemuck::cast_slice(&[self.cells; 3]));
                cpass.set_push_constants(32, bytemuck::cast_slice(&offset));
                cpass.dispatch_workgroups(
                    (self.cells as f32 / 8.0).ceil() as u32,
                    (self.cells as f32 / 8.0).ceil() as u32,
                    (self.cells as f32 / 8.0).ceil() as u32,
                );
            });

        self.surface_x_electric
            .iter()
            .enumerate()
            .for_each(|(idx, surface)| {
                cpass.set_pipeline(&self.surface_x_self_update_pipeline_electric);
                cpass.set_bind_group(0, &surface.psi_self_update_bind_group, &[]);
                cpass.set_push_constants(0, bytemuck::cast_slice(&self.grid_dimension));
                cpass.set_push_constants(
                    16,
                    bytemuck::cast_slice(&[
                        self.cells,
                        self.simulation_dimension[1],
                        self.simulation_dimension[2],
                    ]),
                );
                let offset: [u32; 3] = match idx {
                    0 => [0, self.cells, self.cells],
                    1 => [
                        self.cells + self.simulation_dimension[0],
                        self.cells,
                        self.cells,
                    ],
                    _ => unreachable!(),
                };
                cpass.set_push_constants(32, bytemuck::cast_slice(&offset));
                cpass.set_push_constants(44, bytemuck::cast_slice(&[b]));
                cpass.dispatch_workgroups(
                    (self.cells as f32 / 8.0).ceil() as u32,
                    (self.simulation_dimension[1] as f32 / 8.0).ceil() as u32,
                    (self.simulation_dimension[2] as f32 / 8.0).ceil() as u32,
                );
                cpass.set_pipeline(&self.surface_x_field_update_pipeline_electric);
                cpass.set_bind_group(0, field_update_bind_group, &[]);
                cpass.set_bind_group(1, &surface.psi_field_update_bind_group, &[]);
                cpass.set_push_constants(0, bytemuck::cast_slice(&self.grid_dimension));
                cpass.set_push_constants(
                    16,
                    bytemuck::cast_slice(&[
                        self.cells,
                        self.simulation_dimension[1],
                        self.simulation_dimension[2],
                    ]),
                );
                cpass.set_push_constants(32, bytemuck::cast_slice(&offset));
                cpass.dispatch_workgroups(
                    (self.cells as f32 / 8.0).ceil() as u32,
                    (self.simulation_dimension[1] as f32 / 8.0).ceil() as u32,
                    (self.simulation_dimension[2] as f32 / 8.0).ceil() as u32,
                );
            });
    }

    pub fn update_magnetic_field<'a>(
        &'a self,
        cpass: &mut wgpu::ComputePass<'a>,
        field_update_bind_group: &'a wgpu::BindGroup,
        dt: f32,
        sigma: f32,
    ) {
        let b = (-sigma * dt).exp();
        self.corner_magnetic
            .iter()
            .enumerate()
            .for_each(|(idx, corner)| {
                cpass.set_pipeline(&self.corner_self_update_pipeline_magnetic);
                cpass.set_bind_group(0, &corner.psi_self_update_bind_group, &[]);
                cpass.set_push_constants(0, bytemuck::cast_slice(&self.grid_dimension));
                cpass.set_push_constants(16, bytemuck::cast_slice(&[self.cells; 3]));
                let offset: [u32; 3] = match idx {
                    0 => [0; 3],
                    1 => [self.cells + self.simulation_dimension[0], 0, 0],
                    2 => [
                        self.cells + self.simulation_dimension[0],
                        self.cells + self.simulation_dimension[1],
                        0,
                    ],
                    3 => [0, self.cells + self.simulation_dimension[1], 0],
                    4 => [0, 0, self.cells + self.simulation_dimension[2]],
                    5 => [
                        self.cells + self.simulation_dimension[0],
                        0,
                        self.cells + self.simulation_dimension[2],
                    ],
                    6 => [
                        self.cells + self.simulation_dimension[0],
                        self.cells + self.simulation_dimension[1],
                        self.cells + self.simulation_dimension[2],
                    ],
                    7 => [
                        0,
                        self.cells + self.simulation_dimension[1],
                        self.cells + self.simulation_dimension[2],
                    ],
                    _ => unreachable!(),
                };
                cpass.set_push_constants(32, bytemuck::cast_slice(&offset));
                cpass.set_push_constants(44, bytemuck::cast_slice(&[b]));
                cpass.dispatch_workgroups(
                    (self.cells as f32 / 8.0).ceil() as u32,
                    (self.cells as f32 / 8.0).ceil() as u32,
                    (self.cells as f32 / 8.0).ceil() as u32,
                );
                cpass.set_pipeline(&self.corner_field_update_pipeline_magnetic);
                cpass.set_bind_group(0, field_update_bind_group, &[]);
                cpass.set_bind_group(1, &corner.psi_field_update_bind_group, &[]);
                cpass.set_push_constants(0, bytemuck::cast_slice(&self.grid_dimension));
                cpass.set_push_constants(16, bytemuck::cast_slice(&[self.cells; 3]));
                cpass.set_push_constants(32, bytemuck::cast_slice(&offset));
                cpass.dispatch_workgroups(
                    (self.cells as f32 / 8.0).ceil() as u32,
                    (self.cells as f32 / 8.0).ceil() as u32,
                    (self.cells as f32 / 8.0).ceil() as u32,
                );
            });
        self.surface_x_magnetic
            .iter()
            .enumerate()
            .for_each(|(idx, surface)| {
                cpass.set_pipeline(&self.surface_x_self_update_pipeline_magnetic);
                cpass.set_bind_group(0, &surface.psi_self_update_bind_group, &[]);
                cpass.set_push_constants(0, bytemuck::cast_slice(&self.grid_dimension));
                cpass.set_push_constants(
                    16,
                    bytemuck::cast_slice(&[
                        self.cells,
                        self.simulation_dimension[1],
                        self.simulation_dimension[2],
                    ]),
                );
                let offset: [u32; 3] = match idx {
                    0 => [0, self.cells, self.cells],
                    1 => [
                        self.cells + self.simulation_dimension[0],
                        self.cells,
                        self.cells,
                    ],
                    _ => unreachable!(),
                };
                cpass.set_push_constants(32, bytemuck::cast_slice(&offset));
                cpass.set_push_constants(44, bytemuck::cast_slice(&[b]));
                cpass.dispatch_workgroups(
                    (self.cells as f32 / 8.0).ceil() as u32,
                    (self.simulation_dimension[1] as f32 / 8.0).ceil() as u32,
                    (self.simulation_dimension[2] as f32 / 8.0).ceil() as u32,
                );
                cpass.set_pipeline(&self.surface_x_field_update_pipeline_magnetic);
                cpass.set_bind_group(0, field_update_bind_group, &[]);
                cpass.set_bind_group(1, &surface.psi_field_update_bind_group, &[]);
                cpass.set_push_constants(0, bytemuck::cast_slice(&self.grid_dimension));
                cpass.set_push_constants(
                    16,
                    bytemuck::cast_slice(&[
                        self.cells,
                        self.simulation_dimension[1],
                        self.simulation_dimension[2],
                    ]),
                );
                cpass.set_push_constants(32, bytemuck::cast_slice(&offset));
                cpass.dispatch_workgroups(
                    (self.cells as f32 / 8.0).ceil() as u32,
                    (self.simulation_dimension[1] as f32 / 8.0).ceil() as u32,
                    (self.simulation_dimension[2] as f32 / 8.0).ceil() as u32,
                );
            });
    }
}
