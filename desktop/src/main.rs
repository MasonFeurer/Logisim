#![windows_subsystem = "windows"]

use logisim_common as logisim;

use logisim::glam::{vec2, Vec2};
use logisim::input::{InputEvent, InputState, PtrButton, TextInputState};
use logisim::{app::App, Rect};

use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use winit::event::{ElementState, Event, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::EventLoopBuilder;
use winit::keyboard::{Key, NamedKey};
use winit::window::Window;

struct SaveDirs {
    settings: PathBuf,
    library: PathBuf,
    scene: PathBuf,
}
impl SaveDirs {
    fn new() -> Self {
        let dirs = directories::ProjectDirs::from("com", "", "Logisim").unwrap();
        let dir = dirs.data_dir();
        _ = std::fs::create_dir(dir);
        Self {
            settings: dir.join("settings.data"),
            library: dir.join("library.data"),
            scene: dir.join("scene.data"),
        }
    }
}

fn main() {
    env_logger::init();
    let event_loop = EventLoopBuilder::new().build().unwrap();
    let window = winit::window::WindowBuilder::new()
        .with_title("Logisim")
        .build(&event_loop)
        .unwrap();

    let mut state = State {
        app: App::new(),
        input: InputState::default(),
        window,
        last_frame_time: SystemTime::now(),
        clipboard: arboard::Clipboard::new()
            .map_err(|err| log::warn!("Failed to init system clipboard: {err:?}"))
            .ok(),
        text_input: None,
        save_dirs: SaveDirs::new(),
        ptr_press: None,

        frame_count: 0,
        last_fps_update: SystemTime::now(),
        fps: 0,
    };

    if let Ok(bytes) = std::fs::read(&state.save_dirs.settings) {
        match bincode::deserialize(&bytes) {
            Ok(settings) => state.app.settings = settings,
            Err(err) => log::warn!("Failed to parse settings: {err:?}"),
        }
    }
    if let Ok(bytes) = std::fs::read(&state.save_dirs.library) {
        match bincode::deserialize(&bytes) {
            Ok(library) => state.app.library = library,
            Err(err) => log::warn!("Failed to parse library: {err:?}"),
        }
    }
    if let Ok(bytes) = std::fs::read(&state.save_dirs.scene) {
        match bincode::deserialize(&bytes) {
            Ok(scene) => state.app.scenes = scene,
            Err(err) => log::warn!("Failed to parse scene: {err:?}"),
        }
    }

    _ = event_loop.run(move |event, event_loop| {
        let mut exit = false;
        on_event(&mut state, event, &mut exit);
        if exit {
            event_loop.exit();
        }
    });
}

struct State {
    app: App,
    window: Window,
    input: InputState,
    last_frame_time: SystemTime,
    clipboard: Option<arboard::Clipboard>,
    text_input: Option<TextInputState>,
    save_dirs: SaveDirs,
    ptr_press: Option<(PtrButton, Vec2, SystemTime)>,

    frame_count: u32,
    last_fps_update: SystemTime,
    fps: u32,
}

fn on_event(state: &mut State, event: Event<()>, exit: &mut bool) {
    match event {
        Event::Resumed => {
            let size = <[u32; 2]>::from(state.window.inner_size()).into();
            pollster::block_on(state.app.resume(&state.window, size));
            state.app.update_size(size);
            state.window.request_redraw();
        }
        Event::Suspended => println!("suspended"),
        Event::WindowEvent { event, .. } => on_window_event(state, event, exit),
        Event::LoopExiting => {
            let settings = bincode::serialize(&state.app.settings).unwrap();
            match std::fs::write(&state.save_dirs.settings, settings) {
                Ok(_) => log::info!("Saved settings to {:?}", state.save_dirs.settings),
                Err(err) => log::warn!(
                    "Failed to save settings to {:?} : {err:?}",
                    state.save_dirs.settings
                ),
            }

            let library = bincode::serialize(&state.app.library).unwrap();
            match std::fs::write(&state.save_dirs.library, library) {
                Ok(_) => log::info!("Saved library to {:?}", state.save_dirs.library),
                Err(err) => log::warn!(
                    "Failed to save library to {:?} : {err:?}",
                    state.save_dirs.library
                ),
            }

            let scene = bincode::serialize(&state.app.scenes).unwrap();
            match std::fs::write(&state.save_dirs.scene, scene) {
                Ok(_) => log::info!("Saved scene to {:?}", state.save_dirs.scene),
                Err(err) => log::warn!(
                    "Failed to save scene to {:?} : {err:?}",
                    state.save_dirs.scene
                ),
            }
        }
        _ => {}
    }
}

fn on_window_event(ctx: &mut State, event: WindowEvent, exit: &mut bool) {
    match event {
        WindowEvent::RedrawRequested => {
            let content_size = vec2(
                ctx.window.inner_size().width as f32,
                ctx.window.inner_size().height as f32,
            );
            let content_rect = Rect::from_min_size(vec2(0.0, 0.0), content_size);

            let redraw = SystemTime::now()
                .duration_since(ctx.last_frame_time)
                .unwrap_or(Duration::ZERO)
                .as_millis()
                > (1000 / 60);

            if redraw {
                // Update FPS
                {
                    ctx.frame_count += 1;
                    if SystemTime::now()
                        .duration_since(ctx.last_fps_update)
                        .unwrap()
                        .as_secs()
                        >= 1
                    {
                        ctx.last_fps_update = SystemTime::now();
                        ctx.fps = ctx.frame_count;
                        ctx.frame_count = 0;
                    }
                }

                ctx.input.millis = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .as_ref()
                    .map(Duration::as_millis)
                    .unwrap_or(0);
                ctx.last_frame_time = SystemTime::now();
                if let Err(err) = ctx.app.draw_frame(
                    &mut ctx.input,
                    content_rect,
                    ctx.fps,
                    &mut Default::default(),
                ) {
                    log::warn!("Failed to draw frame: {err:?}");
                }
                ctx.input.update();
            }
            ctx.window.request_redraw();
        }
        WindowEvent::Resized(_size) => {
            let size = <[u32; 2]>::from(ctx.window.inner_size()).into();
            ctx.app.update_size(size);
            ctx.window.request_redraw();
        }
        WindowEvent::CloseRequested => *exit = true,
        WindowEvent::CursorMoved { position, .. } => {
            let pos: [f32; 2] = position.into();
            ctx.input.on_event(InputEvent::Hover(pos.into()));
        }
        WindowEvent::MouseInput { state, button, .. } => {
            let button = match button {
                MouseButton::Left => PtrButton::LEFT,
                MouseButton::Middle => PtrButton::MIDDLE,
                MouseButton::Right => PtrButton::RIGHT,
                MouseButton::Back => PtrButton::BACK,
                MouseButton::Forward => PtrButton::FORWARD,
                MouseButton::Other(id) => PtrButton::new(id),
            };
            let pos = ctx.input.ptr_pos();
            if state == ElementState::Pressed {
                ctx.input.on_event(InputEvent::Press(pos, button));
                ctx.ptr_press = Some((button, pos, SystemTime::now()));
            } else {
                if let Some((press_button, press_pos, instant)) = ctx.ptr_press {
                    // if we've pressed the same button at a close position within the past 2 seconds, register a click.
                    if press_button == button
                        && (pos - press_pos).abs().length_squared() < 5.0
                        && SystemTime::now()
                            .duration_since(instant)
                            .map(|d| d.as_secs() < 2)
                            .ok()
                            == Some(true)
                    {
                        ctx.input.on_event(InputEvent::Click(press_pos, button));
                    }
                    ctx.ptr_press = None;
                }
                ctx.input.on_event(InputEvent::Release(button));
            }
        }
        WindowEvent::MouseWheel { delta, .. } => match delta {
            MouseScrollDelta::LineDelta(x, y) => ctx.input.on_event(InputEvent::Scroll(vec2(x, y))),
            MouseScrollDelta::PixelDelta(pos) => ctx
                .input
                .on_event(InputEvent::Scroll(vec2(pos.x as f32, pos.y as f32))),
        },
        WindowEvent::TouchpadMagnify { delta, .. } => {
            if !ctx.input.ptr_gone() {
                ctx.input
                    .on_event(InputEvent::Zoom(ctx.input.ptr_pos(), delta as f32))
            }
        }
        WindowEvent::KeyboardInput { event, .. } => {
            if matches!(event.state, ElementState::Pressed) {
                match event.logical_key {
                    Key::Named(key) => {
                        if let Some(key) = translate_key(key) {
                            ctx.input.on_event(InputEvent::PressKey(key));
                        }
                    }
                    Key::Character(ref smol_str) => {
                        if smol_str.as_str() == "v" && ctx.input.modifiers().cmd {
                            // Paste command
                            if let Some(text) =
                                ctx.clipboard.as_mut().and_then(|cb| cb.get_text().ok())
                            {
                                ctx.input.on_event(InputEvent::Paste(text));
                            }
                            return;
                        }
                        if smol_str.as_str() == "c" && ctx.input.modifiers().cmd {
                            // Copy command (For now we copy the entire active text field)
                            if let Some(input) = &ctx.text_input {
                                if let Some(clipboard) = &mut ctx.clipboard {
                                    _ = clipboard.set_text(input.text.clone());
                                }
                            }
                            return;
                        }
                        for ch in smol_str.as_str().chars() {
                            ctx.input.on_event(InputEvent::Type(ch))
                        }
                    }
                    _ => {}
                }
            }
            if matches!(event.state, ElementState::Released) {
                if let Key::Named(key) = event.logical_key {
                    if let Some(key) = translate_key(key) {
                        ctx.input.on_event(InputEvent::ReleaseKey(key));
                    }
                }
            }
        }
        _ => {}
    }
}

fn translate_key(key: NamedKey) -> Option<logisim::input::Key> {
    Some(match key {
        NamedKey::Shift => logisim::input::Key::Shift,
        NamedKey::Control => logisim::input::Key::Command,
        NamedKey::Alt => logisim::input::Key::Option,

        NamedKey::Backspace => logisim::input::Key::Backspace,
        NamedKey::Enter => logisim::input::Key::Enter,
        NamedKey::Escape => logisim::input::Key::Esc,
        NamedKey::ArrowLeft => logisim::input::Key::Left,
        NamedKey::ArrowRight => logisim::input::Key::Right,
        NamedKey::ArrowUp => logisim::input::Key::Up,
        NamedKey::ArrowDown => logisim::input::Key::Down,
        NamedKey::Tab => logisim::input::Key::Tab,
        NamedKey::Space => logisim::input::Key::Space,
        NamedKey::Delete => logisim::input::Key::Delete,
        NamedKey::Insert => logisim::input::Key::Insert,
        NamedKey::Home => logisim::input::Key::Home,
        NamedKey::End => logisim::input::Key::End,
        NamedKey::PageUp => logisim::input::Key::PageUp,
        NamedKey::PageDown => logisim::input::Key::PageDown,
        NamedKey::F1 => logisim::input::Key::F1,
        NamedKey::F2 => logisim::input::Key::F2,
        NamedKey::F3 => logisim::input::Key::F3,
        NamedKey::F4 => logisim::input::Key::F4,
        NamedKey::F5 => logisim::input::Key::F5,
        NamedKey::F6 => logisim::input::Key::F6,
        NamedKey::F7 => logisim::input::Key::F7,
        NamedKey::F8 => logisim::input::Key::F8,
        NamedKey::F9 => logisim::input::Key::F9,
        NamedKey::F10 => logisim::input::Key::F10,
        NamedKey::F11 => logisim::input::Key::F11,
        NamedKey::F12 => logisim::input::Key::F12,
        _ => return None,
    })
}
