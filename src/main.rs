#[macro_use]
extern crate bitflags;
extern crate clap;
#[macro_use]
extern crate glium;
extern crate glium_sdl2;
extern crate rand;
extern crate sdl2;

use clap::{Arg, App, AppSettings, SubCommand};
use cpu::Cpu;
use cart::Cart;
use std::fmt::Display;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::process::exit;

mod cart;
mod cart_header;
mod cpu;
mod reg_16;

fn main() {
    let app_matches = App::new("Rustboy")
        .setting(AppSettings::ArgRequiredElseHelp)
        .subcommand(SubCommand::with_name("run")
            .arg(Arg::with_name("ROM")
                .required(true)
                .help("The game rom"))
            .arg(Arg::with_name("INSTRUCTIONS")
                .required(true)
                .help("The number of instructions to execute")))
        .subcommand(SubCommand::with_name("info")
            .arg(Arg::with_name("ROM")
                .required(true)
                .help("The game rom")))
        .subcommand(SubCommand::with_name("window"))
        .get_matches();

    match app_matches.subcommand() {
        ("run", Some(matches)) => {
            let rom_path = matches.value_of("ROM").unwrap();
            let rom = read_rom_file(rom_path);
            let instruction_count = check_error(
                matches.value_of("INSTRUCTIONS").unwrap().parse(),
                "Couldn't parse instruction count",
            );
            let cart_header = check_error(
                cart_header::CartHeader::from_rom(&rom),
                "Couldn't parse cartridge header",
            );
            let cart = Cart::new(rom, &cart_header);
            let mut cpu = Cpu::new(cart);
            cpu.step_n(instruction_count);
        }

        ("info", Some(matches)) => {
            let rom_path = matches.value_of("ROM").unwrap();
            let rom = read_rom_file(rom_path);
            let cart_header = check_error(
                cart_header::CartHeader::from_rom(&rom),
                "Couldn't parse cartridge header",
            );
            println!("{:#?}", cart_header);
        }

        ("window", Some(_)) => {
            open_window();
        }

        _ => unreachable!(),
    }
}

fn read_rom_file<P: AsRef<Path>>(path: P) -> Box<[u8]> {
    let mut file = check_error(File::open(path), "Couldn't open rom file");
    let mut file_buf = Vec::new();
    check_error(file.read_to_end(&mut file_buf), "Couldn't read rom");
    file_buf.into_boxed_slice()
}

fn check_error<T, E: Display>(res: Result<T, E>, message: &'static str) -> T {
    res.unwrap_or_else(|e| {
        println!("{}: {}", message, e);
        exit(1);
    })
}

fn open_window() {
    use glium_sdl2::DisplayBuild;

    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();

    let display = video_subsystem.window("My window", 800, 600)
        .resizable()
        .build_glium()
        .unwrap();

    let mut running = true;
    let mut event_pump = sdl_context.event_pump().unwrap();

    while running {
        let mut target = display.draw();
        // do drawing here...
        target.finish().unwrap();

        // Event loop: polls for events sent to all windows

        for event in event_pump.poll_iter() {
            use sdl2::event::Event;

            match event {
                Event::Quit { .. } => {
                    running = false;
                },
                _ => ()
            }
        }
    }
}
