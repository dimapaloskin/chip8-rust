#[derive(Debug)]
pub struct Settings {
    pub show_settings: bool,

    pub fg_color: [f32; 4],
    pub bg_color: [f32; 4],
    pub window_has_shadow: bool,

    pub ticks_per_frame: u32,

    pub beep_freq: f32,

    pub pp_enabled: bool,
    pub sepia_amount: f32,
}

impl Settings {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            show_settings: false,
            fg_color: [1.0, 0.0, 0.514, 1.0],
            bg_color: [0.024, 0.024, 0.024, 1.0],
            window_has_shadow: true,

            ticks_per_frame: 10,
            beep_freq: 220.0,

            pp_enabled: true,
            sepia_amount: 0.5,
        }
    }
}
