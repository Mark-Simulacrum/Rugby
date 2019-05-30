use crate::cpu::Cpu;
use crate::gpu::{SCREEN_HEIGHT, SCREEN_WIDTH};
use crate::joypad::{ButtonKey, DirKey};
use log::info;
use sdl2::controller::Button;
use sdl2::event::Event;
use sdl2::EventPump;
use sdl2::keyboard::{Keycode, Mod};
use sdl2::render::Canvas;
use sdl2::video::Window;
use sdl2::gfx::framerate::FPSManager;
use sdl2::GameControllerSubsystem;
use sdl2::controller::GameController;
use linefeed::{Interface, ReadResult};

const CYCLES_PER_FRAME: usize = 69905;
const WINDOW_SCALE: usize = 4;

/// The four colors of the original Game Boy screen, from lightest to darkest, in RGB.
const GAME_BOY_COLORS: [sdl2::pixels::Color; 4] = [
    sdl2::pixels::Color { r: 155, g: 188, b: 15, a: 0xFF },
    sdl2::pixels::Color { r: 139, g: 172, b: 15, a: 0xFF },
    sdl2::pixels::Color { r: 48,  g: 98,  b: 48, a: 0xFF },
    sdl2::pixels::Color { r: 15,  g: 56,  b: 15, a: 0xFF },
];

