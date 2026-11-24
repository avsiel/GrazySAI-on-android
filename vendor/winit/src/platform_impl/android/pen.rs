//! GrazySAI: дополнительные данные пера из `MotionEvent`, которые winit выбрасывает
//! при преобразовании в `WindowEvent::Touch` (там есть только позиция и сила нажима).
//!
//! Состояние обновляется прямо в `handle_input_event`, до вызова пользовательского callback,
//! поэтому в момент обработки `egui::Event::Touch` оно соответствует текущему событию.

use android_activity::input::{Axis, Button, MotionEvent, Pointer, ToolType};
use std::sync::Mutex;

/// Данные пера для последнего MotionEvent.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PenState {
    /// Наклон от нормали к поверхности, радианы (`AMOTION_EVENT_AXIS_TILT`).
    pub tilt: f32,
    /// Направление наклона в плоскости экрана, радианы (`AMOTION_EVENT_AXIS_ORIENTATION`).
    pub orientation: f32,
    /// Биты нажатых кнопок (`AMOTION_EVENT_BUTTON_*`).
    pub buttons: u32,
    /// Кнопка, которой начато событие (`action_button`).
    pub action_button: u32,
    /// Инструмент — перо или ластик (не палец).
    pub stylus: bool,
    /// Ластик: обратный конец пера или вторая кнопка (Primary — тоже считаем).
    pub eraser: bool,
    /// Увеличивается на каждом событии: приложение отличает новое событие от старого.
    pub seq: u64,
}

const INITIAL: PenState = PenState {
    tilt: 0.0,
    orientation: 0.0,
    buttons: 0,
    action_button: 0,
    stylus: false,
    eraser: false,
    seq: 0,
};

static STATE: Mutex<PenState> = Mutex::new(INITIAL);

/// Обновить состояние из сырого события Android (вызывается в backend'е).
pub(crate) fn update(event: &MotionEvent<'_>, pointer: &Pointer<'_>) {
    let buttons = event.button_state();
    let action: u32 = event.action_button().into();
    // Некоторые перья сообщают ластик только через ToolType, другие — через кнопки.
    // Классический ластик — обратный конец пера или вторая кнопка. Первая кнопка отдельно
    // доступна через `buttons`/`action_button`: приложение может назначить её ластиком в настройках.
    let eraser = matches!(pointer.tool_type(), ToolType::Eraser)
        || buttons.stylus_secondary()
        || action == u32::from(Button::StylusSecondary);
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    s.tilt = pointer.axis_value(Axis::Tilt);
    s.orientation = pointer.axis_value(Axis::Orientation);
    s.buttons = buttons.0;
    s.action_button = action;
    s.stylus = matches!(pointer.tool_type(), ToolType::Stylus | ToolType::Eraser)
        || buttons.stylus_primary()
        || buttons.stylus_secondary()
        || action == u32::from(Button::StylusPrimary)
        || action == u32::from(Button::StylusSecondary);
    s.eraser = eraser;
    s.seq = s.seq.wrapping_add(1);
}

/// Снимок состояния пера (потокобезопасный, без блокировки на стороне приложения).
pub fn state() -> PenState {
    *STATE.lock().unwrap_or_else(|e| e.into_inner())
}
