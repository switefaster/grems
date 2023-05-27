mod fdtd;

#[derive(argh::FromArgs)]
#[argh(description = "Gpu-accelerated Rusty Electro-Magnetic field Simulator options")]
struct GremOptions {
    #[argh(positional)]
    /// calculation preset file
    preset: String,
}

#[derive(serde::Deserialize, serde::Serialize)]
struct FDTDSettings {
    domain: [[f32; 2]; 3],
    boundary: crate::fdtd::BoundaryCondition,
    spatial_step: f32,
    temporal_step: f32,
    steps_per_second_limit: f32,
    default_slice: SliceSettings,
    default_scaling_factor: f32,
    default_shader: String,
    pause_at: Vec<TimingSettings>,
    exports: Vec<ExportSettings>,
    models: Vec<ModelSettings>,
    sources: Vec<SourceSettings>,
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct SliceSettings {
    field: fdtd::FieldType,
    mode: fdtd::SliceMode,
    position: f32,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[serde(tag = "type", content = "value")]
enum TimingSettings {
    Step(u32),
    Time(f32),
}

#[derive(serde::Serialize, serde::Deserialize)]
struct ExportSettings {
    timing: TimingSettings,
    export: ExportFieldSettings,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(tag = "dimension", content = "settings")]
enum ExportFieldSettings {
    D3 { field: fdtd::FieldType },
    D2(SliceSettings),
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct ModelSettings {
    path: String,
    position: [f32; 3],
    scale: [f32; 3],
    refractive_index: f32,
}

#[derive(serde::Deserialize, serde::Serialize)]
struct SourceSettings {
    wavelength: f32,
    position: [f32; 3],
    size: [f32; 3],
    direction: [f32; 3],
    phase: f32,
    delay: f32,
    fwhm: f32,
    power: f32,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    pos: [f32; 2],
    tex_coord: [f32; 2],
}

fn main() -> anyhow::Result<()> {
    let options: GremOptions = argh::from_env();

    let event_loop = winit::event_loop::EventLoop::new();
    let window = winit::window::WindowBuilder::new()
        .with_title("GREMS")
        .build(&event_loop)?;

    let instance = wgpu::Instance::default();
    let surface = unsafe { instance.create_surface(&window)? };
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface: Some(&surface),
    }))
    .unwrap();
    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: None,
            features: adapter.features(),
            limits: adapter.limits(),
        },
        None,
    ))?;

    let caps = surface.get_capabilities(&adapter);

    let mut surface_config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: caps.formats[0],
        width: window.inner_size().width,
        height: window.inner_size().height,
        present_mode: wgpu::PresentMode::Fifo,
        alpha_mode: wgpu::CompositeAlphaMode::Auto,
        view_formats: vec![caps.formats[0]],
    };

    surface.configure(&device, &surface_config);

    let mut staging_belt = wgpu::util::StagingBelt::new(1024);

    let roboto = wgpu_glyph::ab_glyph::FontArc::try_from_slice(include_bytes!(
        "../fonts/Roboto-Regular.ttf"
    ))?;

    let mut glyph_brush =
        wgpu_glyph::GlyphBrushBuilder::using_font(roboto).build(&device, surface_config.format);

    let settings = config::Config::builder()
        .add_source(config::File::with_name(&options.preset))
        .build()?;

    let mut settings: FDTDSettings = settings.try_deserialize()?;

    settings.pause_at.sort_by_key(|v| match v {
        TimingSettings::Step(step) => *step,
        TimingSettings::Time(time) => (time / settings.temporal_step).round() as u32,
    });

    settings.exports.sort_by_key(|v| match v.timing {
        TimingSettings::Step(step) => step,
        TimingSettings::Time(time) => (time / settings.temporal_step).round() as u32,
    });

    let mut fdtd = fdtd::FDTD::new(
        &device,
        &queue,
        settings.spatial_step,
        settings.temporal_step,
        settings.domain,
        settings.models,
        settings.boundary,
        settings.default_slice,
        &settings.default_shader,
        settings.default_scaling_factor,
    )?;

    let mut step_counter = 0;
    let mut now = std::time::Instant::now();
    let tau = std::time::Duration::from_secs_f32(1.0 / settings.steps_per_second_limit);
    let mut elapsed = std::time::Duration::ZERO;
    let mut paused = false;

    let update_threshold = 10u32;
    let mut last_display_step = 0u32;
    let mut last_display_time = std::time::Instant::now();
    let mut fps_counter = 0f32;
    let show_fps_duration = std::time::Duration::from_secs_f32(1f32);

    let mut ctrl_pressed = false;

    event_loop.run(move |event, _, control_flow| match event {
        winit::event::Event::WindowEvent { window_id, event } if window_id == window.id() => {
            match event {
                winit::event::WindowEvent::CloseRequested => {
                    *control_flow = winit::event_loop::ControlFlow::Exit
                }
                winit::event::WindowEvent::Resized(new_size) => {
                    if new_size.width > 0 && new_size.height > 0 {
                        surface_config.width = new_size.width;
                        surface_config.height = new_size.height;
                        surface.configure(&device, &surface_config);
                        window.request_redraw();
                    }
                }
                winit::event::WindowEvent::ScaleFactorChanged {
                    new_inner_size: new_size,
                    ..
                } => {
                    if new_size.width > 0 && new_size.height > 0 {
                        surface_config.width = new_size.width;
                        surface_config.height = new_size.height;
                        surface.configure(&device, &surface_config);
                        window.request_redraw();
                    }
                }
                winit::event::WindowEvent::MouseWheel { delta, .. } => match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, row) => {
                        fdtd.offset_slice_position(row);
                        window.request_redraw();
                    }
                    winit::event::MouseScrollDelta::PixelDelta(_) => unimplemented!(),
                },
                winit::event::WindowEvent::KeyboardInput { input: winit::event::KeyboardInput {
                    state: winit::event::ElementState::Pressed,
                    virtual_keycode: Some(keycode),
                    ..
                }, .. } if ctrl_pressed => match keycode {
                    winit::event::VirtualKeyCode::Space => {
                        paused = !paused;
                    },
                    winit::event::VirtualKeyCode::X => {
                        fdtd.set_slice_mode(fdtd::SliceMode::X);
                        window.request_redraw();
                    },
                    winit::event::VirtualKeyCode::Y => {
                        fdtd.set_slice_mode(fdtd::SliceMode::Y);
                        window.request_redraw();
                    },
                    winit::event::VirtualKeyCode::Z => {
                        fdtd.set_slice_mode(fdtd::SliceMode::Z);
                        window.request_redraw();
                    }
                    winit::event::VirtualKeyCode::E => {
                        fdtd.set_field_view_mode(fdtd::FieldType::E);
                        window.request_redraw();
                    }
                    winit::event::VirtualKeyCode::H => {
                        fdtd.set_field_view_mode(fdtd::FieldType::H);
                        window.request_redraw();
                    }
                    winit::event::VirtualKeyCode::Left => {
                        fdtd.scale_linear(-1.0);
                        window.request_redraw();
                    }
                    winit::event::VirtualKeyCode::Right => {
                        fdtd.scale_linear(1.0);
                        window.request_redraw();
                    }
                    winit::event::VirtualKeyCode::Up => {
                        fdtd.scale_exponential(1);
                        window.request_redraw();
                    }
                    winit::event::VirtualKeyCode::Down => {
                        fdtd.scale_exponential(-1);
                        window.request_redraw();
                    }
                    _ => (),
                }
                winit::event::WindowEvent::ModifiersChanged(modifiers) => {
                    ctrl_pressed = modifiers.ctrl();
                }
                winit::event::WindowEvent::DroppedFile(file) => {
                    fdtd.reload_shader(file, &device).unwrap();
                    window.request_redraw();
                }
                _ => (),
            }
        }
        winit::event::Event::MainEventsCleared => if !paused {
            window.request_redraw();
            *control_flow = winit::event_loop::ControlFlow::Poll;
        } else {
            *control_flow = winit::event_loop::ControlFlow::Wait;
        },
        winit::event::Event::RedrawRequested(_) => {
            let dt = now.elapsed();
            elapsed += dt;
            now = std::time::Instant::now();
            let mut encoder =
                device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());

            let mut n = (elapsed.as_secs_f32() / tau.as_secs_f32()) as u32;
            if paused {
                n = 0;
                elapsed = std::time::Duration::ZERO;
            }
            if n > update_threshold {
                n = update_threshold;
                elapsed = std::time::Duration::ZERO;
            } else if n > 0 {
                elapsed -= tau * n as u32;
            }

            while n > 0 && !paused {
                n -= 1;
                fdtd.update_magnetic_field(&mut encoder);
                fdtd.update_electric_field(&mut encoder);
                for source in settings.sources.iter() {
                    let pulse_envelope = (-((std::f32::consts::PI
                        * source.fwhm
                        * (step_counter as f32 * settings.temporal_step - source.delay))
                        .powi(2)
                        / (4.0 * (2.0 as f32).ln()))
                    .powi(2))
                    .exp();

                    let cw_component = (-2.0
                        * std::f32::consts::PI
                        * (step_counter as f32 * settings.temporal_step - source.delay)
                        / source.wavelength
                        + source.phase.to_radians())
                    .cos();

                    let direction = nalgebra::Vector3::from(source.direction).normalize();
                    let actual_position = [
                        ((source.position[0] - settings.domain[0][0] - source.size[0] / 2.0)
                            / settings.spatial_step)
                            .ceil() as u32 + settings.boundary.get_extra_grid_extent() / 2,
                        ((source.position[1] - settings.domain[1][0] - source.size[1] / 2.0 )
                            / settings.spatial_step)
                            .ceil() as u32 + settings.boundary.get_extra_grid_extent() / 2,
                        ((source.position[2] - settings.domain[2][0] - source.size[2] / 2.0)
                            / settings.spatial_step)
                            .ceil() as u32 + settings.boundary.get_extra_grid_extent() / 2,
                    ];
                    let actual_size = [
                        if source.size[0] > 0.0 {
                            (source.size[0] / settings.spatial_step).ceil() as u32
                        } else {
                            1
                        },
                        if source.size[0] > 0.0 {
                            (source.size[1] / settings.spatial_step).ceil() as u32
                        } else {
                            1
                        },
                        if source.size[0] > 0.0 {
                            (source.size[1] / settings.spatial_step).ceil() as u32
                        } else {
                            1
                        },
                    ];

                    fdtd.excite_electric_field(
                        &mut encoder,
                        actual_position,
                        actual_size,
                        (direction * pulse_envelope * cw_component * source.power).into(),
                    );
                }

                step_counter += 1;

                while let Some(timing) = settings.pause_at.first() {
                    let step = match timing {
                        TimingSettings::Step(step) => *step,
                        TimingSettings::Time(time) => (time / settings.temporal_step).round() as u32,
                    };

                    if step == step_counter {
                        settings.pause_at.remove(0);
                        paused = true;
                    } else {
                        break;
                    }
                }

                while let Some(export) = settings.exports.first() {
                    let step = match export.timing {
                        TimingSettings::Step(step) => step,
                        TimingSettings::Time(time) => (time / settings.temporal_step).round() as u32,
                    };

                    if step == step_counter {
                        let mut export_encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
                        match export.export {
                            ExportFieldSettings::D3 { field } => {
                                let field_texture = match field {
                                    fdtd::FieldType::E => fdtd.get_electric_field_textures()[0].as_image_copy(),
                                    fdtd::FieldType::H => fdtd.get_magnetic_field_textures()[0].as_image_copy(),
                                };

                                let dimension = fdtd.get_dimension();

                                let bytes_per_pixel = 1 * std::mem::size_of::<f32>() as u32;
                                let unpadded_bytes_per_row = dimension[0] * bytes_per_pixel;
                                let padded_bytes_per_row_padding = (wgpu::COPY_BYTES_PER_ROW_ALIGNMENT - unpadded_bytes_per_row % wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) % wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
                                let padded_bytes_per_row = unpadded_bytes_per_row + padded_bytes_per_row_padding;

                                let copy_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                                    label: None,
                                    size: (padded_bytes_per_row * dimension[1] * dimension[2]) as u64,
                                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                                    mapped_at_creation: false,
                                });

                                export_encoder.copy_texture_to_buffer(field_texture,
                                    wgpu::ImageCopyBufferBase {
                                        buffer: &copy_buffer,
                                        layout: wgpu::ImageDataLayout {
                                            offset: 0,
                                            bytes_per_row: Some(padded_bytes_per_row),
                                            rows_per_image: Some(dimension[1])
                                        }
                                    },
                                    wgpu::Extent3d {
                                    width: dimension[0],
                                    height: dimension[1],
                                    depth_or_array_layers: dimension[2],
                                });
                                queue.submit(Some(export_encoder.finish()));

                                let (sender, receiver) = futures_intrusive::channel::shared::oneshot_channel();
                                let map_slice = copy_buffer.slice(..);
                                map_slice.map_async(wgpu::MapMode::Read, move |v| sender.send(v).unwrap());
                                device.poll(wgpu::Maintain::Wait);

                                if let Some(Ok(())) = pollster::block_on(receiver.receive()) {
                                    {
                                        let data = map_slice.get_mapped_range();
                                        let raw_data: Vec<u8> = data.chunks(padded_bytes_per_row as usize)
                                            .flat_map(|row| &row[..unpadded_bytes_per_row as usize])
                                            .cloned()
                                            .collect();

                                        let mut dds = ddsfile::Dds::new_dxgi(ddsfile::NewDxgiParams {
                                            height: dimension[1],
                                            width: dimension[0],
                                            depth: Some(dimension[2]),
                                            format: ddsfile::DxgiFormat::R32_Float,
                                            mipmap_levels: None,
                                            array_layers: None,
                                            caps2: None,
                                            is_cubemap: false,
                                            resource_dimension: ddsfile::D3D10ResourceDimension::Texture3D,
                                            alpha_mode: ddsfile::AlphaMode::Unknown,
                                        }).unwrap();

                                        dds.data = raw_data;

                                        let mut file = std::fs::OpenOptions::new()
                                            .write(true)
                                            .truncate(true)
                                            .create(true)
                                            .open(std::env::current_dir().unwrap().join(
                                                format!("{}-D3-{:?}-{}.dds", options.preset, field ,step_counter)
                                            ))
                                            .unwrap();

                                        dds.write(&mut file).unwrap();
                                    }
                                    copy_buffer.unmap();
                                }

                            },
                            ExportFieldSettings::D2(ref _settings) => eprintln!("2D Slice Not Yet Implemented"),
                        }
                        settings.exports.remove(0);
                    } else {
                        break;
                    }
                }
            }

            let surface_texture = match surface.get_current_texture() {
                Ok(texture) => texture,
                Err(err) => match err {
                    wgpu::SurfaceError::Timeout => {
                        return;
                    }
                    wgpu::SurfaceError::Outdated | wgpu::SurfaceError::Lost => {
                        surface.configure(&device, &surface_config);
                        return;
                    }
                    wgpu::SurfaceError::OutOfMemory => panic!("OUT OF MEMORY!"),
                },
            };
            let surf_texture_view = surface_texture
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());

            {
                let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: None,
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &surf_texture_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: true,
                        },
                    })],
                    depth_stencil_attachment: None,
                });

                fdtd.visualize(&mut render_pass);
            }

            if last_display_time.elapsed() >= show_fps_duration {
                fps_counter = (step_counter - last_display_step) as f32 / last_display_time.elapsed().as_secs_f32();
                last_display_time = std::time::Instant::now();
                last_display_step = step_counter;
            }

            glyph_brush.queue(wgpu_glyph::Section {
                screen_position: (0.0, 0.0),
                bounds: (surface_config.width as f32, surface_config.height as f32),
                text: vec![wgpu_glyph::Text::new(&format!(
                    "Time step: {} (ct = {:.3}), Steps/sec: {:.3}, Slice position: {:?} = {}, Scaling factor: {:.1}, field: {:?}",
                    step_counter,
                    step_counter as f32 * settings.temporal_step,
                    fps_counter,
                    fdtd.get_slice_mode(),
                    fdtd.get_slice_position(),
                    fdtd.get_scaling_factor(),
                    fdtd.get_field_view_mode()
                ))
                .with_color([1.0, 0.0, 0.0, 1.0])
                .with_scale(20.0)],
                ..Default::default()
            });

            glyph_brush
                .draw_queued(
                    &device,
                    &mut staging_belt,
                    &mut encoder,
                    &surf_texture_view,
                    surface_config.width,
                    surface_config.height,
                )
                .unwrap();

            staging_belt.finish();
            queue.submit(std::iter::once(encoder.finish()));
            device.poll(wgpu::Maintain::Wait);
            surface_texture.present();
            staging_belt.recall();
        }
        _ => (),
    });
}
