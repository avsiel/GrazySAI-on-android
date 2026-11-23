# GrazySAI для Android (полный порт)

APK собирается из **того же кода редактора**, что и десктоп: `crates/paint-app` подключается как
библиотека (`paint_app::run_android`) и запускается внутри `android.app.NativeActivity`.
Интерфейс, кисти, слои, режимы наложения, выделения, фильтры, таймлайн, `.sai2`, 3D и плагины —
это original-код, а не упрощённая копия.

## Архитектура

- `crates/paint-app` — весь редактор (14k+ строк `main.rs`, `sai_ui.rs`, `dock.rs`, `timeline.rs`, ...).
  Добавлен `lib`-таргет на тот же `src/main.rs` и публичная функция `run_android(app)`.
- `crates/paint-android-full` — `cdylib` (`libpaint_android_full.so`) с точкой входа `android_main`.
- `vendor/rfd`, `vendor/arboard` — локальные копии с Android-заглушками: системные диалоги и
  буфер обмена на Android требуют Java/JNI, поэтому пока возвращают «отмена»/ошибку.
- `vendor/winit` — локальная копия winit 0.30.13 (см. `PATCHED.md`): экспортирует наклон и кнопки
  пера из `MotionEvent`, которые стандартный конвертер теряет при создании `egui::Touch`.
- `android/build_apk.ps1` — сборка без Gradle: `cargo ndk` → `aapt2` → `zipalign` → `apksigner`.

На Android-таргете десктопные зависимости собираются без X11/Wayland; `eframe` получает фичу
`android-native-activity`, а `AndroidApp` передаётся в `event_loop_builder`.

## Что работает

Полный интерфейс SAI-подобного редактора: панель инструментов, палитра, навигатор, слои с режимами
наложения, таймлайн, настройки кистей, горячие клавиши — весь UI из `paint-app`. Рисование
пальцем/пером через egui touch events. Нажим с пера приходит по-настоящему: `winit` берёт
`MotionEvent.getPressure()` и передаёт его как `force`; на Lenovo Precision Pen 3 проверено,
что значение меняется от ~0.04 до ~0.82 и кисть реагирует на него. В logcat это видно по
`GrazySAI: paint_app: касание: force=...` (лог ограничен одной записью в 300 мс).
Наклон и ластик пера тоже работают: `vendor/winit` забирает `Axis::Tilt`, `Axis::Orientation`,
кнопки и `ToolType` из сырого `MotionEvent` до преобразования в `egui::Touch`. Наклон подаётся в
динамику кисти (величина 0..1 и направление), вторая кнопка пера и обратный конец — ластик.
На Lenovo Precision Pen 3 в logcat：`перо: наклон=24…34° напр=−122…−132° кнопки=0x40 ластик=true`.

**Масштаб интерфейса.** Верхняя панель справа содержит кнопку `UI 100%` — одно касание
перебирает 75/100/125/150/175/200/250% (те же значения и в `Window → Scaling of User Interface`).
Внизу кнопку ставить нельзя: на планшете нижние ~60 px занимает системный taskbar.

Открытие и сохранение работают через **встроенный FileViewer из SAI** (тот же, что на рабочем
столе): `files::native()` на Android сразу возвращает `false`, а в `run_android` включён
`always_viewer`. Рабочая папка по умолчанию — внешний каталог приложения
`/sdcard/Android/data/com.grazyping.grazysai.android/files` (доступен без разрешений SAF),
он же добавляется в закладки обозревателя. Поддерживаются `.sai2`, `.psd`, `.psb`, `.ora` и
картинки (PNG, JPG, BMP, TGA, TIFF, WebP, QOI).

**Доступ ко всем папкам.** Пункт `File → Доступ ко всем папкам…` открывает системный экран
`MANAGE_EXTERNAL_STORAGE` (на Android 11+), а на старых версиях — обычный диалог
`READ/WRITE_EXTERNAL_STORAGE`. После выдачи разрешения (`Environment.isExternalStorageManager()`)
обозреватель автоматически показывает `/storage/emulated/0` с `Download`, `Pictures`, `Documents`,
`DCIM` и остальными каталогами; корень и стандартные папки добавляются в закладки. Реализация —
JNI-мост `crates/paint-app/src/android_shell.rs` через уже присутствующие `jni`/`ndk-context`,
без Java-кода и Gradle. Проверено: `.sai2` из `/sdcard/Download` открывается, статус подтверждает
`Opened /storage/emulated/0/Download/…`.

## Ограничения Android-порта

- Системный пикер SAF (`ACTION_OPEN_DOCUMENT`/`ACTION_CREATE_DOCUMENT`) не используется: вместо
  него тот же FileViewer SAI плюс разрешение «доступ ко всем файлам». Это даёт прямой доступ к
  обычным путям и сохраняет совместимость форматов.
