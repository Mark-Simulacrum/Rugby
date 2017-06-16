#[macro_use]
extern crate bitflags;
extern crate clap;
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
use std::time::{Instant, Duration};
use std::thread;

mod cart;
mod cart_header;
mod cpu;
mod reg_16;

const CYCLES_PER_FRAME: usize = 69905;
const ITERATIONS: usize = 1000;

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
        .get_matches();

    match app_matches.subcommand() {
        ("run", Some(matches)) => {
            let rom_path = matches.value_of("ROM").unwrap();
            let rom = read_rom_file(rom_path);
            let instruction_count: usize = check_error(
                matches.value_of("INSTRUCTIONS").unwrap().parse(),
                "Couldn't parse instruction count",
            );
            let cart_header = check_error(
                cart_header::CartHeader::from_rom(&rom),
                "Couldn't parse cartridge header",
            );
            let cart = Cart::new(rom, &cart_header);
            let mut cpu = Cpu::new(cart);

            let sdl = check_error(sdl2::init(), "Couldn't initialize SDL2");
            let video_subsystem = check_error(sdl.video(), "Couldn't initialize SDL2 video subsystem");

            let window = check_error(
                video_subsystem
                    .window("My window", 1024, 1024)
                    .resizable()
                    .build(),
                "Couldn't initialize SDL2 window",
            );

            let mut renderer = check_error(window.renderer().build(), "Couldn't initialize SDL2 renderer");
            let mut event_pump = check_error(sdl.event_pump(), "Couldn't initialize SDL2 event pump");
            let frame_duration = Duration::new(0, 16666667); // Duration of a frame at 60 FPS
            let mut frames_too_slow = 0;

            for _ in 0..ITERATIONS {
                let frame_start_time = Instant::now();

                // TODO(solson): Heavily re-write all of the below code. 'tis the product of a
                // horrific late-night hacking session.
                const BYTES_PER_PIXEL: usize = 4;
                const TILE_SIDE: usize = 8;
                const NUM_TILES: usize = 32;
                const SIDE: usize = TILE_SIDE * NUM_TILES;

                let mut image = [0u8; SIDE * SIDE * BYTES_PER_PIXEL];
                {
                    let bg_map = &cpu.video_ram[0x1800..0x1C00];
                    for tile_row in 0..NUM_TILES {
                        for tile_col in 0..NUM_TILES {
                            let tile_i = bg_map[tile_row * NUM_TILES + tile_col] as usize;
                            let tile = &cpu.video_ram[tile_i * 16..(tile_i * 16 + 16)];
                            for row in 0..TILE_SIDE {
                                for col in 0..TILE_SIDE {
                                    let upper_bit = (tile[row * 2 + 0] >> (TILE_SIDE - col - 1)) & 1;
                                    let lower_bit = (tile[row * 2 + 1] >> (TILE_SIDE - col - 1)) & 1;
                                    let tile_color = upper_bit << 1 | lower_bit;
                                    let image_i = (
                                        tile_row * SIDE * TILE_SIDE +
                                        tile_col * TILE_SIDE +
                                        row * SIDE +
                                        col
                                    ) * BYTES_PER_PIXEL;
                                    image[image_i + 2] = GAMEBOY_COLORS[tile_color as usize].rgb().0;
                                    image[image_i + 1] = GAMEBOY_COLORS[tile_color as usize].rgb().1;
                                    image[image_i + 0] = GAMEBOY_COLORS[tile_color as usize].rgb().2;
                                }
                            }
                        }
                    }
                }

                let surface = sdl2::surface::Surface::from_data(
                    &mut image[..],
                    SIDE as u32,
                    SIDE as u32,
                    (SIDE * BYTES_PER_PIXEL) as u32,
                    sdl2::pixels::PixelFormatEnum::RGB888,
                ).unwrap();
                let texture = renderer.create_texture_from_surface(&surface).unwrap();

                renderer.copy(&texture, None, None).unwrap();
                renderer.present();

                for event in event_pump.poll_iter() {
                    use sdl2::event::Event;
                    match event {
                        Event::Quit { .. } => break,// 'main,
                        _ => ()
                    }
                }

                cpu.step_n(CYCLES_PER_FRAME);

                //println!("Desired Frame duration: {} {}", frame_duration.as_secs(), frame_duration.subsec_nanos());
                let frame_done_time = Instant::now();
                let sleep_duration = frame_done_time - frame_start_time;
//                println!("Actual Frame duration: {}", sleep_duration.subsec_nanos());
//                println!("Desired Frame duratio; 16666667");
                if sleep_duration.subsec_nanos() < 16666667 {
                    //println!("Sleeping for duration: {} {}", duration.as_secs(), duration.subsec_nanos());
                    //thread::sleep(duration);
                }
                else {
                    //println!("Failed to calculate frame fast enough!");
                    frames_too_slow += 1;
                }
            }
            println!("Frames too slow: {}", frames_too_slow);
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

/// The four colors of the original Game Boy screen, from lightest to darkest, in RGB.
const GAMEBOY_COLORS: [sdl2::pixels::Color; 4] = [
    sdl2::pixels::Color::RGB(155, 188, 15),
    sdl2::pixels::Color::RGB(139, 172, 15),
    sdl2::pixels::Color::RGB(48, 98, 48),
    sdl2::pixels::Color::RGB(15, 56, 15),
];
