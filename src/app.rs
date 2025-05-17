pub mod keymap;

use lazy_static::lazy_static;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use winit::platform::macos::WindowExtMacOS;
use winit::{
    application::ApplicationHandler, dpi::LogicalSize, event::WindowEvent,
    platform::macos::WindowAttributesExtMacOS, window::Window,
};

use crate::beeper::Beeper;
use crate::settings::Settings;
use crate::vm::Vm;
use crate::wgpu_ctx::WgpuCtx;

lazy_static! {
    pub static ref TARGET_FRAME_TIME: Duration = Duration::from_secs_f64(1.0 / 60.0);
}

pub struct App<'window> {
    window: Option<Arc<Window>>,
    wgpu_ctx: Option<WgpuCtx<'window>>,
    vm: Vm,
    settings: Arc<RwLock<Settings>>,
    beeper: Beeper,
    last_frame_time: Instant,
}

impl App<'_> {
    pub fn new(vm: Vm, settings: Arc<RwLock<Settings>>) -> Self {
        let (beep_freq, scale) = {
            let settings = settings.read().unwrap();
            (settings.beep_freq, settings.scale_mode)
        };

        let mut beeper = Beeper::new();
        _ = beeper.init_stream();
        beeper.set_scale_mode(scale);
        beeper.set_freq(beep_freq);

        Self {
            window: None,
            wgpu_ctx: None,
            vm,
            settings,
            beeper,
            last_frame_time: Instant::now(),
        }
    }
}

impl ApplicationHandler for App<'_> {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let window = {
            let window_attrs = Window::default_attributes()
                .with_inner_size(LogicalSize::new(640 * 2, 320 * 2))
                .with_resizable(true)
                .with_titlebar_transparent(true)
                .with_fullsize_content_view(true)
                .with_title_hidden(true)
                .with_has_shadow(true)
                .with_transparent(true)
                .with_title("CHIP8");

            event_loop
                .create_window(window_attrs)
                .expect("Failed to create window")
        };

        let window = Arc::new(window);
        let wgpu_ctx = WgpuCtx::new(Arc::clone(&window), Arc::clone(&self.settings));
        self.window = Some(window);
        self.wgpu_ctx = Some(wgpu_ctx);
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        self.wgpu_ctx
            .as_mut()
            .unwrap()
            .egui_renderer
            .handle_input(self.window.as_ref().unwrap(), &event);

        match event {
            WindowEvent::RedrawRequested => {
                if let (Some(window), Some(wgpu_ctx)) = (&self.window, self.wgpu_ctx.as_mut()) {
                    let (
                        ticks_per_frame,
                        show_settings,
                        beep_freqency,
                        window_has_shadow,
                        scale_mode,
                    ) = {
                        let settings = self.settings.read().unwrap();
                        (
                            settings.ticks_per_frame,
                            settings.show_settings,
                            settings.beep_freq,
                            settings.window_has_shadow,
                            settings.scale_mode,
                        )
                    };

                    window.set_has_shadow(window_has_shadow);

                    self.beeper.set_scale_mode(scale_mode);
                    if !scale_mode {
                        self.beeper.set_freq(beep_freqency);
                    }

                    if !show_settings {
                        for _ in 0..ticks_per_frame {
                            self.vm.tick();
                        }

                        if self.vm.st > 0 {
                            self.beeper.play();
                        } else {
                            self.beeper.pause();
                        }

                        self.vm.delay_timer();
                        self.vm.sound_timer();
                    }

                    wgpu_ctx.draw(&self.vm.vb);
                    window.request_redraw();

                    let elapsed = self.last_frame_time.elapsed();
                    if elapsed < *TARGET_FRAME_TIME {
                        std::thread::sleep(*TARGET_FRAME_TIME - elapsed);
                    }

                    self.last_frame_time = Instant::now();
                }
            }

            WindowEvent::CloseRequested => {
                event_loop.exit();
            }

            WindowEvent::Resized(new_size) => {
                if let (Some(window), Some(wgpu_ctx)) = (&self.window, self.wgpu_ctx.as_mut()) {
                    wgpu_ctx.resize(new_size);
                    window.request_redraw();
                }
            }

            WindowEvent::KeyboardInput { event, .. } => {
                use keymap::KEYMAP;
                use winit::event::ElementState;
                use winit::keyboard::KeyCode;

                if event.physical_key == KeyCode::Escape {
                    event_loop.exit();
                }

                if event.physical_key == KeyCode::Semicolon && event.state == ElementState::Pressed
                {
                    let current = { self.settings.read().unwrap().show_settings };

                    self.settings.write().unwrap().show_settings = !current;
                    return;
                }

                let key_num = KEYMAP.iter().position(|&kc| kc == event.physical_key);
                if key_num.is_none() {
                    return;
                }

                self.vm
                    .set_kb(key_num.unwrap(), event.state == ElementState::Pressed);
            }

            _ => {}
        }
    }
}
