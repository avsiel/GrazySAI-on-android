//! NativeActivity entry point for the full GrazySAI editor.
//!
//! The whole desktop application lives in `paint-app` and is reused as-is.

#[cfg(target_os = "android")]
#[no_mangle]
pub fn android_main(app: winit::platform::android::activity::AndroidApp) {
    let data_dir = app.internal_data_path();

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if let Err(err) = paint_app::run_android(app) {
            log::error!("GrazySAI: {err}");
        }
    }));

    if let Err(payload) = result {
        let text = if let Some(s) = payload.downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = payload.downcast_ref::<String>() {
            s.clone()
        } else {
            format!("{payload:?}")
        };
        log::error!("GrazySAI fatal panic: {text}");
        if let Some(dir) = data_dir {
            let _ = std::fs::write(dir.join("crash.txt"), format!("GrazySAI fatal panic: {text}\n"));
        }
    }
}