pub fn start_frontend(cpu: &mut Cpu) {
    let sdl = sdl2::init().expect("Failed to initialize SDL");

    let sdl_video = sdl.video().expect("Failed to access SDL video subsystem");
    let window = sdl_video
        .window(
            "Rugby",
            (SCREEN_WIDTH * WINDOW_SCALE) as u32,
            (SCREEN_HEIGHT * WINDOW_SCALE) as u32,
        )
        .build()
        .expect("Failed to create SDL window");
    let mut canvas = window.into_canvas().build().expect("Failed to get SDL window canvas");
    let mut sdl_events = sdl.event_pump().expect("Failed to get SDL event pump");

    let mut sdl_fps = sdl2::gfx::framerate::FPSManager::new();
    sdl_fps.set_framerate(60).expect("Failed to set SDL framerate");

    let sdl_controllers = sdl.game_controller().expect("Failed to get SDL game controllers");
    let mut controllers = vec![];

    let mut speed_multiplier: f32 = 1.0;
    let mut paused = false;
    let mut pause_next_frame = false;

    'main: loop {
        const BYTES_PER_PIXEL: usize = 4;
        let mut image = [0u8; SCREEN_WIDTH * SCREEN_HEIGHT * BYTES_PER_PIXEL];

        for tile_row in 0..SCREEN_HEIGHT {
            for tile_col in 0..SCREEN_WIDTH {
                let pixel_i = (tile_row * SCREEN_WIDTH + tile_col) * 4;
                let color_i = cpu.gpu.screen_buffer[tile_row][tile_col] as usize;
                let color = GAME_BOY_COLORS[color_i].rgb();
                image[pixel_i + 2] = color.0;
                image[pixel_i + 1] = color.1;
                image[pixel_i + 0] = color.2;
            }
        }

        let surface = sdl2::surface::Surface::from_data(
            &mut image[..],
            SCREEN_WIDTH as u32,
            SCREEN_HEIGHT as u32,
            (SCREEN_WIDTH * BYTES_PER_PIXEL) as u32,
            sdl2::pixels::PixelFormatEnum::RGB888,
        ).unwrap();
        let texture_creator = canvas.texture_creator();
        let texture = texture_creator.create_texture_from_surface(&surface).unwrap();

        canvas.copy(&texture, None, None).unwrap();
        canvas.present();

        if pause_next_frame {
            pause_next_frame = false;
            paused = true;
        }

        for event in sdl_events.poll_iter() {
            match event {
                Event::Quit { .. } => break 'main,

                Event::KeyDown { keycode: Some(keycode), keymod, repeat, .. } => {
                    let modifiers = Mod::LSHIFTMOD | Mod::RSHIFTMOD | Mod::LCTRLMOD |
                        Mod::RCTRLMOD | Mod::LALTMOD | Mod::RALTMOD | Mod::LGUIMOD |
                        Mod::RGUIMOD;
                    if !keymod.intersects(modifiers) {
                        match keycode {
                            Keycode::W if !repeat => cpu.joypad.dir_key_down(DirKey::Up),
                            Keycode::A if !repeat => cpu.joypad.dir_key_down(DirKey::Left),
                            Keycode::S if !repeat => cpu.joypad.dir_key_down(DirKey::Down),
                            Keycode::D if !repeat => cpu.joypad.dir_key_down(DirKey::Right),
                            Keycode::Return if !repeat =>
                                cpu.joypad.button_key_down(ButtonKey::Start),
                            Keycode::Tab if !repeat =>
                                cpu.joypad.button_key_down(ButtonKey::Select),
                            Keycode::K if !repeat => cpu.joypad.button_key_down(ButtonKey::A),
                            Keycode::J if !repeat => cpu.joypad.button_key_down(ButtonKey::B),
                            Keycode::P if !repeat => paused = !paused,
                            Keycode::Space => {
                                paused = false;
                                pause_next_frame = true;
                            }
                            _ => {}
                        }
                    }
                }

                Event::KeyUp { keycode: Some(keycode), keymod, .. } => {
                    let modifiers = Mod::LSHIFTMOD | Mod::RSHIFTMOD | Mod::LCTRLMOD |
                        Mod::RCTRLMOD | Mod::LALTMOD | Mod::RALTMOD | Mod::LGUIMOD |
                        Mod::RGUIMOD;
                    if !keymod.intersects(modifiers) {
                        match keycode {
                            Keycode::W => cpu.joypad.dir_key_up(DirKey::Up),
                            Keycode::A => cpu.joypad.dir_key_up(DirKey::Left),
                            Keycode::S => cpu.joypad.dir_key_up(DirKey::Down),
                            Keycode::D => cpu.joypad.dir_key_up(DirKey::Right),
                            Keycode::Return => cpu.joypad.button_key_up(ButtonKey::Start),
                            Keycode::Tab => cpu.joypad.button_key_up(ButtonKey::Select),
                            Keycode::K => cpu.joypad.button_key_up(ButtonKey::A),
                            Keycode::J => cpu.joypad.button_key_up(ButtonKey::B),
                            Keycode::RightBracket =>
                                speed_multiplier = (speed_multiplier * 2.0).min(4.0),
                            Keycode::LeftBracket =>
                                speed_multiplier = (speed_multiplier / 2.0).max(0.25),
                            _ => {}
                        }
                    }
                }

                Event::ControllerDeviceAdded { which, .. } => {
                    if let Ok(controller) = sdl_controllers.open(which) {
                        info!("Successfully opened new controller with index {}", which);
                        controllers.push(controller);
                    } else {
                        info!("Failed to open new controller with index {}", which);
                    }
                }

                Event::ControllerDeviceRemoved { which, .. } => {
                    controllers.retain(|c| c.instance_id() != which);
                    info!("Removed controller with index {}", which);
                }

                Event::ControllerButtonDown { button, .. } => {
                    match button {
                        Button::A => cpu.joypad.button_key_down(ButtonKey::A),
                        Button::X => cpu.joypad.button_key_down(ButtonKey::B),
                        Button::Start => cpu.joypad.button_key_down(ButtonKey::Start),
                        Button::Back => cpu.joypad.button_key_down(ButtonKey::Select),
                        Button::DPadLeft => cpu.joypad.dir_key_down(DirKey::Left),
                        Button::DPadRight => cpu.joypad.dir_key_down(DirKey::Right),
                        Button::DPadUp => cpu.joypad.dir_key_down(DirKey::Up),
                        Button::DPadDown => cpu.joypad.dir_key_down(DirKey::Down),
                        _ => {}
                    }
                }

                Event::ControllerButtonUp { button, .. } => {
                    match button {
                        Button::A => cpu.joypad.button_key_up(ButtonKey::A),
                        Button::X => cpu.joypad.button_key_up(ButtonKey::B),
                        Button::Start => cpu.joypad.button_key_up(ButtonKey::Start),
                        Button::Back => cpu.joypad.button_key_up(ButtonKey::Select),
                        Button::DPadLeft => cpu.joypad.dir_key_up(DirKey::Left),
                        Button::DPadRight => cpu.joypad.dir_key_up(DirKey::Right),
                        Button::DPadUp => cpu.joypad.dir_key_up(DirKey::Up),
                        Button::DPadDown => cpu.joypad.dir_key_up(DirKey::Down),
                        Button::RightShoulder =>
                            speed_multiplier = (speed_multiplier * 2.0).min(4.0),
                        Button::LeftShoulder =>
                            speed_multiplier = (speed_multiplier / 2.0).max(0.25),
                        _ => {}
                    }
                }

                _ => ()
            }
        }

        if !paused {
            cpu.step_cycles((CYCLES_PER_FRAME as f32 * speed_multiplier) as usize);
        }

        sdl_fps.delay();
    }
}

