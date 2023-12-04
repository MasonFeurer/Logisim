use logisim_common as logisim;

use logisim::glam::vec2;
use logisim::input::{InputEvent, InputState, PtrButton};
use logisim::Perf;
use logisim::{app::App, Rect};

use std::time::{Duration, SystemTime};

use winit::event::{ElementState, Event, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::EventLoopBuilder;
use winit::keyboard::{Key, NamedKey};
use winit::window::Window;

fn main() {
    env_logger::init();
    let event_loop = EventLoopBuilder::new().build().unwrap();
    let window = winit::window::WindowBuilder::new()
        .with_title("Logisim")
        .build(&event_loop)
        .unwrap();

    let mut state = State {
        app: App::default(),
        input: InputState::default(),
        window,
        frame_timer: Timer::new(60),
        perf: Perf::default(),
    };
    state.app.init();

    _ = event_loop.run(move |event, event_loop| {
        let mut exit = false;
        on_event(&mut state, event, &mut exit);
        if exit {
            event_loop.exit();
        }
    });
}

#[derive(Clone, Debug)]
struct Timer {
    per_second: u32,
    last_reset: SystemTime,
}
impl Timer {
    fn new(per_second: u32) -> Self {
        Self {
            per_second,
            last_reset: SystemTime::now(),
        }
    }

    fn until_ready(&self) -> Duration {
        let ready_time = self.last_reset + Duration::from_millis(1000 / self.per_second as u64);
        // ready_time might not be in the future
        ready_time
            .duration_since(SystemTime::now())
            .unwrap_or(Duration::ZERO)
    }

    fn ready(&self) -> bool {
        self.until_ready().as_millis() == 0
    }

    fn reset(&mut self) {
        self.last_reset = SystemTime::now();
    }
}

struct State {
    app: App,
    window: Window,
    frame_timer: Timer,
    input: InputState,
    perf: Perf,
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
            if ctx.frame_timer.ready() {
                ctx.frame_timer.reset();
                _ = ctx.app.draw_frame(
                    &mut ctx.input,
                    content_rect,
                    &mut Default::default(),
                    &mut ctx.perf,
                );
                ctx.perf.end_frame();
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
                ctx.input.on_event(InputEvent::Click(pos, button));
                ctx.input.on_event(InputEvent::Press(pos, button));
            } else {
                ctx.input.on_event(InputEvent::Release(pos, button));
            }
        }
        WindowEvent::MouseWheel { delta, .. } => match delta {
            MouseScrollDelta::LineDelta(x, y) => ctx.input.on_event(InputEvent::Scroll(vec2(x, y))),
            MouseScrollDelta::PixelDelta(pos) => ctx
                .input
                .on_event(InputEvent::Scroll(vec2(pos.x as f32, pos.y as f32))),
        },
        WindowEvent::TouchpadMagnify { delta, .. } => {
            ctx.input.on_event(InputEvent::Zoom(delta as f32))
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