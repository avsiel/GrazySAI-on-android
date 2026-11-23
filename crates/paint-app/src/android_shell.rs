//! Мостик к Java-API Android через JNI (без Gradle и без Java-кода в APK).
//!
//! Начиная с Android 11 приложение по умолчанию видит только свой каталог
//! (`Android/data/<пакет>/files`) и медиатеку. Чтобы встроенный обозреватель файлов показывал
//! `Download`, `Pictures` и всю общую память — как на рабочем столе, — просим у системы
//! «доступ ко всем файлам» (`MANAGE_EXTERNAL_STORAGE`). Пользователь выдаёт его в настройках,
//! ответа через `onActivityResult` не требуется: достаточно перепроверить
//! `Environment.isExternalStorageManager()` после возврата в приложение.
//!
//! Здесь же — полноэкранный вид (`immersive_fullscreen`) и вырез камеры (`safe_insets`):
//! тема `Theme.NoTitleBar.Fullscreen` в манифесте рисует окно под статусной строкой,
//! и над холстом остаётся чёрная полоса от значка шторки.

use jni::objects::{JObject, JValue};
use jni::JavaVM;
use std::path::PathBuf;

/// Пакет приложения — для ссылки `package:…` в настройках Android.
pub const PACKAGE: &str = "com.grazyping.grazysai.android";
/// Общая память устройства.
pub const STORAGE: &str = "/storage/emulated/0";

/// JavaVM и глобальная ссылка на Activity из `ndk-context`.
fn context() -> Option<(JavaVM, JObject<'static>)> {
    // SAFETY: ndk-context заполняется android-activity до вызова android_main, а JavaVM и
    // глобальная ссылка на Activity живут всё время существования activity. Паника при
    // неинициализированном контексте (например, в тестах) перехватывается.
    let ctx = std::panic::catch_unwind(ndk_context::android_context).ok()?;
    // SAFETY: указатель получен из ndk-context и указывает на настоящий JavaVM.
    let vm = unsafe { JavaVM::from_raw(ctx.vm().cast()) }.ok()?;
    // SAFETY: это глобальная JNI-ссылка, её не нужно (и нельзя) освобождать здесь.
    let activity = unsafe { JObject::from_raw(ctx.context().cast()) };
    Some((vm, activity))
}

/// Выполнить `f` с окружением JNI на текущем (UI) потоке.
fn with_env<R>(f: impl FnOnce(&mut jni::JNIEnv, &JObject) -> R) -> Option<R> {
    let (vm, activity) = context()?;
    let mut guard = vm.attach_current_thread().ok()?;
    Some(f(&mut guard, &activity))
}

/// Номер уровня API (`Build.VERSION.SDK_INT`), 0 — если узнать не удалось.
fn sdk_int(env: &mut jni::JNIEnv) -> i32 {
    env.get_static_field("android/os/Build$VERSION", "SDK_INT", "I")
        .and_then(|v| v.i())
        .unwrap_or(0)
}

/// Есть ли доступ ко всей общей памяти.
pub fn all_files_access() -> bool {
    with_env(|env, activity| {
        if sdk_int(env) < 30 {
            // До Android 11 хватает обычного разрешения на общую память.
            let Ok(read) = env.new_string("android.permission.READ_EXTERNAL_STORAGE") else { return false };
            return env
                .call_method(activity, "checkSelfPermission", "(Ljava/lang/String;)I", &[JValue::Object(&read)])
                .and_then(|v| v.i())
                .map(|v| v == 0)
                .unwrap_or(false);
        }
        match env.call_static_method("android/os/Environment", "isExternalStorageManager", "()Z", &[]) {
            Ok(v) => v.z().unwrap_or(false),
            Err(err) => {
                let _ = env.exception_clear();
                log::warn!("GrazySAI: isExternalStorageManager: {err}");
                false
            }
        }
    })
    .unwrap_or(false)
}

/// Открыть системный экран выдачи доступа ко всем файлам. `true` — экран открылся.
pub fn request_all_files_access() -> bool {
    with_env(|env, activity| {
        if sdk_int(env) < 30 {
            return request_legacy(env, activity);
        }
        log::info!("GrazySAI: requesting all-files access, sdk={}", sdk_int(env));
        // 1) экран «Доступ ко всем файлам» именно для нашего приложения
        if start_settings(env, activity, "android.settings.MANAGE_APP_ALL_FILES_ACCESS_PERMISSION", Some("package:".to_owned() + PACKAGE)) {
            return true;
        }
        // 2) общий список приложений с таким доступом (есть не на всех прошивках)
        if start_settings(env, activity, "android.settings.MANAGE_ALL_FILES_ACCESS_PERMISSION", None) {
            return true;
        }
        // 3) хотя бы сведения о приложении — оттуда до разрешений два касания
        start_settings(env, activity, "android.settings.APPLICATION_DETAILS_SETTINGS", Some("package:".to_owned() + PACKAGE))
    })
    .unwrap_or(false)
}

