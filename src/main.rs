use std::path::Path;

use clap::Parser;
use ndarray::ShapeBuilder;
use pollster::FutureExt;
use wgpu::util::DeviceExt;
use wgpu_text::{
    glyph_brush::{Section as TextSection, Text},
    BrushBuilder,
};
use winit::{
    event::{ElementState, KeyEvent},
    keyboard::PhysicalKey,
};
mod fdtd;
mod interpolator;

/// Gpu-accelerated Rusty Electro-Magnetic field Simulator
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct GremOptions {
    #[arg(long)]
    /// Print device infos and quit
    info: bool,
    #[arg(long)]
    /// Disable Visualization <unsupported>
    no_visual: bool,
    #[arg(required_unless_present = "info")]
    /// Simulation preset file
    preset: Option<String>,
}

#[derive(serde::Deserialize, serde::Serialize)]
struct FDTDSettings {
    domain: [[f32; 2]; 3],
    workgroup: Option<WorkgroupSettings>, // this is kind of 'meta', maybe move it to another configs?
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
pub struct WorkgroupSettings {
    x: u32,
    y: u32,
    z: u32,
}

impl WorkgroupSettings {
    pub fn cache_volume(&self) -> u32 {
        self.x * self.y * self.z
    }
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
#[serde(rename_all = "snake_case")]
#[serde(tag = "type", content = "settings")]
enum ModeSettings {
    PointCloud {
        file: String,
        exclude: Vec<(fdtd::FieldType, fdtd::Component)>,
    },
    Texture {
        ex: Option<String>,
        ey: Option<String>,
        ez: Option<String>,
        hx: Option<String>,
        hy: Option<String>,
        hz: Option<String>,
        spatial_step: f32,
    },
    Volume {
        direction: [f32; 3],
        field: fdtd::FieldType,
    },
}

#[derive(serde::Deserialize, serde::Serialize)]
struct SourceSettings {
    wavelength: f32,
    position: [f32; 3],
    size: [f32; 3],
    mode: ModeSettings,
    phase: f32,
    delay: f32,
    fwhm: f32,
    power: f32,
}

enum Source {
    Texture {
        source_bind_group: wgpu::BindGroup,
        z_layer: u32,
        wavelength: f32,
        delay: f32,
        fwhm: f32,
    },
    Volume {
        direction: [f32; 3],
        wavelength: f32,
        position: [f32; 3],
        size: [f32; 3],
        phase: f32,
        delay: f32,
        fwhm: f32,
        power: f32,
    },
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    pos: [f32; 2],
    tex_coord: [f32; 2],
}

struct RG32;

impl resize::PixelFormat for RG32 {
    type InputPixel = nalgebra::Vector2<f32>;

    type OutputPixel = nalgebra::Vector2<f32>;

    type Accumulator = nalgebra::Vector2<f32>;

    #[inline(always)]
    fn new() -> Self::Accumulator {
        nalgebra::vector![0.0, 0.0]
    }

    #[inline(always)]
    fn add(&self, acc: &mut Self::Accumulator, inp: Self::InputPixel, coeff: f32) {
        acc.x += inp.x * coeff;
        acc.y += inp.y * coeff;
    }

    #[inline(always)]
    fn add_acc(acc: &mut Self::Accumulator, inp: Self::Accumulator, coeff: f32) {
        acc.x += inp.x * coeff;
        acc.y += inp.y * coeff;
    }

