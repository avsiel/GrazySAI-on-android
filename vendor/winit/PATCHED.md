# winit 0.30.13 — локальная правка для GrazySAI

Патч подключён через `[patch.crates-io] winit = { path = "vendor/winit" }` в корне репозитория.

## Что изменено

1. `src/platform_impl/android/pen.rs` (новый файл) — `PenState` и потокобезопасное хранилище:
   наклон (`AMOTION_EVENT_AXIS_TILT`), направление (`AXIS_ORIENTATION`), биты кнопок,
   `action_button`, тип инструмента и признак ластика.
2. `src/platform_impl/android/mod.rs` — `pen::update(motion_event, &pointer)` вызывается в
   `handle_input_event` до превращения MotionEvent в `WindowEvent::Touch`.
3. `src/platform/android.rs` — публичная функция `winit::platform::android::pen_state()`.

## Зачем

`android-activity` отдаёт все эти данные, но стандартный конвертер winit оставляет от касания
только `position` и `force`. Без патча наклон пера и кнопки (ластик) недоступны приложению:
`egui::Event::Touch` их не содержит.

Патч не меняет поведение остальных платформ и не ломает совместимость API winit.