/// До Android 11: обычный запрос разрешений на общую память (диалог системы).
fn request_legacy(env: &mut jni::JNIEnv, activity: &JObject) -> bool {
    let (Ok(read), Ok(write), Ok(class)) = (
        env.new_string("android.permission.READ_EXTERNAL_STORAGE"),
        env.new_string("android.permission.WRITE_EXTERNAL_STORAGE"),
        env.find_class("java/lang/String"),
    ) else {
        return false;
    };
    let Ok(arr) = env.new_object_array(2, class, JObject::null()) else { return false };
    if env.set_object_array_element(&arr, 0, &read).is_err() || env.set_object_array_element(&arr, 1, &write).is_err() {
        return false;
    }
    let ok = env
        .call_method(activity, "requestPermissions", "([Ljava/lang/String;I)V", &[JValue::Object(&arr), JValue::Int(7)])
        .is_ok();
    if !ok {
        let _ = env.exception_clear();
    }
    ok
}

/// `Intent(action[, data]).let { activity.startActivity(it) }`; ошибки только в logcat.
fn start_settings(env: &mut jni::JNIEnv, activity: &JObject, action: &str, data: Option<String>) -> bool {
    let res = (|| -> jni::errors::Result<()> {
        let action = env.new_string(action)?;
        let intent = env.new_object("android/content/Intent", "(Ljava/lang/String;)V", &[JValue::Object(&action)])?;
        let intent = match data {
            Some(data) => {
                let data = env.new_string(data)?;
                let uri = env
                    .call_static_method("android/net/Uri", "parse", "(Ljava/lang/String;)Landroid/net/Uri;", &[JValue::Object(&data)])?
                    .l()?;
                env.call_method(&intent, "setData", "(Landroid/net/Uri;)Landroid/content/Intent;", &[JValue::Object(&uri)])?.l()?
            }
            None => intent,
        };
        // Из NativeActivity не всегда нужен NEW_TASK, но на части прошивок без него падает.
        let intent = env.call_method(&intent, "addFlags", "(I)Landroid/content/Intent;", &[JValue::Int(0x10000000)])?.l()?;
        env.call_method(activity, "startActivity", "(Landroid/content/Intent;)V", &[JValue::Object(&intent)])?;
        Ok(())
    })();
    match res {
        Ok(()) => true,
        Err(err) => {
            let _ = env.exception_clear();
            log::warn!("GrazySAI: startActivity: {err}");
            false
        }
    }
}

/// Полноэкранный вид: холст под статусной строкой и навигацией, вырез камеры в стороне.
/// Без этого над холстом остаётся чёрная полоса до значка шторки.
pub fn immersive_fullscreen() {
    with_env(|env, activity| {
        let window = match env.call_method(activity, "getWindow", "()Landroid/view/Window;", &[]) {
            Ok(w) => match w.l() {
                Ok(w) => w,
                Err(_) => return,
            },
            Err(err) => {
                let _ = env.exception_clear();
                log::warn!("GrazySAI: getWindow: {err}");
                return;
            }
        };
        // Окно во всю панель, включая области под вырезом и системными полосами.
        let mask = 0x00000200 | 0x00000400 | 0x00000100 | 0x00000010 | 0x00000800; // LAYOUT_NO_LIMITS|LAYOUT_IN_SCREEN|LAYOUT_FULLSCREEN|LAYOUT_STABLE|LAYOUT_HIDE_NAVIGATION
        let _ = env.call_method(&window, "addFlags", "(I)V", &[JValue::Int(mask)]);
        let _ = env.call_method(&window, "clearFlags", "(I)V", &[JValue::Int(0x00000400 | 0x00000200)]);
        // ContentView кладём под системные полосы (Android 9 разрешает, от 11 и на прочих).
        let decor = env.call_method(&window, "getDecorView", "()Landroid/view/View;", &[]).ok().and_then(|d| d.l().ok());
        if let Some(decor) = decor {
            if sdk_int(env) >= 28 {
                let _ = env.call_method(
                    &decor,
                    "setSystemUiVisibility",
                    "(I)V",
                    &[JValue::Int(0x00000400 | 0x00000100)],
                );
            }
        }
        if sdk_int(env) >= 30 {
            // окно во всю панель, включая вырез и системные полосы
            let _ = env.call_method(&window, "setDecorFitsSystemWindows", "(Z)V", &[JValue::Bool(0)]);
            cutout_mode(env, &window, 2); // LAYOUT_IN_DISPLAY_CUTOUT_MODE_ALWAYS
        }
        // Занавеска и панель навигации прячутся: касание или свайп показывают их снова.
        let _ = env.call_method(
            &window,
            "setSystemUiVisibility",
            "(I)V",
            &[JValue::Int(0x00000004 | 0x00000002 | 0x00000100 | 0x00000400)],
        );
        if sdk_int(env) >= 30 {
            translucent_bars(env, &window);
        }
        // Панель навигации полупрозрачная: спрятанная, а не чёрная полоса.
        let _ = env.call_method(&window, "setNavigationBarColor", "(I)V", &[JValue::Int(0x00000000)]);
        let _ = env.call_method(&window, "setStatusBarColor", "(I)V", &[JValue::Int(0x00000000)]);
    });
}