    #[inline(always)]
    fn into_pixel(&self, acc: Self::Accumulator) -> Self::OutputPixel {
        acc
    }
}

fn fill_real_imag_csv<P: AsRef<Path>>(
    path: P,
    phase: f32,
    power_scale: f32,
    dimension_scale: [f32; 3],
    offset: [f32; 3],
    domain: [[f32; 2]; 3],
    dx: f32,
    texture_dx: f32,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> anyhow::Result<wgpu::TextureView> {
    let step_x = (domain[0][1] - domain[0][0]) / dx;
    let step_y = (domain[1][1] - domain[1][0]) / dx;

    let grid_x = step_x.ceil() as usize;
    let grid_y = step_y.ceil() as usize;

    let mut rdr = csv::Reader::from_path(path.as_ref())?;
    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;

    for record in rdr.records() {
        let record = record?;
        let x: f32 = record.get(0).unwrap().parse()?;
        let y: f32 = record.get(1).unwrap().parse()?;
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_y = min_y.min(y);
        max_y = max_y.max(y);
    }

    let width = max_x - min_x;
    let height = max_y - min_y;

    anyhow::ensure!(width > 0. && height > 0.);

    let texture_width = (width / texture_dx).ceil() as usize + 1;
    let texture_height = (height / texture_dx).ceil() as usize + 1;

    let mut input_texture =
        ndarray::Array2::<nalgebra::Vector2<f32>>::default((texture_width, texture_height).f());
    let (ps, pc) = phase.to_radians().sin_cos();

    let mut rdr = csv::Reader::from_path(path)?;

    for record in rdr.records() {
        let record = record?;
        let x: f32 = record.get(0).unwrap().parse()?;
        let y: f32 = record.get(1).unwrap().parse()?;
        let real_amp: f32 = record.get(2).unwrap().parse()?;
        let imag_amp: f32 = record.get(3).unwrap().parse()?;

        let x = ((x - min_x) / texture_dx).round() as usize;
        let y = ((y - min_y) / texture_dx).round() as usize;

        input_texture[[x, y]] =
            nalgebra::vector![real_amp * pc - imag_amp * ps, real_amp * ps + imag_amp * pc,]
                * power_scale;
    }

    let dst_width = (width * dimension_scale[0] / dx).ceil() as usize;
    let dst_height = (height * dimension_scale[1] / dx).ceil() as usize;

    let mut result_texture =
        ndarray::Array2::<nalgebra::Vector2<f32>>::default((dst_width, dst_height).f());

    let mut resizer = resize::new(
        texture_width,
        texture_height,
        dst_width,
        dst_height,
        RG32,
        resize::Type::Lanczos3,
    )?;

    resizer.resize(
        input_texture.as_slice_memory_order().unwrap(),
        result_texture.as_slice_memory_order_mut().unwrap(),
    )?;

    let mut embed_texture =
        ndarray::Array2::<nalgebra::Vector2<f32>>::default((grid_x, grid_y).f());

    let offset_x = (offset[0] / dx).round() as i32 + (grid_x as i32 - dst_width as i32) / 2;
    let offset_y = (offset[1] / dx).round() as i32 + (grid_y as i32 - dst_height as i32) / 2;

    for x in 0..dst_width as i32 {
        for y in 0..dst_height as i32 {
            let embed_x = x + offset_x;
            let embed_y = y + offset_y;

            if embed_x > 0 && embed_y > 0 && embed_x < grid_x as i32 && embed_y < grid_y as i32 {
                embed_texture[[embed_x as usize, embed_y as usize]] =
                    result_texture[[x as usize, y as usize]];
            }
        }
    }

    Ok(device
        .create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d {
                    width: grid_x as _,
                    height: grid_y as _,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rg32Float,
                usage: wgpu::TextureUsages::STORAGE_BINDING,
                view_formats: &[],
            },
            bytemuck::cast_slice(embed_texture.as_slice_memory_order().unwrap()),
        )
        .create_view(&wgpu::TextureViewDescriptor::default()))
}

fn fill_poing_cloud_csv<P: AsRef<Path>>(
    path: P,
    phase: f32,
    power_scale: f32,
    dimension_scale: [f32; 3],
    offset: [f32; 3],
    domain: [[f32; 2]; 3],
    dx: f32,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> anyhow::Result<wgpu::TextureView> {
    let step_x = (domain[0][1] - domain[0][0]) / dx;
    let step_y = (domain[1][1] - domain[1][0]) / dx;

    let grid_x = step_x.ceil() as usize;
    let grid_y = step_y.ceil() as usize;

    let mut rdr = csv::Reader::from_path(path)?;

    let interp = interpolator::Linear2DInterpolator::<2>::new(
        rdr.records()
            .map(|record| {
                let record = record.unwrap();
                let x: f32 = record.get(0).unwrap().parse().unwrap();
                let y: f32 = record.get(1).unwrap().parse().unwrap();
                let real_amp: f32 = record.get(2).unwrap().parse().unwrap();
                let imag_amp: f32 = record.get(3).unwrap().parse().unwrap();

                let x = x * dimension_scale[0] - domain[0][0] + offset[0];
                let y = y * dimension_scale[1] - domain[1][0] + offset[1];

                (nalgebra::vector![x as f64, y as f64], [real_amp, imag_amp])
            })
            .collect(),
    );

    let (ps, pc) = phase.to_radians().sin_cos();

    let texture_array: ndarray::Array2<nalgebra::Vector2<f32>> =
        ndarray::Array2::from_shape_fn((grid_x as usize, grid_y as usize).f(), |(x, y)| {
            let v = interp
                .interpolate(nalgebra::vector![
                    (x as f64 + 0.5) * dx as f64,
                    (y as f64 + 0.5) * dx as f64
                ])
                .unwrap_or_default();

            nalgebra::vector![v[0] * pc - v[1] * ps, v[0] * ps + v[1] * pc] * power_scale
        });

    Ok(device
        .create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d {
                    width: grid_x as _,
                    height: grid_y as _,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rg32Float,
                usage: wgpu::TextureUsages::STORAGE_BINDING,
                view_formats: &[],
            },
            bytemuck::cast_slice(texture_array.as_slice_memory_order().unwrap()),
        )
        .create_view(&wgpu::TextureViewDescriptor::default()))
}

fn main() -> anyhow::Result<()> {
    let options = GremOptions::parse();

    if options.info {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            ..Default::default()
        });
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: None,
            })
            .block_on()
            .unwrap();
        println!("Device: {:?}", adapter.get_info());
        println!("{:?}", adapter.limits());
        return Ok(());
    }

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::VULKAN,
        ..Default::default()
    });
    let visualize_component = if !options.no_visual {
        let event_loop = winit::event_loop::EventLoop::new()?;
        let window = std::sync::Arc::new(
            winit::window::WindowBuilder::new()
                .with_title("GREMS")
                .build(&event_loop)?,
        );
        (
            Some(event_loop),
            Some(unsafe { instance.create_surface(&window)? }),
            Some(window),
        )
    } else {
        (None, None, None)
    };
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: visualize_component.1.as_ref(),
        })
        .block_on()
        .unwrap();
    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                features: adapter.features(),
                limits: adapter.limits(),
            },
            None,
        )
        .block_on()?;

    let settings = config::Config::builder()
        .add_source(config::File::with_name(options.preset.as_ref().unwrap()))
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

    anyhow::ensure!(
        settings.domain[0][1] > settings.domain[0][0],
        "RHS of domain[0] is less or equal than LHS!"
    );
    anyhow::ensure!(
        settings.domain[1][1] > settings.domain[1][0],
        "RHS of domain[1] is less or equal than LHS!"
    );
    anyhow::ensure!(
        settings.domain[2][1] > settings.domain[2][0],
        "RHS of domain[2] is less or equal than LHS!"
    );

    let mode_source_bind_group_layout =
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::ReadOnly,
                        format: wgpu::TextureFormat::Rg32Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::ReadOnly,
                        format: wgpu::TextureFormat::Rg32Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::ReadOnly,
                        format: wgpu::TextureFormat::Rg32Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });

    let empty_placeholder = device
        .create_texture_with_data(
            &queue,
            &wgpu::TextureDescriptor {
                label: Some("EMPTY D2 Rg32Float"),
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rg32Float,
                usage: wgpu::TextureUsages::STORAGE_BINDING,
                view_formats: &[],
            },
            bytemuck::cast_slice(&[0f32; 2]),
        )
        .create_view(&wgpu::TextureViewDescriptor::default());

    let mut electric_sources = vec![];
    let mut magnetic_sources = vec![];

    for source in settings.sources.iter_mut() {
        match &mut source.mode {
            ModeSettings::Texture {
                ex,
                ey,
                ez,
                hx,
                hy,
                hz,
                spatial_step,
            } => {
                let ex = ex
                    .as_ref()
                    .map(|path| {
                        fill_real_imag_csv(
                            path,
                            source.phase,
                            source.power,
                            source.size,
                            source.position,
                            settings.domain,
                            settings.spatial_step,
                            *spatial_step,
                            &device,
                            &queue,
                        )
                    })
                    .transpose()?;
                let ey = ey
                    .as_ref()
                    .map(|path| {
                        fill_real_imag_csv(
                            path,
                            source.phase,
                            source.power,
                            source.size,
                            source.position,
                            settings.domain,
                            settings.spatial_step,
                            *spatial_step,
                            &device,
                            &queue,
                        )
                    })
                    .transpose()?;
                let ez = ez
                    .as_ref()
                    .map(|path| {
                        fill_real_imag_csv(
                            path,
                            source.phase,
                            source.power,
                            source.size,
                            source.position,
                            settings.domain,
                            settings.spatial_step,
                            *spatial_step,
                            &device,
                            &queue,
                        )
                    })
                    .transpose()?;

                if ex.is_some() || ey.is_some() || ez.is_some() {
                    let electric_source_bind_group =
                        device.create_bind_group(&wgpu::BindGroupDescriptor {
                            label: None,
                            layout: &mode_source_bind_group_layout,
                            entries: &[
                                wgpu::BindGroupEntry {
                                    binding: 0,
                                    resource: wgpu::BindingResource::TextureView(match &ex {
                                        Some(texture_view) => texture_view,
                                        None => &empty_placeholder,
                                    }),
                                },
                                wgpu::BindGroupEntry {
                                    binding: 1,
                                    resource: wgpu::BindingResource::TextureView(match &ey {
                                        Some(texture_view) => texture_view,
                                        None => &empty_placeholder,
                                    }),
                                },
                                wgpu::BindGroupEntry {
                                    binding: 2,
                                    resource: wgpu::BindingResource::TextureView(match &ez {
                                        Some(texture_view) => texture_view,
                                        None => &empty_placeholder,
                                    }),
                                },
                            ],
                        });

                    electric_sources.push(Source::Texture {
                        source_bind_group: electric_source_bind_group,
                        wavelength: source.wavelength,
                        delay: source.delay,
                        fwhm: source.fwhm,
                        z_layer: ((source.position[2] - settings.domain[2][0])
                            / settings.spatial_step)
                            .round() as u32,
                    });
                }

                let hx = hx
                    .as_ref()
                    .map(|path| {
                        fill_real_imag_csv(
                            path,
                            source.phase,
                            source.power,
                            source.size,
                            source.position,
                            settings.domain,
                            settings.spatial_step,
                            *spatial_step,
                            &device,
                            &queue,
                        )
                    })
                    .transpose()?;
                let hy = hy
                    .as_ref()
                    .map(|path| {
                        fill_real_imag_csv(
                            path,
                            source.phase,
                            source.power,
                            source.size,
                            source.position,
                            settings.domain,
                            settings.spatial_step,
                            *spatial_step,
                            &device,
                            &queue,
                        )
                    })
                    .transpose()?;
                let hz = hz
                    .as_ref()
                    .map(|path| {
                        fill_real_imag_csv(
                            path,
                            source.phase,
                            source.power,
                            source.size,
                            source.position,
                            settings.domain,
                            settings.spatial_step,
                            *spatial_step,
                            &device,
                            &queue,
                        )
                    })
                    .transpose()?;

                if hx.is_some() || hy.is_some() || hz.is_some() {
                    let magnetic_source_bind_group =
                        device.create_bind_group(&wgpu::BindGroupDescriptor {
                            label: None,
                            layout: &mode_source_bind_group_layout,
                            entries: &[
                                wgpu::BindGroupEntry {
                                    binding: 0,
                                    resource: wgpu::BindingResource::TextureView(match &hx {
                                        Some(texture_view) => texture_view,
                                        None => &empty_placeholder,
                                    }),
                                },
                                wgpu::BindGroupEntry {
                                    binding: 1,
                                    resource: wgpu::BindingResource::TextureView(match &hy {
                                        Some(texture_view) => texture_view,
                                        None => &empty_placeholder,
                                    }),
                                },
                                wgpu::BindGroupEntry {
                                    binding: 2,
                                    resource: wgpu::BindingResource::TextureView(match &hz {
                                        Some(texture_view) => texture_view,
                                        None => &empty_placeholder,
                                    }),
                                },
                            ],
                        });

                    magnetic_sources.push(Source::Texture {
                        source_bind_group: magnetic_source_bind_group,
                        wavelength: source.wavelength,
                        delay: source.delay,
                        fwhm: source.fwhm,
                        z_layer: ((source.position[2] - settings.domain[2][0])
                            / settings.spatial_step)
                            .round() as u32,
                    });
                }
            }
            ModeSettings::Volume { direction, field } => match field {
                fdtd::FieldType::E => electric_sources.push(Source::Volume {
                    direction: *direction,
                    wavelength: source.wavelength,
                    position: source.position,
                    size: source.size,
                    phase: source.phase,
                    delay: source.delay,
                    fwhm: source.fwhm,
                    power: source.power,
                }),
                fdtd::FieldType::H => magnetic_sources.push(Source::Volume {
                    direction: *direction,
                    wavelength: source.wavelength,
                    position: source.position,
                    size: source.size,
                    phase: source.phase,
                    delay: source.delay,
                    fwhm: source.fwhm,
                    power: source.power,
                }),
            },
            ModeSettings::PointCloud { file, exclude } => todo!(),
        }
    }

    if let (Some(event_loop), Some(surface), Some(window)) = visualize_component {
        let caps = surface.get_capabilities(&adapter);

        let mut surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: caps.formats[0],
            width: window.inner_size().width,
            height: window.inner_size().height,
            present_mode: wgpu::PresentMode::AutoNoVsync,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![caps.formats[0]],
        };

        surface.configure(&device, &surface_config);

        let mut brush =
            BrushBuilder::using_font_bytes(include_bytes!("../fonts/Roboto-Regular.ttf"))?.build(
                &device,
                surface_config.width,
                surface_config.height,
                surface_config.format,
            );

        let mut fdtd = fdtd::FDTD::new(
            &device,
            &queue,
            Some(surface_config.format),
            settings.spatial_step,
            settings.temporal_step,
            settings.domain,
            settings.models,
            settings.boundary,
            settings.default_slice,
            &settings.default_shader,
            settings.default_scaling_factor,
            settings.workgroup.unwrap_or({
                let cell =
                    (adapter.limits().max_compute_invocations_per_workgroup as f32).cbrt() as u32;
                WorkgroupSettings {
                    x: cell,
                    y: cell,
                    z: cell,
                }
            }),
            &mode_source_bind_group_layout,
        )?;

        let mut step_counter = 0;
        let mut now = std::time::Instant::now();
        let tau = std::time::Duration::from_secs_f32(1.0 / settings.steps_per_second_limit);
        let mut elapsed = std::time::Duration::ZERO;
        let mut paused = false;

        let mut last_display_step = 0u32;
        let mut last_display_time = std::time::Instant::now();
        let mut fps_counter = 0f32;
        let show_fps_duration = std::time::Duration::from_secs_f32(1f32);

        let mut ctrl_pressed = false;

        event_loop.run(move |event, target| match event {
        winit::event::Event::WindowEvent { window_id, event } if window_id == window.id() => {
            match event {
                winit::event::WindowEvent::CloseRequested => {
                    target.exit();
                }
                winit::event::WindowEvent::Resized(new_size) => {
                    if new_size.width > 0 && new_size.height > 0 {
                        surface_config.width = new_size.width;
                        surface_config.height = new_size.height;
                        surface.configure(&device, &surface_config);
                        brush.resize_view(new_size.width as f32, new_size.height as f32, &queue);
                        window.request_redraw();
                    }
                },
                winit::event::WindowEvent::MouseWheel { delta, .. } => match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, row) => {
                        fdtd.offset_slice_position(row);
                        window.request_redraw();
                    }
                    winit::event::MouseScrollDelta::PixelDelta(_) => unimplemented!(),
                },
                winit::event::WindowEvent::KeyboardInput {
                    event: KeyEvent {
                        physical_key: PhysicalKey::Code(keycode),
                        state: ElementState::Pressed,
                        ..
                    },
                    ..
                } if ctrl_pressed => match keycode {
                    winit::keyboard::KeyCode::Space => {
                        paused = !paused;
                        if !paused {
                            elapsed = std::time::Duration::ZERO;
                            now = std::time::Instant::now();
                        }
                    },
                    winit::keyboard::KeyCode::KeyX => {
                        fdtd.set_slice_mode(fdtd::SliceMode::X);
                        window.request_redraw();
                    },
                    winit::keyboard::KeyCode::KeyY => {
                        fdtd.set_slice_mode(fdtd::SliceMode::Y);
                        window.request_redraw();
                    },
                    winit::keyboard::KeyCode::KeyZ => {
                        fdtd.set_slice_mode(fdtd::SliceMode::Z);
                        window.request_redraw();
                    }
                    winit::keyboard::KeyCode::KeyE => {
                        fdtd.set_field_view_mode(fdtd::FieldType::E);
                        window.request_redraw();
                    }
                    winit::keyboard::KeyCode::KeyH => {
                        fdtd.set_field_view_mode(fdtd::FieldType::H);
                        window.request_redraw();
                    }
                    winit::keyboard::KeyCode::ArrowLeft => {
                        fdtd.scale_linear(-1.0);
                        window.request_redraw();
                    }
                    winit::keyboard::KeyCode::ArrowRight => {
                        fdtd.scale_linear(1.0);
                        window.request_redraw();
                    }
                    winit::keyboard::KeyCode::ArrowUp => {
                        fdtd.scale_exponential(1);
                        window.request_redraw();
                    }
                    winit::keyboard::KeyCode::ArrowDown => {
                        fdtd.scale_exponential(-1);
                        window.request_redraw();
                    }
                    _ => (),
                }
                winit::event::WindowEvent::ModifiersChanged(modifiers) => {
                    ctrl_pressed = modifiers.state().control_key();
                }
                winit::event::WindowEvent::DroppedFile(file) => {
                    fdtd.reload_shader(file, &device, surface_config.format).unwrap();
                    window.request_redraw();
                }
                winit::event::WindowEvent::RedrawRequested => {
                    let dt = now.elapsed();
                    elapsed += dt;
                    now = std::time::Instant::now();

                    if elapsed < tau {
                        return;
                    }
                    while elapsed >= tau {
                        elapsed -= tau;
                    }

                    if paused {
                        return;
                    }

                    let mut encoder =
                        device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());

                    fdtd.update_magnetic_field(&mut encoder);
                    for source in magnetic_sources.iter() {
                        match source {
                            Source::Texture { source_bind_group, z_layer, wavelength, delay, fwhm } => {
                                let pulse_envelope = (-((std::f32::consts::PI
                                    * fwhm
                                    * (step_counter as f32 * settings.temporal_step - delay))
                                    .powi(2)
                                    / (4.0 * (2.0 as f32).ln()))
                                .powi(2))
                                .exp();

                                let position = [
                                    settings.boundary.get_extra_grid_extent() / 2,
                                    settings.boundary.get_extra_grid_extent() / 2,
                                    settings.boundary.get_extra_grid_extent() / 2 + z_layer,
                                ];

                                let phasor = (-2.0
                                    * std::f32::consts::PI
                                    * (step_counter as f32 * settings.temporal_step - delay)
                                    / wavelength).sin_cos();

                                fdtd.excite_magnetic_field_mode(&mut encoder, position, phasor, pulse_envelope, source_bind_group);
                            },
                            Source::Volume { direction, wavelength, position, size, phase, delay, fwhm, power } => {
                                let pulse_envelope = (-((std::f32::consts::PI
                                    * fwhm
                                    * (step_counter as f32 * settings.temporal_step - delay))
                                    .powi(2)
                                    / (4.0 * (2.0 as f32).ln()))
                                .powi(2))
                                .exp();

                                let cw_component = (-2.0
                                    * std::f32::consts::PI
                                    * (step_counter as f32 * settings.temporal_step - delay)
                                    / wavelength
                                    + phase.to_radians())
                                .cos();

                                let direction = nalgebra::Vector3::from(*direction).normalize();
                                let actual_position = [
                                    ((position[0] - settings.domain[0][0] - size[0] / 2.0)
                                        / settings.spatial_step)
                                        .ceil() as u32 + settings.boundary.get_extra_grid_extent() / 2,
                                    ((position[1] - settings.domain[1][0] - size[1] / 2.0 )
                                        / settings.spatial_step)
                                        .ceil() as u32 + settings.boundary.get_extra_grid_extent() / 2,
                                    ((position[2] - settings.domain[2][0] - size[2] / 2.0)
                                        / settings.spatial_step)
                                        .ceil() as u32 + settings.boundary.get_extra_grid_extent() / 2,
                                ];
                                let actual_size = [
                                    if size[0] > 0.0 {
                                        (size[0] / settings.spatial_step).ceil() as u32
                                    } else {
                                        1
                                    },
                                    if size[1] > 0.0 {
                                        (size[1] / settings.spatial_step).ceil() as u32
                                    } else {
                                        1
                                    },
                                    if size[2] > 0.0 {
                                        (size[2] / settings.spatial_step).ceil() as u32
                                    } else {
                                        1
                                    },
                                ];

                                fdtd.excite_magnetic_field_volume(
                                    &mut encoder,
                                    actual_position,
                                    actual_size,
                                    (direction * pulse_envelope * cw_component * *power).into(),
                                );
                            },
                        }
                    }
                    fdtd.update_electric_field(&mut encoder);
                    for source in electric_sources.iter() {
                        match source {
                            Source::Texture { source_bind_group, z_layer, wavelength, delay, fwhm } => {
                                let pulse_envelope = (-((std::f32::consts::PI
                                    * fwhm
                                    * (step_counter as f32 * settings.temporal_step - delay))
                                    .powi(2)
                                    / (4.0 * (2.0 as f32).ln()))
                                .powi(2))
                                .exp();

                                let position = [
                                    settings.boundary.get_extra_grid_extent() / 2,
                                    settings.boundary.get_extra_grid_extent() / 2,
                                    settings.boundary.get_extra_grid_extent() / 2 + z_layer,
                                ];

                                let phasor = (-2.0
                                    * std::f32::consts::PI
                                    * (step_counter as f32 * settings.temporal_step - delay)
                                    / wavelength).sin_cos();

                                fdtd.excite_electric_field_mode(&mut encoder, position, phasor, pulse_envelope, source_bind_group);
                            },
                           Source::Volume { direction, wavelength, position, size, phase, delay, fwhm, power } => {
                                let pulse_envelope = (-((std::f32::consts::PI
                                    * fwhm
                                    * (step_counter as f32 * settings.temporal_step - delay))
                                    .powi(2)
                                    / (4.0 * (2.0 as f32).ln()))
                                .powi(2))
                                .exp();

                                let cw_component = (-2.0
                                    * std::f32::consts::PI
                                    * (step_counter as f32 * settings.temporal_step - delay)
                                    / wavelength
                                    + phase.to_radians())
                               .cos();

                                let direction = nalgebra::Vector3::from(*direction).normalize();
                                let actual_position = [
                                    ((position[0] - settings.domain[0][0] - size[0] / 2.0)
                                        / settings.spatial_step)
                                        .ceil() as u32 + settings.boundary.get_extra_grid_extent() / 2,
                                    ((position[1] - settings.domain[1][0] - size[1] / 2.0 )
                                        / settings.spatial_step)
                                        .ceil() as u32 + settings.boundary.get_extra_grid_extent() / 2,
                                    ((position[2] - settings.domain[2][0] - size[2] / 2.0)
                                        / settings.spatial_step)
                                        .ceil() as u32 + settings.boundary.get_extra_grid_extent() / 2,
                                ];
                                let actual_size = [
                                    if size[0] > 0.0 {
                                        (size[0] / settings.spatial_step).ceil() as u32
                                    } else {
                                        1
                                    },
                                    if size[1] > 0.0 {
                                        (size[1] / settings.spatial_step).ceil() as u32
                                    } else {
                                        1
                                    },
                                    if size[2] > 0.0 {
                                        (size[2] / settings.spatial_step).ceil() as u32
                                    } else {
                                        1
                                    },
                                ];

                                fdtd.excite_electric_field_volume(
                                    &mut encoder,
                                    actual_position,
                                    actual_size,
                                    (direction * pulse_envelope * cw_component * *power).into(),
                                );
                            },
                        }
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
                            TimingSettings::Time(time) => {
                                (time / settings.temporal_step).round() as u32
                            }
                        };

                        if step == step_counter {
                            let mut export_encoder = device
                                .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
                            match export.export {
                                ExportFieldSettings::D3 { field } => {
                                    let field_texture = match field {
                                        fdtd::FieldType::E => {
                                            fdtd.get_electric_field_textures()[0].as_image_copy()
                                        }
                                        fdtd::FieldType::H => {
                                            fdtd.get_magnetic_field_textures()[0].as_image_copy()
                                        }
                                    };

                                   let dimension = fdtd.get_dimension();

                                    let bytes_per_pixel = 1 * std::mem::size_of::<f32>() as u32;
                                    let unpadded_bytes_per_row = dimension[0] * bytes_per_pixel;
                                    let padded_bytes_per_row_padding =
                                        (wgpu::COPY_BYTES_PER_ROW_ALIGNMENT
                                            - unpadded_bytes_per_row
                                                % wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
                                            % wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
                                    let padded_bytes_per_row =
                                        unpadded_bytes_per_row + padded_bytes_per_row_padding;

                                    let copy_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                                        label: None,
                                        size: (padded_bytes_per_row * dimension[1] * dimension[2])
                                            as u64,
                                        usage: wgpu::BufferUsages::COPY_DST
                                            | wgpu::BufferUsages::MAP_READ,
                                        mapped_at_creation: false,
                                    });

                                    export_encoder.copy_texture_to_buffer(
                                        field_texture,
                                        wgpu::ImageCopyBufferBase {
                                            buffer: &copy_buffer,
                                            layout: wgpu::ImageDataLayout {
                                                offset: 0,
                                                bytes_per_row: Some(padded_bytes_per_row),
                                                rows_per_image: Some(dimension[1]),
                                            },
                                        },
                                        wgpu::Extent3d {
                                            width: dimension[0],
                                            height: dimension[1],
                                            depth_or_array_layers: dimension[2],
                                        },
                                    );
                                    let index = queue.submit(Some(export_encoder.finish()));

                                    let (sender, receiver) =
                                        futures_intrusive::channel::shared::oneshot_channel();
                                    let map_slice = copy_buffer.slice(..);
                                    map_slice.map_async(wgpu::MapMode::Read, move |v| {
                                        sender.send(v).unwrap()
                                    });
                                    device.poll(wgpu::Maintain::WaitForSubmissionIndex(index));

                                    if let Some(Ok(())) = receiver.receive().block_on() {
                                        {
                                            let data = map_slice.get_mapped_range();
                                            let raw_data: Vec<u8> = data
                                                .chunks(padded_bytes_per_row as usize)
                                                .flat_map(|row| &row[..unpadded_bytes_per_row as usize])
                                                .cloned()
                                                .collect();

                                            let mut dds =
                                                ddsfile::Dds::new_dxgi(ddsfile::NewDxgiParams {
                                                    height: dimension[1],
                                                    width: dimension[0],
                                                    depth: Some(dimension[2]),
                                                    format: ddsfile::DxgiFormat::R32_Float,
                                                    mipmap_levels: None,
                                                    array_layers: None,
                                                    caps2: None,
                                                    is_cubemap: false,
                                                    resource_dimension:
                                                        ddsfile::D3D10ResourceDimension::Texture3D,
                                                    alpha_mode: ddsfile::AlphaMode::Unknown,
                                                })
                                                .unwrap();

                                            dds.data = raw_data;

                                            let mut file = std::fs::OpenOptions::new()
                                                .write(true)
                                                .truncate(true)
                                                .create(true)
                                                .open(std::env::current_dir().unwrap().join(format!(
                                                    "{}-D3-{:?}-{}.dds",
                                                    options.preset.as_ref().unwrap(),
                                                    field,
                                                    step_counter
                                                )))
                                                .unwrap();

                                            dds.write(&mut file).unwrap();
                                        }
                                        copy_buffer.unmap();
                                    }
                                }
                                ExportFieldSettings::D2(ref _settings) => {
                                    eprintln!("2D Slice Not Yet Implemented")
                                }
                            }
                            settings.exports.remove(0);
                            now = std::time::Instant::now();
                            elapsed = std::time::Duration::ZERO;
                        } else {
                            break;
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

                        brush.queue(&device, &queue, vec![TextSection {
                            screen_position: (0.0, 0.0),
                            bounds: (surface_config.width as f32, surface_config.height as f32),
                            text: vec![Text::new(&format!(
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
                        }]).unwrap();

                    {
                        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: None,
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view: &surf_texture_view,
                                resolve_target: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                                    store: wgpu::StoreOp::Store,
                                },
                            })],
                            depth_stencil_attachment: None,
                            timestamp_writes: None,
                            occlusion_query_set: None,
                        });

                        fdtd.visualize(&mut render_pass);
                        brush.draw(&mut render_pass);
                    }

                    let last_display_delta = last_display_time.elapsed();
                    if last_display_delta >= show_fps_duration {
                        fps_counter = (step_counter - last_display_step) as f32 / last_display_delta.as_secs_f32();
                        last_display_time = std::time::Instant::now();
                        last_display_step = step_counter;
                    }

                    queue.submit(std::iter::once(encoder.finish()));
                    surface_texture.present();
                }
                _ => (),
            }
        }
        winit::event::Event::AboutToWait => if !paused {
            window.request_redraw();
            target.set_control_flow(winit::event_loop::ControlFlow::Poll);
        } else {
            target.set_control_flow(winit::event_loop::ControlFlow::Wait);
        },
        _ => (),
    })?;
    } else {
        assert!(
            settings.pause_at.len() > 0,
            "MUST have pause_at when running in non visualized mode"
        );

        unimplemented!("currently unsupported because too buggy");
    }

    Ok(())
}
