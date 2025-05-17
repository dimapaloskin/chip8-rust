use std::sync::{Arc, RwLock};
use std::time::Instant;
use std::{borrow::Cow, num::NonZeroU64};

use wgpu::util::DeviceExt;
use winit::window::Window;

use crate::egui::EguiRenderer;
use crate::settings::Settings;
use crate::ui::Ui;
use crate::video_buffer::VideoBuffer;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct ScreenSize {
    pub width: f32,
    pub height: f32,
}

impl Default for ScreenSize {
    fn default() -> Self {
        Self {
            width: 0.0,
            height: 0.0,
        }
    }
}

pub struct WgpuCtx<'window> {
    window: Arc<Window>,
    settings: Arc<RwLock<Settings>>,
    surface: wgpu::Surface<'window>,
    surface_cfg: wgpu::SurfaceConfiguration,
    device: wgpu::Device,
    queue: wgpu::Queue,
    render_pipeline: wgpu::RenderPipeline,
    postprocess_pipeline: wgpu::RenderPipeline,
    postprocess_sampler: wgpu::Sampler,

    msaa_texture: wgpu::Texture,
    msaa_view: wgpu::TextureView,
    postprocess_texture: wgpu::Texture,
    postprocess_view: wgpu::TextureView,

    pub egui_renderer: EguiRenderer,
    ui: Ui,

    fg_color_uniform_buffer: wgpu::Buffer,
    bg_color_uniform_buffer: wgpu::Buffer,
    screen_size_uniform_buffer: wgpu::Buffer,
    time_uniform_buffer: wgpu::Buffer,
    pp_enabled_uniform_buffer: wgpu::Buffer,
    sepia_amount_uniform_buffer: wgpu::Buffer,
    uniforms_bind_group: wgpu::BindGroup,

    video_buffer: wgpu::Buffer,
    video_bind_group: wgpu::BindGroup,

    postprocess_bind_group_layout: wgpu::BindGroupLayout,
    postprocess_bind_group: wgpu::BindGroup,

    start: Instant,
}

