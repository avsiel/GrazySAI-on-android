//! GrazySAI: данные пера/инструмента из Android MotionEvent.
//!
//! Winit теряет tool type/tilt/buttons при конвертации в WindowEvent::Touch.
//! Сохраняем per-pointer состояние и глобальный снимок последнего события.

use android_activity::input::{Axis, Button, MotionEvent, Pointer, ToolType};
use std::collections::HashMap;
use std::sync::Mutex;

/// Тип инструмента Android MotionEvent.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ToolKind {
    #[default]
    Unknown,
    Finger,
    Stylus,
    Eraser,
}

/// Данные последнего MotionEvent (глобально, для совместимости).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PenState {
    /// Наклон от нормали, радианы.
    pub tilt: f32,
    /// Направление наклона, радианы.
    pub orientation: f32,
    /// Биты кнопок.
    pub buttons: u32,
    /// Кнопка, вызвавшая событие.
    pub action_button: u32,
    /// Событие от стилуса/ластика.
    pub stylus: bool,
    /// Ластик стилуса.
    pub eraser: bool,
    /// Монотонный счётчик событий.
    pub seq: u64,
    /// Тип инструмента текущего pointer.
    pub tool: ToolKind,
    /// Android pointer id текущего pointer.
    pub pointer_id: i32,
}

/// Снимок состояния конкретного pointer.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PointerState {
    pub tool: ToolKind,
    pub tilt: f32,
    pub orientation: f32,
    pub buttons: u32,
    pub action_button: u32,
    pub stylus: bool,
    pub eraser: bool,
    pub seq: u64,
}

static STATE: Mutex<PenState> = Mutex::new(PenState {
    tilt: 0.0,
    orientation: 0.0,
    buttons: 0,
    action_button: 0,
    stylus: false,
    eraser: false,
    seq: 0,
    tool: ToolKind::Unknown,
    pointer_id: -1,
});

static POINTERS: Mutex<Option<HashMap<i32, PointerState>>> = Mutex::new(None);

fn tool_kind(
    pointer: &Pointer<'_>,
    buttons: android_activity::input::ButtonState,
    action: u32,
) -> ToolKind {
    let eraser = matches!(pointer.tool_type(), ToolType::Eraser)
        || buttons.stylus_secondary()
        || action == u32::from(Button::StylusSecondary);
    if eraser {
        return ToolKind::Eraser;
    }
    let stylus = matches!(pointer.tool_type(), ToolType::Stylus)
        || buttons.stylus_primary()
        || action == u32::from(Button::StylusPrimary);
    if stylus {
        return ToolKind::Stylus;
    }
    match pointer.tool_type() {
        ToolType::Finger => ToolKind::Finger,
        _ => ToolKind::Unknown,
    }
}

/// Обновить состояние для pointer Android.
pub(crate) fn update(event: &MotionEvent<'_>, pointer: &Pointer<'_>) {
    let buttons = event.button_state();
    let action: u32 = event.action_button().into();
    let kind = tool_kind(pointer, buttons, action);
    let eraser = kind == ToolKind::Eraser;
    let stylus = matches!(kind, ToolKind::Stylus | ToolKind::Eraser);

    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    s.tilt = pointer.axis_value(Axis::Tilt);
    s.orientation = pointer.axis_value(Axis::Orientation);
    s.buttons = buttons.0;
    s.action_button = action;
    s.stylus = stylus;
    s.eraser = eraser;
    s.seq = s.seq.wrapping_add(1);
    s.tool = kind;
    s.pointer_id = pointer.pointer_id();
    let snapshot = PointerState {
        tool: kind,
        tilt: s.tilt,
        orientation: s.orientation,
        buttons: s.buttons,
        action_button: s.action_button,
        stylus,
        eraser,
        seq: s.seq,
    };
    drop(s);

    let mut map = POINTERS.lock().unwrap_or_else(|e| e.into_inner());
    let map = map.get_or_insert_with(HashMap::new);
    map.insert(pointer.pointer_id(), snapshot);
}

/// Удалить pointer (Up/Cancel).
pub(crate) fn remove(pointer_id: i32) {
    let mut map = POINTERS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(map) = map.as_mut() {
        map.remove(&pointer_id);
    }
}

/// Очистить все pointer-ы (Cancel/focus loss).
pub(crate) fn clear() {
    let mut map = POINTERS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(map) = map.as_mut() {
        map.clear();
    }
}

/// Последний глобальный снимок (совместимость).
pub fn state() -> PenState {
    *STATE.lock().unwrap_or_else(|e| e.into_inner())
}

/// Снимок для конкретного Android pointer id.
pub fn pointer_state(pointer_id: i32) -> Option<PointerState> {
    let map = POINTERS.lock().unwrap_or_else(|e| e.into_inner());
    map.as_ref().and_then(|m| m.get(&pointer_id).copied())
}
