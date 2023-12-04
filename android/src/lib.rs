use logisim_common as logisim;

use logisim::app::App;
use logisim::glam::{uvec2, vec2, UVec2, Vec2};
use logisim::input::{InputEvent as LogisimInputEvent, InputState, PtrButton, TextInputState};
use logisim::Perf;

use android_activity::{
    input::{InputEvent, KeyAction, KeyEvent, KeyMapChar, MotionAction},
    AndroidApp, InputStatus, MainEvent, PollEvent,
};
use ndk::native_window::NativeWindow;
use raw_window_handle::{
    AndroidDisplayHandle, HasRawDisplayHandle, HasRawWindowHandle, RawDisplayHandle,
    RawWindowHandle,
};
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::{Duration, SystemTime};

static TOP_DISPLAY_INSET: AtomicI32 = AtomicI32::new(0);
static RIGHT_DISPLAY_INSET: AtomicI32 = AtomicI32::new(0);
static BOTTOM_DISPLAY_INSET: AtomicI32 = AtomicI32::new(0);
static LEFT_DISPLAY_INSET: AtomicI32 = AtomicI32::new(0);

#[allow(dead_code)]
#[allow(non_snake_case)]
#[no_mangle]
/// Callback from Java code to update display insets (cutouts).
pub extern "C" fn Java_com_logisim_android_MainActivity_onDisplayInsets(
    env: jni::JNIEnv,
    _class: jni::objects::JObject,
    cutouts: jni::sys::jarray,
) {
    use jni::objects::{JObject, JPrimitiveArray};

    let mut array: [i32; 4] = [0; 4];
    unsafe {
        let j_obj = JObject::from_raw(cutouts);
        let j_arr = JPrimitiveArray::from(j_obj);
        env.get_int_array_region(j_arr, 0, array.as_mut()).unwrap();
    }

    TOP_DISPLAY_INSET.store(array[0], Ordering::Relaxed);
    RIGHT_DISPLAY_INSET.store(array[1], Ordering::Relaxed);
    BOTTOM_DISPLAY_INSET.store(array[2], Ordering::Relaxed);
    LEFT_DISPLAY_INSET.store(array[3], Ordering::Relaxed);
    log::info!("Settings DISPLAY_INSETS to {array:?}");
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

#[derive(Clone)]
struct Window {
    inner: NativeWindow,
}
impl Window {
    fn new(inner: NativeWindow) -> Self {
        Self { inner }
    }

    fn size(&self) -> UVec2 {
        uvec2(self.inner.width() as u32, self.inner.height() as u32)
    }
}
unsafe impl HasRawWindowHandle for Window {
    fn raw_window_handle(&self) -> RawWindowHandle {
        HasRawWindowHandle::raw_window_handle(&self.inner)
    }
}
unsafe impl HasRawDisplayHandle for Window {
    fn raw_display_handle(&self) -> RawDisplayHandle {
        RawDisplayHandle::Android(AndroidDisplayHandle::empty())
    }
}

#[derive(Debug)]
struct TouchTranslater {
    device_id: Option<i32>,
    ignore_release: bool,
    last_press_time: SystemTime,
    last_pos: Vec2,
    press_pos: Option<Vec2>,
    holding: bool,
}
impl Default for TouchTranslater {
    fn default() -> Self {
        Self {
            device_id: None,
            ignore_release: false,
            last_press_time: SystemTime::UNIX_EPOCH,
            last_pos: Vec2::ZERO,
            press_pos: None,
            holding: false,
        }
    }
}
impl TouchTranslater {
    fn update(&mut self, mut out: impl FnMut(LogisimInputEvent)) {
        if self.holding
            && SystemTime::now()
                .duration_since(self.last_press_time)
                .unwrap_or(Duration::ZERO)
                .as_millis()
                > 1000
        {
            out(LogisimInputEvent::Click(self.last_pos, PtrButton::RIGHT));
            self.ignore_release = true;
            self.holding = false;
        }
    }

    fn phase_start(&mut self, id: i32, pos: Vec2, mut out: impl FnMut(LogisimInputEvent)) {
        self.device_id = Some(id);
        out(LogisimInputEvent::Hover(pos));
        out(LogisimInputEvent::Press(pos, PtrButton::LEFT));

        self.last_pos = pos;
        self.last_press_time = SystemTime::now();
        self.press_pos = Some(pos);
        self.holding = true;
        self.ignore_release = false;
    }

    fn phase_move(&mut self, _id: i32, pos: Vec2, mut out: impl FnMut(LogisimInputEvent)) {
        self.last_pos = pos;
        out(LogisimInputEvent::Hover(pos));

        if let Some(press_pos) = self.press_pos {
            let press_dist = press_pos.distance_squared(pos).abs();
            if press_dist >= 50.0 * 50.0 {
                self.holding = false;
                self.press_pos = None;
            }
        }
    }

    fn phase_end(&mut self, _id: i32, pos: Vec2, mut out: impl FnMut(LogisimInputEvent)) {
        out(LogisimInputEvent::Release(pos, PtrButton::LEFT));

        // If we've been holding the pointer still and have not
        // triggered a right click, we should send a left click
        if !self.ignore_release && self.holding {
            out(LogisimInputEvent::Click(pos, PtrButton::LEFT));
        }
        self.press_pos = None;
        self.holding = false;
        out(LogisimInputEvent::PointerLeft);
    }
}

struct State {
    combining_accent: Option<char>,
    window: Option<Window>,
    quit: bool,
    redraw: bool,
    running: bool,
    app: App,
    android: AndroidApp,
    input: InputState,
    translater: TouchTranslater,
    text_input: Option<TextInputState>,
}

#[no_mangle]
fn android_main(android: AndroidApp) {
    android_logd_logger::builder()
        .filter_level(log::LevelFilter::Error)
        .filter_module("logisim_common", log::LevelFilter::Info)
        .filter_module("main", log::LevelFilter::Info)
        .init();
    let mut frame_timer = Timer::new(60);

    log::trace!("ANDROID_MAIN (TRACE)");
    log::info!("ANDROID_MAIN (INFO)");
    log::warn!("ANDROID_MAIN (WARN)");
    log::error!("ANDROID_MAIN (ERROR)");

    let mut state = State {
        combining_accent: None,
        window: None,
        quit: false,
        redraw: true,
        running: false,
        app: App::default(),
        android: android.clone(),
        input: InputState::default(),
        translater: TouchTranslater::default(),
        text_input: None,
    };
    state.app.init();
    let timeout = Duration::from_millis(1000 / 60);
    let mut perf = Perf::default();

    while !state.quit {
        android.poll_events(Some(timeout), |event| {
            match event {
                PollEvent::Wake => {}
                PollEvent::Timeout => state.redraw = true,
                PollEvent::Main(main_event) => {
                    on_main_event(main_event, &mut state);
                }
                _ => {}
            }

            // TODO: FIX FPS BEING CAPPED AT 20
            if frame_timer.ready() && state.running {
                frame_timer.reset();
                state.redraw = false;

                let mut debug_perf = false;
                on_frame(&mut state, &mut perf, &mut debug_perf);
                perf.end_frame();

                if debug_perf {
                    log::info!("FRAME AVERGAES: {:#?}", perf.averages());
                }
            }
        });
    }
}

fn on_main_event(event: MainEvent, state: &mut State) {
    match event {
        MainEvent::SaveState { .. } => {}
        MainEvent::Pause => {
            log::info!("App paused - dropping display & GPU handles...");

            state.running = false;
            state.app.pause();
        }
        MainEvent::Resume { .. } => {
            log::info!("App resumed");
            state.running = true;
        }
        MainEvent::InitWindow { .. } => {
            log::info!("Window initialized - creating display & GPU handles...");
            state.window = state.android.native_window().map(Window::new);
            state.redraw = true;

            if let Some(win) = &state.window {
                pollster::block_on(state.app.resume(win, win.size()));
            } else {
                log::error!("native_window() returned None during InitWindow callback");
            }
        }
        MainEvent::TerminateWindow { .. } => {
            log::info!("App terminated...");
            state.window = None;
        }
        MainEvent::WindowResized { .. } => {
            state.redraw = true;
            if let Some(win) = &state.window {
                log::info!("Resizing app to {:?}", win.size());
                state.app.update_size(win.size());
            } else {
                log::error!("Handling WindowResized but window is None");
            }
        }
        MainEvent::RedrawNeeded { .. } => {
            state.redraw = true;
        }
        MainEvent::InputAvailable { .. } => {
            state.redraw = true;
        }
        MainEvent::ConfigChanged { .. } => {}
        MainEvent::LowMemory => log::warn!("Recieved LowMemory Event..."),
        MainEvent::Destroy => {
            log::info!("App destroyed...");
            state.quit = true;
        }
        _ => {}
    }
}

fn on_input_event(
    input: &mut InputState,
    translater: &mut TouchTranslater,
    event: &InputEvent,
    android: &AndroidApp,
    combining_accent: &mut Option<char>,
    text_input: &mut Option<TextInputState>,
) -> InputStatus {
    match event {
        InputEvent::KeyEvent(key_event) => {
            let combined_key_char =
                character_map_and_combine_key(android, key_event, combining_accent);
            match combined_key_char {
                Some(KeyMapChar::Unicode(ch)) | Some(KeyMapChar::CombiningAccent(ch)) => {
                    input.on_event(LogisimInputEvent::Type(ch));
                }
                _ => {}
            }
        }
        InputEvent::MotionEvent(motion_event) => {
            let id = motion_event.device_id();
            let pointer = motion_event.pointer_at_index(motion_event.pointer_index());
            let pos = vec2(pointer.x(), pointer.y());
            let handler = |e: LogisimInputEvent| input.on_event(e);

            match motion_event.action() {
                MotionAction::Down => translater.phase_start(id, pos, handler),
                MotionAction::Up => translater.phase_end(id, pos, handler),
                MotionAction::Move => translater.phase_move(id, pos, handler),
                a => log::warn!("Unknown motion action: {a:?}"),
            }
        }
        InputEvent::TextEvent(text_state) => {
            let min = text_state.selection.start.min(text_state.selection.end) as u32;
            let max = text_state.selection.start.max(text_state.selection.end) as u32;
            let compose = text_state.compose_region.and_then(|span| {
                let min = span.start.min(span.end);
                let max = span.start.max(span.end);
                match min == max {
                    true => None,
                    false => Some(min as u32..max as u32),
                }
            });

            let mut info = TextInputState {
                text: text_state.text.clone(),
                selection: min..max,
                compose,
            };
            // Temporary fix for backspace deleting 2 characters on android keyboard.
            if let Some(input) = text_input {
                if input.text.len().saturating_sub(info.text.len()) == 2 {
                    info.text.push(input.text.chars().rev().nth(1).unwrap());
                    info.selection.end += 1;
                    if let Some(range) = &mut info.compose {
                        range.end += 1;
                    }
                }

                android.set_text_input_state(android_activity::input::TextInputState {
                    text: info.text.clone(),
                    selection: android_activity::input::TextSpan {
                        start: info.selection.start as usize,
                        end: info.selection.end as usize,
                    },
                    compose_region: info.compose.as_ref().map(|range| {
                        android_activity::input::TextSpan {
                            start: range.start as usize,
                            end: range.end as usize,
                        }
                    }),
                });
            }
            log::info!("Android set TextInput to {info:?}");
            input.set_text_input(Some(info.clone()));
            *text_input = Some(info);
        }
        _ => {}
    }
    InputStatus::Unhandled
}

fn on_frame(state: &mut State, perf: &mut Perf, debug_perf: &mut bool) {
    state.translater.update(|e| state.input.on_event(e));
    let (input, translater) = (&mut state.input, &mut state.translater);
    perf.start("input handling");
    match state.android.input_events_iter() {
        Ok(mut iter) => loop {
            if !iter.next(|event| {
                on_input_event(
                    input,
                    translater,
                    event,
                    &state.android,
                    &mut state.combining_accent,
                    &mut state.text_input,
                )
            }) {
                break;
            }
        },
        Err(err) => {
            log::warn!("Failed to get input events iterator: {err:?}");
        }
    }
    perf.end();

    let Some(_win) = &state.window else {
        log::warn!("Failed to draw frame: window is None");
        return;
    };

    perf.start("content_rect access");
    let mut text_input = None;
    let content_rect = state.android.content_rect();
    let mut content_rect = logisim_common::graphics::Rect::from_min_max(
        logisim_common::glam::vec2(content_rect.left as f32, content_rect.top as f32),
        logisim_common::glam::vec2(content_rect.right as f32, content_rect.bottom as f32),
    );
    content_rect.min += logisim_common::glam::vec2(
        LEFT_DISPLAY_INSET.load(Ordering::Relaxed) as f32,
        TOP_DISPLAY_INSET.load(Ordering::Relaxed) as f32,
    );
    content_rect.max -= logisim_common::glam::vec2(
        RIGHT_DISPLAY_INSET.load(Ordering::Relaxed) as f32,
        BOTTOM_DISPLAY_INSET.load(Ordering::Relaxed) as f32,
    );
    perf.end();

    if let Err(err) = state
        .app
        .draw_frame(&mut state.input, content_rect, &mut text_input, perf)
    {
        log::warn!("Failed to draw frame: {err:?}");
        perf.end();
        return;
    }

    perf.start("misc");
    state.input.update();
    if state.text_input.is_none() && text_input.is_some() {
        log::info!("App started wanting text input ;' opening keyboard");
        state.android.show_soft_input(true);
    }
    if state.text_input.is_some() && text_input.is_none() {
        log::info!("App stopped wanting text input ;' closing keyboard");
        state.android.hide_soft_input(true);
    }

    if text_input.is_some() && text_input != state.text_input {
        let text = text_input.as_ref().expect("Can't happen");
        log::info!("Setting androids TextInput to {text_input:?}");
        state
            .android
            .set_text_input_state(android_activity::input::TextInputState {
                text: text.text.clone(),
                selection: android_activity::input::TextSpan {
                    start: text.selection.start as usize,
                    end: text.selection.end as usize,
                },
                compose_region: text.compose.as_ref().map(|range| {
                    android_activity::input::TextSpan {
                        start: range.start as usize,
                        end: range.end as usize,
                    }
                }),
            });
    }
    perf.end();
    state.text_input = text_input;
}

/// Tries to map the `key_event` to a `KeyMapChar` containing a unicode character or dead key accent
///
/// This shows how to take a `KeyEvent` and look up its corresponding `KeyCharacterMap` and
/// use that to try and map the `key_code` + `meta_state` to a unicode character or a
/// dead key that be combined with the next key press.
fn character_map_and_combine_key(
    android: &AndroidApp,
    key_event: &KeyEvent,
    combining_accent: &mut Option<char>,
) -> Option<KeyMapChar> {
    let device_id = key_event.device_id();

    let key_map = match android.device_key_character_map(device_id) {
        Ok(key_map) => key_map,
        Err(err) => {
            log::warn!("Failed to look up `KeyCharacterMap` for device {device_id}: {err:?}");
            return None;
        }
    };

    match key_map.get(key_event.key_code(), key_event.meta_state()) {
        Ok(KeyMapChar::Unicode(unicode)) => {
            // Only do dead key combining on key down
            if key_event.action() == KeyAction::Down {
                let combined_unicode = if let Some(accent) = combining_accent {
                    match key_map.get_dead_char(*accent, unicode) {
                        Ok(Some(key)) => {
                            log::warn!("KeyEvent: Combined '{unicode}' with accent '{accent}' to give '{key}'");
                            Some(key)
                        }
                        Ok(None) => None,
                        Err(err) => {
                            log::warn!("KeyEvent: Failed to combine 'dead key' accent '{accent}' with '{unicode}': {err:?}");
                            None
                        }
                    }
                } else {
                    Some(unicode)
                };
                *combining_accent = None;
                combined_unicode.map(|unicode| KeyMapChar::Unicode(unicode))
            } else {
                Some(KeyMapChar::Unicode(unicode))
            }
        }
        Ok(KeyMapChar::CombiningAccent(accent)) => {
            if key_event.action() == KeyAction::Down {
                *combining_accent = Some(accent);
            }
            Some(KeyMapChar::CombiningAccent(accent))
        }
        Ok(KeyMapChar::None) => {
            // Leave any combining_accent state in tact (seems to match how other
            // Android apps work)
            log::warn!("KeyEvent: Pressed non-unicode key");
            None
        }
        Err(err) => {
            log::warn!("KeyEvent: Failed to get key map character: {err:?}");
            *combining_accent = None;
            None
        }
    }
}