impl<'window> WgpuCtx<'window> {
    pub fn new(window: Arc<Window>, settings: Arc<RwLock<Settings>>) -> WgpuCtx<'window> {
        pollster::block_on(Self::new_async(window, settings))
    }

    async fn new_async(window: Arc<Window>, settings: Arc<RwLock<Settings>>) -> WgpuCtx<'window> {
        let ui = Ui::new(Arc::clone(&settings));
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());

        let surface = instance
            .create_surface(Arc::clone(&window))
            .expect("Failed to create surface");

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("Failed to find an appropriate adapter");

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("wgpu_device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    memory_hints: wgpu::MemoryHints::Performance,
                },
                None,
            )
            .await
            .expect("Failed to create device");

        let size = window.inner_size();
        let width = size.width.max(1);
        let height = size.height.max(1);

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .expect("Failed to find a suitable surface format");

        let alpha_mode = if surface_caps
            .alpha_modes
            .contains(&wgpu::CompositeAlphaMode::PostMultiplied)
        {
            wgpu::CompositeAlphaMode::PostMultiplied
        } else if surface_caps
            .alpha_modes
            .contains(&wgpu::CompositeAlphaMode::PreMultiplied)
        {
            wgpu::CompositeAlphaMode::PreMultiplied
        } else {
            surface_caps.alpha_modes[0]
        };

        let surface_cfg = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width,
            height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&device, &surface_cfg);

        let (msaa_texture, msaa_view) = Self::recreate_msaa(&device, &surface_cfg);
        let (postprocess_texture, postprocess_view) =
            Self::recreate_postprocess(&device, &surface_cfg);

        let shader_source = include_str!("shader.wgsl");
        let postprocess_shader_source = include_str!("postprocess_shader.wgsl");

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(shader_source)),
        });

        let postprocess_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("postprocess shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(postprocess_shader_source)),
        });

        let postprocess_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("postprocess sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let (fg_color, bg_color) = {
            let settings = settings.read().unwrap();

            (settings.fg_color, settings.bg_color)
        };

        let fg_color_uniform_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("color uniform buffer"),
                contents: bytemuck::bytes_of(&fg_color),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

        let bg_color_uniform_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("background color uniform buffer"),
                contents: bytemuck::bytes_of(&bg_color),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

        let screen_size_uniform_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("screen size uniform buffer"),
                contents: bytemuck::bytes_of(&ScreenSize::default()),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

        let time_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("u_time uniform buffer"),
            contents: bytemuck::bytes_of(&0.0f32),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let pp_enabled = { settings.read().unwrap().pp_enabled as u32 };
        let pp_enabled_uniform_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("postprocess bool toggle buffer"),
                contents: bytemuck::bytes_of(&pp_enabled),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

        let sepia_amount = { settings.read().unwrap().sepia_amount };
        let sepia_amount_uniform_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("sepia amount buffer"),
                contents: bytemuck::bytes_of(&sepia_amount),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

        let uniforms_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("color bind group layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        let uniforms_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("color bind group"),
            layout: &uniforms_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: fg_color_uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: bg_color_uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: screen_size_uniform_buffer.as_entire_binding(),
                },
            ],
        });

        let video_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("video storage buffer"),
            contents: VideoBuffer::default().as_bytes(),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        let video_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("video bind group layour"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: Some(
                            NonZeroU64::new(std::mem::size_of::<VideoBuffer>() as u64).unwrap(),
                        ),
                    },
                    count: None,
                }],
            });

        let video_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("video storage bind group"),
            layout: &video_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: video_buffer.as_entire_binding(),
            }],
        });

        let postprocess_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("postprocess bind group layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 4,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 5,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        let postprocess_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("postprocess bind group"),
            layout: &postprocess_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: screen_size_uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&postprocess_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&postprocess_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: time_uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: pp_enabled_uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: sepia_amount_uniform_buffer.as_entire_binding(),
                },
            ],
        });

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("pipeline layout"),
                bind_group_layouts: &[&uniforms_bind_group_layout, &video_bind_group_layout],
                push_constant_ranges: &[],
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("render pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_cfg.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 4,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });

        let postprocess_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("postprocess pipeline layout"),
                bind_group_layouts: &[&postprocess_bind_group_layout],
                push_constant_ranges: &[],
            });

        let postprocess_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("postprocess pipeline"),
            layout: Some(&postprocess_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &postprocess_shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &postprocess_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_cfg.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let egui_renderer = EguiRenderer::new(&device, surface_format, Arc::clone(&window));

        Self {
            window,
            settings,
            surface,
            surface_cfg,
            device,
            queue,
            render_pipeline,
            postprocess_pipeline,
            postprocess_sampler,

            msaa_texture,
            msaa_view,
            postprocess_texture,
            postprocess_view,

            egui_renderer,
            ui,

            fg_color_uniform_buffer,
            bg_color_uniform_buffer,
            screen_size_uniform_buffer,
            time_uniform_buffer,
            pp_enabled_uniform_buffer,
            sepia_amount_uniform_buffer,
            uniforms_bind_group,

            video_buffer,
            video_bind_group,

            postprocess_bind_group,
            postprocess_bind_group_layout,

            start: Instant::now(),
        }
    }

    fn recreate_msaa(
        device: &wgpu::Device,
        surface_cfg: &wgpu::SurfaceConfiguration,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("render texture"),
            dimension: wgpu::TextureDimension::D2,
            format: surface_cfg.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            mip_level_count: 1,
            sample_count: 4,
            size: wgpu::Extent3d {
                width: surface_cfg.width,
                height: surface_cfg.height,
                depth_or_array_layers: 1,
            },
            view_formats: &[surface_cfg.format],
        });

        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("render texture view"),
            ..Default::default()
        });

        (texture, texture_view)
    }

    fn recreate_postprocess(
        device: &wgpu::Device,
        surface_cfg: &wgpu::SurfaceConfiguration,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("postprocess texture"),
            dimension: wgpu::TextureDimension::D2,
            format: surface_cfg.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST,
            mip_level_count: 1,
            sample_count: 1,
            size: wgpu::Extent3d {
                width: surface_cfg.width,
                height: surface_cfg.height,
                depth_or_array_layers: 1,
            },
            view_formats: &[surface_cfg.format],
        });

        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("postprocess texture view"),
            ..Default::default()
        });

        (texture, texture_view)
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        self.surface_cfg.width = new_size.width.max(1);
        self.surface_cfg.height = new_size.height.max(1);
        self.surface.configure(&self.device, &self.surface_cfg);

        (self.msaa_texture, self.msaa_view) = Self::recreate_msaa(&self.device, &self.surface_cfg);
        (self.postprocess_texture, self.postprocess_view) =
            Self::recreate_postprocess(&self.device, &self.surface_cfg);

        self.postprocess_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("postprocess bind group"),
            layout: &self.postprocess_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.screen_size_uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&self.postprocess_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.postprocess_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.time_uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: self.pp_enabled_uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: self.sepia_amount_uniform_buffer.as_entire_binding(),
                },
            ],
        });

        let ss = ScreenSize {
            width: self.surface_cfg.width as f32,
            height: self.surface_cfg.height as f32,
        };

        self.queue
            .write_buffer(&self.screen_size_uniform_buffer, 0, bytemuck::bytes_of(&ss));
    }

    fn update_buffers(&self, vb: &VideoBuffer) {
        use bytemuck::bytes_of;

        let (fg_color, bg_color, pp_enabled, sepia_amount) = {
            let settings = self.settings.read().unwrap();

            (
                settings.fg_color,
                settings.bg_color,
                settings.pp_enabled as u32,
                settings.sepia_amount,
            )
        };

        self.queue
            .write_buffer(&self.fg_color_uniform_buffer, 0, bytes_of(&fg_color));

        self.queue
            .write_buffer(&self.bg_color_uniform_buffer, 0, bytes_of(&bg_color));

        self.queue
            .write_buffer(&self.video_buffer, 0, vb.as_bytes());

        let now = self.start.elapsed().as_secs_f32();
        self.queue
            .write_buffer(&self.time_uniform_buffer, 0, bytemuck::bytes_of(&now));

        self.queue
            .write_buffer(&self.pp_enabled_uniform_buffer, 0, bytes_of(&pp_enabled));

        self.queue.write_buffer(
            &self.sepia_amount_uniform_buffer,
            0,
            bytes_of(&sepia_amount),
        );
    }

    pub fn draw(&mut self, vb: &VideoBuffer) {
        self.update_buffers(vb);

        let target_texture = self.surface.get_current_texture().unwrap();
        let target_view = target_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor {
                ..Default::default()
            });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());

        let color_attachment = wgpu::RenderPassColorAttachment {
            view: &self.msaa_view,
            resolve_target: Some(&self.postprocess_view),
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 1.0,
                }),
                store: wgpu::StoreOp::Store,
            },
        };

        let render_pass_desc = wgpu::RenderPassDescriptor {
            label: Some("render pass descriptor"),
            color_attachments: &[Some(color_attachment)],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        };

        let postprocess_render_pass_desc = wgpu::RenderPassDescriptor {
            label: Some("postprocess render pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        };

        {
            let mut render_pass = encoder.begin_render_pass(&render_pass_desc);
            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.uniforms_bind_group, &[]);
            render_pass.set_bind_group(1, &self.video_bind_group, &[]);
            render_pass.draw(0..6, 0..1);
        }

        {
            let mut pp_pass = encoder.begin_render_pass(&postprocess_render_pass_desc);
            pp_pass.set_pipeline(&self.postprocess_pipeline);
            pp_pass.set_bind_group(0, &self.postprocess_bind_group, &[]);
            pp_pass.draw(0..6, 0..1);
        }

        let show_settings = {
            let settings = self.settings.read().unwrap();
            settings.show_settings
        };

        if show_settings {
            self.render_egui(&mut encoder, &target_view);
        }

        self.queue.submit(std::iter::once(encoder.finish()));

        target_texture.present();
    }

    fn render_egui(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        texture_view: &wgpu::TextureView,
    ) {
        self.egui_renderer.begin_frame(self.window.as_ref());

        self.ui.draw(&self.egui_renderer);

        self.egui_renderer.end_frame_and_draw(
            self.window.as_ref(),
            &self.device,
            &self.queue,
            encoder,
            texture_view,
            &self.surface_cfg,
        );
    }
}