/// `Window.setDecorFitsSystemWindows(false)` (API 30+) — окно рисуется под вырезом.
fn cutout_mode(env: &mut jni::JNIEnv, window: &JObject, mode: i32) {
    let attrs = env.call_method(window, "getAttributes", "()Landroid/view/WindowManager$LayoutParams;", &[]).ok().and_then(|a| a.l().ok());
    let Some(attrs) = attrs else { return };
    let Ok(cls) = env.get_object_class(&attrs) else { return };
    let _ = env.set_field(&attrs, "layoutInDisplayCutoutMode", "I", JValue::Int(mode));
    let _ = cls;
    let _ = env.call_method(window, "setAttributes", "(Landroid/view/WindowManager$LayoutParams;)V", &[JValue::Object(&attrs)]);
}

/// Прозрачные полосы `WindowInsetsController.setSystemBarsBehavior` (API 30+).
fn translucent_bars(env: &mut jni::JNIEnv, window: &JObject) {
    let Ok(controller) = env.call_method(window, "getInsetsController", "()Landroid/view/WindowInsetsController;", &[]) else { return };
    let Ok(controller) = controller.l() else { return };
    // BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
    let _ = env.call_method(&controller, "setSystemBarsBehavior", "(I)V", &[JValue::Int(2)]);
}

/// Отступы в точках экрана (физические пиксели): сверху (вырез/статус), снизу, слева, справа.
/// Нужны, чтобы строка кнопок не уезжала под вырез камеры.
pub fn safe_insets() -> [f32; 4] {
    with_env(|env, activity| {
        let Some(window) = env.call_method(activity, "getWindow", "()Landroid/view/Window;", &[]).ok().and_then(|w| w.l().ok()) else {
            return [0.0; 4];
        };
        let Some(decor) = env.call_method(&window, "getDecorView", "()Landroid/view/View;", &[]).ok().and_then(|d| d.l().ok()) else {
            return [0.0; 4];
        };
        let Some(insets) = env.call_method(&decor, "getRootWindowInsets", "()Landroid/view/WindowInsets;", &[]).ok().and_then(|i| i.l().ok()) else {
            return [0.0; 4];
        };
        // getInsets(WindowInsets.Type.systemBars()|displayCutout()) — только с API 30
        if sdk_int(env) < 30 {
            return [0.0; 4];
        }
        let Ok(ty) = env.find_class("android/view/WindowInsets$Type") else { return [0.0; 4] };
        let mut bits = 0i32;
        for name in ["statusBars", "displayCutout", "navigationBars"] {
            if let Ok(v) = env.call_static_method(&ty, name, "()I", &[]).and_then(|v| v.i()) {
                bits |= v;
            }
        }
        let Ok(ins) = env.call_method(&insets, "getInsets", "(I)Landroid/graphics/Insets;", &[JValue::Int(bits)]).and_then(|i| i.l()) else {
            return [0.0; 4];
        };
        let get = |env: &mut jni::JNIEnv, name: &str| -> f32 {
            env.get_field(&ins, name, "I").and_then(|v| v.i()).map_or(0.0, |v| v as f32)
        };
        [get(env, "top"), get(env, "bottom"), get(env, "left"), get(env, "right")]
    })
    .unwrap_or([0.0; 4])
}

/// Папки для закладок обозревателя: корень памяти (если доступен) и обычные каталоги.
pub fn roots() -> Vec<PathBuf> {
    let mut v = Vec::new();
    let storage = PathBuf::from(STORAGE);
    if all_files_access() {
        v.push(storage.clone());
    }
    for name in ["Download", "Pictures", "Documents", "DCIM"] {
        let p = storage.join(name);
        if std::fs::read_dir(&p).is_ok() {
            v.push(p);
        }
    }
    v
}
