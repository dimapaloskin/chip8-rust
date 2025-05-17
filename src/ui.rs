use std::sync::{Arc, RwLock};

use crate::egui::EguiRenderer;
use crate::settings::Settings;

pub struct Ui {
    settings: Arc<RwLock<Settings>>,

    fg_color: [f32; 4],
    bg_color: [f32; 4],
    window_has_shadow: bool,

    ticks_per_frame: u32,
    beep_freqency: f32,
    scale_mode: bool,

    pp_enabled: bool,
    sepia_amount: f32,

    dirty: bool,
}

impl Ui {
    pub fn new(settings: Arc<RwLock<Settings>>) -> Self {
        let (
            fg_color,
            bg_color,
            ticks_per_frame,
            beep_freqency,
            scale_mode,
            window_has_shadow,
            pp_enabled,
            sepia_amount,
        ) = {
            let settings = settings.read().unwrap();

            (
                settings.fg_color,
                settings.bg_color,
                settings.ticks_per_frame,
                settings.beep_freq,
                settings.scale_mode,
                settings.window_has_shadow,
                settings.pp_enabled,
                settings.sepia_amount,
            )
        };

        Self {
            settings,
            fg_color,
            bg_color,
            window_has_shadow,
            ticks_per_frame,
            beep_freqency,
            scale_mode,
            pp_enabled,
            sepia_amount,
            dirty: false,
        }
    }

    pub fn draw(&mut self, egui_renderer: &EguiRenderer) {
        let ctx = egui_renderer.context();

        egui::Window::new("Settings")
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            // .default_pos(egui::pos2(30.0, 40.0))
            .title_bar(false)
            .resizable(false)
            .frame(egui::Frame::window(&ctx.style()).inner_margin(egui::Margin::symmetric(15, 15)))
            .show(ctx, |ui| {
                ui.heading("Settings");
                ui.add_space(10.0);

                ui.horizontal(|ui| {
                    ui.label("Foreground");
                    ui.add_space(30.0);
                    self.dirty |= ui
                        .color_edit_button_rgba_unmultiplied(&mut self.fg_color)
                        .changed();
                });

                ui.horizontal(|ui| {
                    ui.label("Background");
                    ui.add_space(30.0);
                    self.dirty |= ui
                        .color_edit_button_rgba_unmultiplied(&mut self.bg_color)
                        .changed();
                });

                self.dirty |= ui
                    .checkbox(&mut self.window_has_shadow, "Window has shadow")
                    .changed();

                ui.add_space(20.0);
                ui.vertical(|ui| {
                    ui.label("Ticks per frame");
                    self.dirty |= ui
                        .add(egui::Slider::new(&mut self.ticks_per_frame, 1..=700).show_value(true))
                        .changed();
                });

                ui.add_space(20.0);
                ui.vertical(|ui| {
                    ui.label("Beep frequency");
                    self.dirty |= ui
                        .add_enabled(
                            !self.scale_mode,
                            egui::Slider::new(&mut self.beep_freqency, 55.0..=880.0)
                                .show_value(true),
                        )
                        .changed();
                });
                ui.add_space(5.0);
                self.dirty |= ui
                    .checkbox(&mut self.scale_mode, "Major-scale mode")
                    .changed();

                ui.add_space(20.0);
                self.dirty |= ui
                    .checkbox(&mut self.pp_enabled, "Post-processing")
                    .changed();

                ui.add_space(20.0);
                ui.vertical(|ui| {
                    ui.label("Sepia amount");
                    self.dirty |= ui
                        .add(egui::Slider::new(&mut self.sepia_amount, 0.0..=1.0).show_value(true))
                        .changed();
                });
            });

        if self.dirty {
            self.update_settings();
        }
    }

    pub fn update_settings(&mut self) {
        self.dirty = false;

        let mut settings = self.settings.write().unwrap();
        settings.fg_color = self.fg_color;
        settings.bg_color = self.bg_color;
        settings.ticks_per_frame = self.ticks_per_frame;
        settings.beep_freq = self.beep_freqency;
        settings.window_has_shadow = self.window_has_shadow;
        settings.pp_enabled = self.pp_enabled;
        settings.sepia_amount = self.sepia_amount;
        settings.scale_mode = self.scale_mode;
    }
}
