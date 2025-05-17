use std::sync::Arc;

use winit::{event::WindowEvent, window::Window};

pub struct EguiRenderer {
    state: egui_winit::State,
    pub renderer: egui_wgpu::Renderer,
    frame_started: bool,
}

impl EguiRenderer {
    pub fn new(
        device: &wgpu::Device,
        texture_format: wgpu::TextureFormat,
        window: Arc<Window>,
    ) -> Self {
        let egui_ctx = egui::Context::default();

        let state = egui_winit::State::new(
            egui_ctx,
            egui::viewport::ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            None,
            Some(2048),
        );

        let renderer = egui_wgpu::Renderer::new(device, texture_format, None, 1, true);

        Self {
            state,
            renderer,
            frame_started: false,
        }
    }

    pub fn handle_input(&mut self, window: &Window, event: &WindowEvent) {
        _ = self.state.on_window_event(window, event);
    }

    pub fn context(&self) -> &egui::Context {
        self.state.egui_ctx()
    }

    pub fn set_ppp(&mut self, ppp: f32) {
        self.state.egui_ctx().set_pixels_per_point(ppp);
    }

    pub fn begin_frame(&mut self, window: &Window) {
        let raw_input = self.state.take_egui_input(window);
        self.state.egui_ctx().begin_pass(raw_input);
        self.frame_started = true;
    }

    pub fn end_frame_and_draw(
        &mut self,
        window: &Window,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        texture_view: &wgpu::TextureView,
        surface_cfg: &wgpu::SurfaceConfiguration,
    ) {
        if !self.frame_started {
            panic!("begin_frame must must be called before end_frame_and_draw");
        }

        let screen_desc = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [surface_cfg.width, surface_cfg.height],
            pixels_per_point: window.scale_factor() as f32,
        };

        self.set_ppp(screen_desc.pixels_per_point);

        let full_output = self.state.egui_ctx().end_pass();
        self.state
            .handle_platform_output(window, full_output.platform_output);

        let tris = self
            .state
            .egui_ctx()
            .tessellate(full_output.shapes, self.state.egui_ctx().pixels_per_point());

        for (id, image_delta) in &full_output.textures_delta.set {
            self.renderer
                .update_texture(device, queue, *id, image_delta);
        }

        self.renderer
            .update_buffers(device, queue, encoder, &tris, &screen_desc);

        let render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("egui main render pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: texture_view,
                resolve_target: None,
                ops: egui_wgpu::wgpu::Operations {
                    load: egui_wgpu::wgpu::LoadOp::Load,
                    store: egui_wgpu::wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        self.renderer
            .render(&mut render_pass.forget_lifetime(), &tris, &screen_desc);

        for x in &full_output.textures_delta.free {
            self.renderer.free_texture(x);
        }

        self.frame_started = false;
    }
}
