use clap::Parser;

use std::{
    error::Error,
    sync::{Arc, RwLock},
};
use winit::event_loop::{ControlFlow, EventLoop};

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    #[arg(index = 1, help = "ROM")]
    rom_path: Option<String>,
}

use chip8::app::App;
use chip8::settings::Settings;
use chip8::vm::Vm;

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let settings = Arc::new(RwLock::new(Settings::new()));

    let mut vm = Vm::new();
    if let Some(rom_path) = &args.rom_path {
        vm.load_rom(rom_path.clone())?;
    }

    let mut app = App::new(vm, settings);
    event_loop.run_app(&mut app)?;

    Ok(())
}
