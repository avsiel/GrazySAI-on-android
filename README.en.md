# GrazySAI on Android

[Русский](README.md) | **English**

Android port of GrazySAI; for now, the interface is adapted for tablets only. GrazySAI is a free drawing app with lots of extra features, written in Rust and modeled after PaintTool SAI 2 ([desktop original](https://github.com/grazyping/GrazySAI), version 1.4.6).

The engine here is the same as on desktop, so brushes, layers, timeline, 3D, and files work.

## Interface screenshot
<img width="2944" height="1840" alt="Screenshot_20261125-223251" src="https://github.com/user-attachments/assets/46815148-8da2-4dac-9ce7-67a71d3a2500" />

## Status

The port is still rough. Icons are sometimes mixed up, there are stripes between panels, gestures leave accidental strokes, and on short screens part of the interface is cut off. Sometimes the app crashes.

## How to install (works only on newer devices; support for older ones is not planned yet)

!Required: Android 7.0 or newer and an arm64-v8a processor. The APK is about 48 MB because the whole engine is inside.
1. Download `GrazySAI-<version>-android-arm64-v8a.apk` from the [Releases](https://github.com/avsiel/GrazySAI-on-android/releases/latest) page.
2. Allow your browser or file manager to install apps from unknown sources.
3. Install and launch it.

When opening files for the first time, the app will request access to all files. Without it, it can only see its own folder. — This is needed for the in-app file browser/library to work.

## What it can do

**Drawing.** Brushes like in SAI: pencil, airbrush, watercolor, marker, and eraser. Paint blends with the canvas, and there is a stabilizer. Layers with folders, masks and clipping, 28 blend modes, effects, layer styles, and paper texture. Also selections, fill, filters, rulers, curves, and text layers. Brushes can be imported from SAI 2, Photoshop, Krita, and Clip Studio.

**Animation and 3D.** Timeline with onion skin, export to GIF, APNG, MP4, and PNG frames, puppet deformation. 3D models (glTF, VRM, OBJ, FBX, PMX, COLLADA) can be inserted as a layer and converted to outlines.

**Files.** `.sai2` (opened by SAI 2 itself), PSD and PSB with layers, OpenRaster, PNG, JPEG, WebP, TIFF, BMP, TGA, QOI.

**Pen.** Pressure, tilt, and rotation are supported.

## What I changed for Android

- The editor launches via `android.app.NativeActivity`, so a real `paint-app` opens on the phone, not a cut-down viewer.
- Fullscreen mode that respects system insets.
- UI scale can be changed from 75% to 250%, independently of the system scale.
- SAI-style file browser for `/sdcard`.
- Pressure and tilt are taken from `MotionEvent`.
- Had to patch `winit`, `rfd`, and `arboard`. Details are in [`android/README.md`](android/README.md).

## Limitations

- The APK is signed with the Android SDK debug key. You can only update over this same build; otherwise Android will refuse to install it.
- Only arm64-v8a. There are no builds for `armeabi-v7a` or `x86_64`.
- On Android 11+, the "all files access" permission (`MANAGE_EXTERNAL_STORAGE`) is required; otherwise the file browser only sees the app folder.

If something is broken, please write to [Issues](https://github.com/avsiel/GrazySAI-on-android/issues). Media that reproduces the bug and your device name will help me a lot.

## Building

You will need Rust with `cargo-ndk`, Android SDK (build-tools and platform android-34), and NDK.

```powershell
.\android\build_apk.ps1              # arm64-v8a, signed with debug key
.\android\build_apk.ps1 -SkipRust    # rebuild APK from existing libraries
```

The script runs `cargo ndk build --release -p paint-android-full`, then `aapt2`, `zipalign`, and `apksigner`. The result is the file `GrazySAI-android.apk` in the repository root.

## Repository layout

| Directory | Contents |
|---|---|
| `crates/paint-core` | Pixels, tiles, layers, blending, brushes, strokes, vectors, resampling, animation, puppet deformation |
| `crates/paint-formats` | Reading and writing `.sai2`, PSD, OpenRaster, and regular images |
| `crates/paint-app` | egui editor window, translations in `lang/` |
| `crates/paint-android-full` | `cdylib` with `android_main` for the APK |
| `crates/paint-3d` | 3D models: reading, poses, rendering, export to GLB and OBJ |
| `crates/paint-studio`, `paint-convert`, `paint-cli` | Brush workshop, converter, and CLI utility. Desktop only, not included in the APK |
| `android/` | Manifest, resources, `build_apk.ps1`, port notes |
| `vendor/` | Patched `winit`, `rfd`, `arboard` |
| `docs/` | Plan, `.sai2` format description, program rules, checks |
| `tools/` | Scripts for releasing desktop builds |

Desktop version with manual: [grazyping/GrazySAI](https://github.com/grazyping/GrazySAI).

## License

Same as GrazySAI, see [LICENSE](LICENSE). I parsed the `.sai2` format from users' files. Addresses from the SAI binary do not enter the repository.