const COMMANDS: &str = "\
h:      Display commands
p:      Play emulator (Press again to pause)
w <r>:  Watch writes to a memory address 'r' (TODO)
rm:     Read memory address (TODO)
rr:     Read registers
l:      List watches (TODO)
d:      Delete watch (TODO)
s [n]:  Step forward 'n' instructions (defaults to 1)
e:      Exit debugger";

pub fn start_frontend_debug(cpu: &mut Cpu) {
    let sdl = sdl2::init().expect("Failed to initialize SDL");

    let sdl_video = sdl.video().expect("Failed to access SDL video subsystem");
    let window = sdl_video
        .window(
            "Rugby",
            (SCREEN_WIDTH * WINDOW_SCALE) as u32,
            (SCREEN_HEIGHT * WINDOW_SCALE) as u32,
        )
        .build()
        .expect("Failed to create SDL window");
    let mut canvas = window.into_canvas().build().expect("Failed to get SDL window canvas");
    let mut sdl_events = sdl.event_pump().expect("Failed to get SDL event pump");

    let mut sdl_fps = FPSManager::new();
    sdl_fps.set_framerate(60).expect("Failed to set SDL framerate");

    let sdl_controllers = sdl.game_controller().expect("Failed to get SDL game controllers");
    let mut controllers = vec![];

    let reader = Interface::new("rugby-interactive-debugger").expect("Failed to create interactive terminal");
    println!("\nWelcome to the rugby debugger! Press h for help");
    reader.set_prompt("rugby> ").expect("Failed to set terminal prompt");


    while let Some(ReadResult::Input(input)) = reader.read_line().ok() {
        let (cmd, args) = split_first_word(&input);

        match cmd {
            "h" => {
                println!("{}", COMMANDS);
            }
            "p" => {
                run_emulator(cpu, &mut canvas, &mut sdl_events, &mut sdl_fps, &sdl_controllers, &mut controllers, None)
            }
            "s" => {
                let n;
                if let Some(x) = args.parse::<usize>().ok() {
                    n = x;
                }
                else {
                    n = 1;
                }
                run_emulator(cpu, &mut canvas, &mut sdl_events, &mut sdl_fps, &sdl_controllers, &mut controllers, Some(n))
            }
            "rr" => {
                cpu.print_regs();
            }
            "e" => {
                println!("Happy debugging :)");
                break
            }
            _ => println!("unknown command: {:?}", input)
        }
    }
}

fn split_first_word(s: &str) -> (&str, &str) {
    let s = s.trim();

    match s.find(|ch: char| ch.is_whitespace()) {
        Some(pos) => (&s[..pos], s[pos..].trim_start()),
        None => (s, "")
    }
}