- Буфер обмена и crash-owner проверки обновлений пока заглушки.
- Нет нативной панели ввода/клавиатуры Android: горячие клавиши работают только с BT-клавиатурой.
- Перо: наклон и ластик идут мимо `egui::Touch`, поэтому для них патчится `vendor/winit`.
  Первая кнопка пера пока не назначена на действие (вторая — ластик, проверено на планшете).
- Интерфейс рассчитан на крупный экран (планшет); на телефоне панели тесные. Строка меню
  на узком экране прокручивается по горизонтали (свайп по ней), иначе `Window`, `View` и кнопка
  масштаба UI уходят за правый край.
- Self-update/запуск внешнего `exe` отключён.

## Сборка

Нужны: Rust с таргетом `aarch64-linux-android`, `cargo-ndk`, Android SDK (Platform 34,
Build Tools, NDK 26+) и JDK из Android Studio.

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\android\build_apk.ps1
```

Скрипт:

1. `cargo ndk -t arm64-v8a -o android/jniLibs build --release -p paint-android-full`;
2. `aapt2 compile/link` (min SDK 24, target SDK 34);
3. добавляет `lib/arm64-v8a/libpaint_android_full.so` и `libc++_shared.so` из NDK в APK и выравнивает;
4. подписывает debug-ключом.

`libc++_shared.so` обязателен: без него `NativeActivity` падает с `UnsatisfiedLinkError` ещё до
запуска Rust-кода.

Результат: `GrazySAI-android.apk` в корне репозитория (~39 МБ).

## Установка и запуск

```powershell
adb install -r GrazySAI-android.apk
adb shell am start -n com.grazyping.grazysai.android/android.app.NativeActivity
adb logcat -s GrazySAI:I eframe:I
```

## Экран телефона на ПК

Если scrcpy недоступен (нет сети/пакета), есть локальная замена — `android/phone_screen.py`.
Скрипту нужны только `adb` и Pillow; он каждые ~0,3 с снимает `screencap` и показывает экран
в окне Tk. Левый клик — касание, протяжка — свайп, правый клик — «Назад», средний — «Домой»,
колесо — прокрутка. `Esc` — «Назад», `Home` — «Домой», `Ctrl+S` — снимок, `Ctrl+Q` — закрыть.

```powershell
python android\phone_screen.py
python android\phone_screen.py --serial R3CX20H54YZ --height 900
```

Снимки и журнал действий складываются в `%TEMP%\opencode\phone_shots`
(`001_start.png`, `002_tap_…png`, `actions.log`). Это удобно и для ручной проверки, и для
разбора: видно, куда именно нажали и что получилось.

## Проверка

```powershell
cargo ndk -t arm64-v8a -P 24 check -p paint-app --lib
cargo ndk -t arm64-v8a -P 24 check -p paint-android-full
cargo check -p paint-app --bin GrazySAI   # десктоп не сломан
cargo test -p paint-formats --lib         # SVG/кисти: 34 теста
```

`aapt2 dump badging GrazySAI-android.apk` показывает `native-code: 'arm64-v8a'`.

### Проверено на Lenovo TB371FC (Android 14)

- 150 случайных жестов + 120 штрихов пером с разным нажимом: PID не менялся, паник и
  `AndroidRuntime` в logcat нет.
- Память: 135 МБ PSS на старте → 190 МБ после нагрузки → 189 МБ после сворачивания,
  повторного открытия и поворота экрана в обе стороны (утечки нет).
- Нажим пера доходит до движка кисти: `force` 0.04…0.82 при реальном рисовании.
- Open/Save через FileViewer: `NewCanvas1.sai2` сохранён в
  `/sdcard/Android/data/com.grazyping.grazysai.android/files` (56 КБ) и открыт обратно из
  обозревателя; статус подтвердил `Opened …/NewCanvas1.sai2`.
- После выдачи «доступа ко всем файлам» FileViewer показал `/storage/emulated/0` со списком
  `Documents`, `Download`, `Pictures`, `Movies`, `Music`; файл из `/sdcard/Download` открылся
  (`Opened /storage/emulated/0/Download/AAAA_ALL_FILES.sai2`, 1200×900, слои загружены).
- Наклон пера 23–34° и направление −115…−133° идут в движок; вторая кнопка (`0x40`) включает
  ластик — штрих стирает, а не рисует.
- Кнопка `UI 100%` в верхней панели переключила масштаб на 125%: zoom холста 170% → 213%,
  меню и панели увеличились.
- `cargo test -p paint-formats --lib`: 34 passed, 0 failed (включая чтение SVG-оттиска кисти).
