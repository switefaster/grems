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
    dimension: [f32; 3],
    spatial_step: f32,
    temporal_step: f32,
    gltfs: Vec<GLTFSettings>,
    source: SourceSettings,
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
    delay: f32,
    width: f32,
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
                max_push_constant_size: 32,
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

    let settings = config::Config::builder()
        .add_source(config::File::with_name(&options.preset))
        .build()?;

    let settings: FDTDSettings = settings.try_deserialize()?;

    let fdtd = fdtd::FDTD::new(
        &device,
        &queue,
        settings.spatial_step,
        settings.temporal_step,
        settings.dimension,
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
                let additive_source_strength = (-((step_counter as f32 * settings.temporal_step
                    - settings.source.delay)
                    / settings.source.width)
                    .powi(2))
                .exp();

                let screen_width = (settings.dimension[0] / settings.spatial_step).ceil() as u32;
                let screen_height = (settings.dimension[1] / settings.spatial_step).ceil() as u32;
                queue.write_texture(
                    wgpu::ImageCopyTexture {
                        texture: fdtd.get_electric_additive_source_map(),
                        mip_level: 0,
                        origin: wgpu::Origin3d { x: 0, y: 0, z: 0 },
                        aspect: wgpu::TextureAspect::All,
                    },
                    bytemuck::cast_slice(&vec![
                        [0.0, additive_source_strength, 0.0, 1.0];
                        (screen_width * screen_height) as usize
                    ]),
                    wgpu::ImageDataLayout {
                        offset: 0,
                        bytes_per_row: std::num::NonZeroU32::new(16 * screen_width),
                        rows_per_image: std::num::NonZeroU32::new(screen_height),
                    },
                    wgpu::Extent3d {
                        width: screen_width,
                        height: screen_height,
                        depth_or_array_layers: 1,
                    },
                );
                fdtd.step(&mut encoder);
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

            queue.submit(std::iter::once(encoder.finish()));
            surface_texture.present();
        }
        _ => (),
    });
}
