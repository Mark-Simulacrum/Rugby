#[macro_use]
extern crate bitflags;
extern crate clap;

use clap::{Arg, App, AppSettings, SubCommand};
use cpu::Cpu;
use std::fmt::Display;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::process::exit;

mod cartridge;
mod cpu;
mod reg_16;

fn main() {
    let app_matches = App::new("Rustboy")
        .setting(AppSettings::ArgRequiredElseHelp)
        .subcommand(SubCommand::with_name("run")
            .setting(AppSettings::ArgRequiredElseHelp)
            .arg(Arg::with_name("ROM")
                .required(true)
                .help("The game rom"))
            .arg(Arg::with_name("INSTRUCTIONS")
                .required(true)
                .help("The number of instructions to execute")))
        .subcommand(SubCommand::with_name("info")
            .setting(AppSettings::ArgRequiredElseHelp)
            .arg(Arg::with_name("ROM")
                .required(true)
                .help("The game rom")))
        .get_matches();

    match app_matches.subcommand() {
        ("run", Some(matches)) => {
            let rom_path = matches.value_of("ROM").unwrap();
            let rom = read_rom_file(rom_path);
            let instruction_count = matches.value_of("INSTRUCTIONS").unwrap().parse().unwrap();
            let mut cpu = Cpu::new(rom);
            cpu.step_n(instruction_count);
        }

        ("info", Some(matches)) => {
            let rom_path = matches.value_of("ROM").unwrap();
            let rom = read_rom_file(rom_path);
            let cart_header = check_error(
                cartridge::CartHeader::from_rom(&rom),
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
    match res {
        Ok(r) => r,
        Err(e) => {
            println!("{}: {}", message, e);
            exit(1);
        }
    }
}
