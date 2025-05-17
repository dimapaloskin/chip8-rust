use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rand::seq::IndexedRandom;
use std::{
    f32,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
};

pub struct Beeper {
    stream: Option<cpal::Stream>,
    phase_inc: Arc<AtomicU32>,
    gain: Arc<AtomicU32>,
    sample_rate: f32,
    playing: Arc<AtomicBool>,
    reset_phase: Arc<AtomicBool>,
    scale_mode: bool,
}

const MAJOR_SCALE: [f32; 15] = [
    130.81, // C3
    146.83, // D3
    164.81, // E3
    174.61, // F3
    196.0,  // G3
    220.0,  // A3
    246.94, // B3
    261.63, // C4
    293.66, // D4
    329.63, // E4
    349.23, // F4
    392.00, // G4
    440.00, // A4
    493.88, // B4
    523.25, // C5
];

impl Beeper {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            stream: None,
            phase_inc: Arc::new(AtomicU32::new(0)),
            gain: Arc::new(AtomicU32::new(1)),
            sample_rate: 0.0,
            playing: Arc::new(AtomicBool::new(false)),
            reset_phase: Arc::new(AtomicBool::new(false)),
            scale_mode: false,
        }
    }

    pub fn init_stream(&mut self) -> Option<()> {
        let host = cpal::default_host();
        let device = host.default_output_device()?;
        let config = device.default_output_config().ok()?;
        let sample_rate = config.sample_rate().0 as f32;
        let channels = config.channels() as usize;
        self.sample_rate = sample_rate;

        let phase_inc_clone = Arc::clone(&self.phase_inc);
        let gain_clone = Arc::clone(&self.gain);
        let playing_clone = Arc::clone(&self.playing);
        let reset_phase_clone = Arc::clone(&self.reset_phase);

        let new_phase_inc = f32::consts::TAU * 220.0 / sample_rate;
        self.phase_inc
            .store(f32::to_bits(new_phase_inc), Ordering::Relaxed);
        self.gain.store(f32::to_bits(1.0), Ordering::Relaxed);

        let fade_speed = 1.0 / (sample_rate * 0.05);
        let rise_speed = 1.0 / (sample_rate * 0.0001);

        let mut phase: f32 = 0.0;
        let mut fade: f32 = 0.0;

        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => device
                .build_output_stream(
                    &config.into(),
                    move |data: &mut [f32], _| {
                        for frame in data.chunks_mut(channels) {
                            let phase_inc = f32::from_bits(phase_inc_clone.load(Ordering::Relaxed));
                            let gain = f32::from_bits(gain_clone.load(Ordering::Relaxed));
                            let playing = playing_clone.load(Ordering::Relaxed);
                            let reset_phase = reset_phase_clone.load(Ordering::Relaxed);

                            for sample in frame.iter_mut() {
                                if reset_phase {
                                    if fade == 0.0 {
                                        phase = 0.0;
                                    }

                                    reset_phase_clone.store(false, Ordering::Relaxed);
                                }

                                if !playing && fade > 0.0 {
                                    fade -= fade_speed;
                                    if fade < 0.0 {
                                        fade = 0.0;
                                    }
                                }

                                if playing && fade == 0.0 {
                                    fade = 1.0;
                                } else if playing && fade != 1.0 {
                                    fade += rise_speed;
                                    if fade > 1.0 {
                                        fade = 1.0;
                                    }
                                }

                                let s = phase.sin() * gain * fade;
                                *sample = s;
                            }

                            phase += phase_inc;
                            if phase > f32::consts::TAU {
                                phase -= f32::consts::TAU;
                            }
                        }
                    },
                    |err| eprintln!("an error occurred on stream: {}", err),
                    None,
                )
                .ok()?,
            _ => return None,
        };

        stream.play().ok()?;
        self.stream = Some(stream);

        Some(())
    }

    pub fn play(&mut self) {
        let is_playing = self.playing.load(Ordering::Relaxed);
        if is_playing {
            return;
        }

        if self.scale_mode {
            let mut rng = rand::rng();
            let freq = MAJOR_SCALE.choose(&mut rng);
            if let Some(freq) = freq {
                let new_phase_inc = f32::consts::TAU * freq / self.sample_rate;
                self.phase_inc
                    .store(f32::to_bits(new_phase_inc), Ordering::Relaxed);
                self.set_freq(*freq);
            }
        }

        self.reset_phase.store(true, Ordering::Relaxed);
        self.playing.store(true, Ordering::Relaxed);
    }

    pub fn pause(&mut self) {
        let is_playing = self.playing.load(Ordering::Relaxed);
        if !is_playing {
            return;
        }

        self.playing.store(false, Ordering::Relaxed);
    }

    pub fn set_freq(&mut self, freq: f32) {
        let new_phase_inc = f32::consts::TAU * freq / self.sample_rate;
        self.phase_inc
            .store(f32::to_bits(new_phase_inc), Ordering::Relaxed);
    }

    pub fn set_scale_mode(&mut self, scale_mode: bool) {
        self.scale_mode = scale_mode;
    }
}
