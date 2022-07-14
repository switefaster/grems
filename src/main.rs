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
    spatial_step: f32,
    temporal_step: f32,
    gltfs: Vec<GLTFSettings>,
    sources: Vec<SourceSettings>,
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct GLTFSettings {
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

    let instance = wgpu::Instance::new(wgpu::Backends::PRIMARY);
    let surface = unsafe { instance.create_surface(&window) };
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface: Some(&surface),
    }))
    .unwrap();
    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: None,
            features: wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES
                | wgpu::Features::PUSH_CONSTANTS,
            limits: wgpu::Limits {
                max_push_constant_size: 60,
                max_compute_invocations_per_workgroup: 512,
                ..Default::default()
            },
        },
        None,
    ))?;

    let mut surface_config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: surface.get_supported_formats(&adapter)[0],
        width: window.inner_size().width,
        height: window.inner_size().height,
        present_mode: wgpu::PresentMode::Fifo,
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

    let settings: FDTDSettings = settings.try_deserialize()?;

    let fdtd = fdtd::FDTD::new(
        &device,
        &queue,
        settings.spatial_step,
        settings.temporal_step * 1e-6 / physical_constants::SPEED_OF_LIGHT_IN_VACUUM as f32,
        settings.domain,
        settings.gltfs,
    )?;

    let mut step_counter = 0;
    let mut now = std::time::Instant::now();
    let tau = std::time::Duration::from_secs_f32(1.0 / 60f32);
    let mut elapsed = std::time::Duration::ZERO;
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
                    }
                }
                _ => (),
            }
        }
        winit::event::Event::MainEventsCleared => window.request_redraw(),
        winit::event::Event::RedrawRequested(_) => {
            let dt = now.elapsed();
            elapsed += dt;
            now = std::time::Instant::now();
            let mut encoder =
                device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
            while elapsed >= tau {
                elapsed -= tau;
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
                        - source.phase.to_radians())
                    .cos();

                    let direction = nalgebra::Vector3::from(source.direction).normalize();
                    let actual_position = [
                        ((source.position[0] - settings.domain[0][0] - source.size[0] / 2.0)
                            / settings.spatial_step)
                            .ceil() as u32,
                        ((source.position[1] - settings.domain[1][0] - source.size[1] / 2.0)
                            / settings.spatial_step)
                            .ceil() as u32,
                        ((source.position[2] - settings.domain[2][0] - source.size[2] / 2.0)
                            / settings.spatial_step)
                            .ceil() as u32,
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
            }

            let surface_texture = surface.get_current_texture().unwrap();
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

            glyph_brush.queue(wgpu_glyph::Section {
                screen_position: (0.0, 0.0),
                bounds: (surface_config.width as f32, surface_config.height as f32),
                text: vec![
                    wgpu_glyph::Text::new(&format!("Time step: {}", step_counter))
                        .with_color([0.0, 0.0, 0.0, 1.0])
                        .with_scale(40.0),
                ],
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
