use std::env;
use std::sync::Mutex;
use std::time::Duration;
use std::sync::mpsc::{Receiver, Sender};

use egui::{Color32, ScrollArea, TextEdit};
use egui_backend::{
    gl, 
    sdl2,
    DpiScaling, 
    ShaderVersion,
};
use egui_backend::egui;
use egui_backend::sdl2::event::Event;
use egui_backend::egui::FullOutput;
use egui_backend::sdl2::video::{GLProfile, SwapInterval};

use std::time::Instant;
use egui_sdl2_gl as egui_backend;

use once_cell::sync::OnceCell;

// The log message sender. This allows for multiple clients to send messages
// across threads to the consumer within the main UI loop.
pub static MSG_TX: OnceCell<Sender<String>> = OnceCell::new();

const SCREEN_WIDTH: u32 = 800;
const SCREEN_HEIGHT: u32 = 600;

/// Opens the lovely debug console.
/// This is currently a copy-paste from a minimal egui-sdl2 example project. I'll expand it later.
pub fn open(rx: Receiver<String>) {
    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();
    let gl_attr = video_subsystem.gl_attr();
    gl_attr.set_context_profile(GLProfile::Core);
    // On linux, OpenGL ES Mesa driver 22.0.0+ can be used like so:
    // gl_attr.set_context_profile(GLProfile::GLES);

    gl_attr.set_double_buffer(true);
    gl_attr.set_multisample_samples(4);

    let mut window = video_subsystem
        .window(
            "Lovely: Debug Console",
            SCREEN_WIDTH,
            SCREEN_HEIGHT,
        )
        .opengl()
        .build()
        .unwrap();

    // Create a window context
    let _ctx = window.gl_create_context().unwrap();
    // Init egui stuff
    let shader_ver = ShaderVersion::Default;
    // On linux use GLES SL 100+, like so:
    // let shader_ver = ShaderVersion::Adaptive;
    let (mut painter, mut egui_state) =
        egui_backend::with_sdl2(&window, shader_ver, DpiScaling::Default);
    let egui_ctx = egui::Context::default();
    let mut event_pump = sdl_context.event_pump().unwrap();

    let mut test_str: String =
        "A text box to write in. Cut, copy, paste commands are available.\n".to_owned();

    let enable_vsync = true;
    let mut quit = false;

    if enable_vsync {
        if let Err(error) = window.subsystem().gl_set_swap_interval(SwapInterval::VSync) {
            println!(
                "Failed to gl_set_swap_interval(SwapInterval::VSync): {}",
                error
            );
        }
    } else if let Err(error) = window
        .subsystem()
        .gl_set_swap_interval(SwapInterval::Immediate)
    {
        println!(
            "Failed to gl_set_swap_interval(SwapInterval::Immediate): {}",
            error
        );
    }

    let start_time = Instant::now();

    'running: loop {
        unsafe {
            // Clear the screen to green
            gl::ClearColor(0.3, 0.6, 0.3, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);
        }

        egui_state.input.time = Some(start_time.elapsed().as_secs_f64());
        egui_ctx.begin_frame(egui_state.input.take());

        if let Ok(read) = rx.recv_timeout(Duration::from_millis(20)) {
            test_str = format!("{test_str}\n{read}");
        }

        egui::CentralPanel::default().show(&egui_ctx, |ui| {
            if ui.button("Quit").clicked() {
                quit = true;
            }
            ui.separator();

            ScrollArea::vertical().stick_to_bottom(true).show(ui, |ui| {
                let mut text = test_str.as_str();
                let text_edit = TextEdit::multiline(&mut text)
                    .interactive(true)
                    .cursor_at_end(true)
                    .text_color(Color32::WHITE);
                ui.add_sized(ui.available_size(), text_edit);
            })
        });

        let FullOutput {
            platform_output,
            textures_delta,
            shapes,
            pixels_per_point,
            viewport_output,
        } = egui_ctx.end_frame();

        // Process ouput
        egui_state.process_output(&window, &platform_output);

        // For default dpi scaling only, Update window when the size of resized window is very small (to avoid egui::CentralPanel distortions).
        // if egui_ctx.used_size() != painter.screen_rect.size() {
        //     println!("resized.");
        //     let _size = egui_ctx.used_size();
        //     let (w, h) = (_size.x as u32, _size.y as u32);
        //     window.set_size(w, h).unwrap();
        // }

        let paint_jobs = egui_ctx.tessellate(shapes, pixels_per_point);
        painter.paint_jobs(None, textures_delta, paint_jobs);
        window.gl_swap_window();

        let repaint_after = viewport_output
            .get(&egui::ViewportId::ROOT)
            .expect("Missing ViewportId::ROOT")
            .repaint_delay;

        if !repaint_after.is_zero() {
            if let Some(event) = event_pump.wait_event_timeout(5) {
                match event {
                    Event::Quit { .. } => break 'running,
                    _ => {
                        // Process input event
                        egui_state.process_input(&window, event, &mut painter);
                    }
                }
            }
        } else {
            for event in event_pump.poll_iter() {
                match event {
                    Event::Quit { .. } => break 'running,
                    _ => {
                        // Process input event
                        egui_state.process_input(&window, event, &mut painter);
                    }
                }
            }
        }

        if quit {
            break;
        }
    }
}