fn run_emulator(cpu: &mut Cpu, canvas: &mut Canvas<Window>, sdl_events: &mut EventPump, sdl_fps: &mut FPSManager, sdl_controllers: &GameControllerSubsystem, controllers: &mut Vec<GameController>, num_instrs: Option<usize>) {
    'main: loop {
        const BYTES_PER_PIXEL: usize = 4;
        let mut image = [0u8; SCREEN_WIDTH * SCREEN_HEIGHT * BYTES_PER_PIXEL];

        for tile_row in 0..SCREEN_HEIGHT {
            for tile_col in 0..SCREEN_WIDTH {
                let pixel_i = (tile_row * SCREEN_WIDTH + tile_col) * 4;
                let color_i = cpu.gpu.screen_buffer[tile_row][tile_col] as usize;
                let color = GAME_BOY_COLORS[color_i].rgb();
                image[pixel_i + 2] = color.0;
                image[pixel_i + 1] = color.1;
                image[pixel_i + 0] = color.2;
            }
        }

        let surface = sdl2::surface::Surface::from_data(
            &mut image[..],
            SCREEN_WIDTH as u32,
            SCREEN_HEIGHT as u32,
            (SCREEN_WIDTH * BYTES_PER_PIXEL) as u32,
            sdl2::pixels::PixelFormatEnum::RGB888,
        ).unwrap();
        let texture_creator = canvas.texture_creator();
        let texture = texture_creator.create_texture_from_surface(&surface).unwrap();

        canvas.copy(&texture, None, None).unwrap();
        canvas.present();

        for event in sdl_events.poll_iter() {
            match event {
                Event::Quit { .. } => break 'main,

                Event::KeyDown { keycode: Some(keycode), keymod, repeat, .. } => {
                    let modifiers = Mod::LSHIFTMOD | Mod::RSHIFTMOD | Mod::LCTRLMOD |
                        Mod::RCTRLMOD | Mod::LALTMOD | Mod::RALTMOD | Mod::LGUIMOD |
                        Mod::RGUIMOD;
                    if !keymod.intersects(modifiers) {
                        match keycode {
                            Keycode::W if !repeat => cpu.joypad.dir_key_down(DirKey::Up),
                            Keycode::A if !repeat => cpu.joypad.dir_key_down(DirKey::Left),
                            Keycode::S if !repeat => cpu.joypad.dir_key_down(DirKey::Down),
                            Keycode::D if !repeat => cpu.joypad.dir_key_down(DirKey::Right),
                            Keycode::Return if !repeat =>
                                cpu.joypad.button_key_down(ButtonKey::Start),
                            Keycode::Tab if !repeat =>
                                cpu.joypad.button_key_down(ButtonKey::Select),
                            Keycode::K if !repeat => cpu.joypad.button_key_down(ButtonKey::A),
                            Keycode::J if !repeat => cpu.joypad.button_key_down(ButtonKey::B),
                            Keycode::P if !repeat => break 'main,
                            _ => {}
                        }
                    }
                }

                Event::KeyUp { keycode: Some(keycode), keymod, .. } => {
                    let modifiers = Mod::LSHIFTMOD | Mod::RSHIFTMOD | Mod::LCTRLMOD |
                        Mod::RCTRLMOD | Mod::LALTMOD | Mod::RALTMOD | Mod::LGUIMOD |
                        Mod::RGUIMOD;
                    if !keymod.intersects(modifiers) {
                        match keycode {
                            Keycode::W => cpu.joypad.dir_key_up(DirKey::Up),
                            Keycode::A => cpu.joypad.dir_key_up(DirKey::Left),
                            Keycode::S => cpu.joypad.dir_key_up(DirKey::Down),
                            Keycode::D => cpu.joypad.dir_key_up(DirKey::Right),
                            Keycode::Return => cpu.joypad.button_key_up(ButtonKey::Start),
                            Keycode::Tab => cpu.joypad.button_key_up(ButtonKey::Select),
                            Keycode::K => cpu.joypad.button_key_up(ButtonKey::A),
                            Keycode::J => cpu.joypad.button_key_up(ButtonKey::B),
                            _ => {}
                        }
                    }
                }

                Event::ControllerDeviceAdded { which, .. } => {
                    if let Ok(controller) = sdl_controllers.open(which) {
                        info!("Successfully opened new controller with index {}", which);
                        controllers.push(controller);
                    } else {
                        info!("Failed to open new controller with index {}", which);
                    }
                }

                Event::ControllerDeviceRemoved { which, .. } => {
                    controllers.retain(|c| c.instance_id() != which);
                    info!("Removed controller with index {}", which);
                }

                Event::ControllerButtonDown { button, .. } => {
                    match button {
                        Button::A => cpu.joypad.button_key_down(ButtonKey::A),
                        Button::X => cpu.joypad.button_key_down(ButtonKey::B),
                        Button::Start => cpu.joypad.button_key_down(ButtonKey::Start),
                        Button::Back => cpu.joypad.button_key_down(ButtonKey::Select),
                        Button::DPadLeft => cpu.joypad.dir_key_down(DirKey::Left),
                        Button::DPadRight => cpu.joypad.dir_key_down(DirKey::Right),
                        Button::DPadUp => cpu.joypad.dir_key_down(DirKey::Up),
                        Button::DPadDown => cpu.joypad.dir_key_down(DirKey::Down),
                        _ => {}
                    }
                }

                Event::ControllerButtonUp { button, .. } => {
                    match button {
                        Button::A => cpu.joypad.button_key_up(ButtonKey::A),
                        Button::X => cpu.joypad.button_key_up(ButtonKey::B),
                        Button::Start => cpu.joypad.button_key_up(ButtonKey::Start),
                        Button::Back => cpu.joypad.button_key_up(ButtonKey::Select),
                        Button::DPadLeft => cpu.joypad.dir_key_up(DirKey::Left),
                        Button::DPadRight => cpu.joypad.dir_key_up(DirKey::Right),
                        Button::DPadUp => cpu.joypad.dir_key_up(DirKey::Up),
                        Button::DPadDown => cpu.joypad.dir_key_up(DirKey::Down),
                        _ => {}
                    }
                }

                _ => ()
            }
        }

        match num_instrs {
            Some(n) => {
                cpu.step_n_debug(n);
                break 'main;
            },
            None => cpu.step_cycles_debug(CYCLES_PER_FRAME),
        }

        sdl_fps.delay();
    }
}
