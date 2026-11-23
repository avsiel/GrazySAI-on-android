# GrazySAI on Android

[Русский](README.md) | **English**

This is an attempt to bring GrazySAI — a free painting and animation app in Rust, modelled on
PaintTool SAI 2 — to phones and tablets. The engine is the one from the desktop version
([grazyping/GrazySAI](https://github.com/grazyping/GrazySAI), 1.4.6): brushes, layers, the timeline,
3D and the file formats work. What doesn't work yet is the thing that matters most on a phone —
a usable interface.

![GrazySAI on Android](docs/screenshot.png)

## Where it stands

The port is in development, and the interface for phones is **very rough**: buttons are small in
places, panels overlap each other, there are almost no gestures, and things get cut off on short
screens. It crashes sometimes. The APK is here so people can try it and say what exactly gets in
the way — not because it can replace SAI on a tablet yet.

## Try it

1. Download `GrazySAI-<version>-android-arm64-v8a.apk` from
   [Releases](https://github.com/avsiel/GrazySAI-on-android/releases/latest).
2. Let your browser or file manager install apps from unknown sources.
3. Install and run it. When you first open a file it asks for access to all files; without that
   only the app's own folder is visible.

Android 7.0+ (minSdk 24) and an arm64-v8a CPU. The APK is about 48 MB because the whole native
engine is inside.

## What works

Drawing:
- SAI-style brushes — pencil, airbrush, watercolor, marker, eraser — that mix paint with the canvas,
  plus a stabilizer;
- layers with folders, masks and clipping, all 28 blend modes, layer effects and styles, paper
  texture;
- selections, bucket fill, filters, rulers, curves, text layers;
- brush import from SAI 2, Photoshop, Krita and Clip Studio.

Animation and 3D: a timeline with cels and onion skin, export to GIF, APNG, PNG frames and MP4,
puppet deformation, and 3D layers from glTF, VRM, OBJ, FBX, PMX and COLLADA, traceable to lines.

Files: `.sai2` (SAI 2 opens these), PSD and PSB with layers, OpenRaster, PNG, JPEG, WebP, TIFF, BMP,
TGA, QOI.

Stylus: pressure, tilt and rotation. Tested with a Lenovo Precision Pen 3 and nothing else so far.
If pressure doesn't work, the `GrazySAI: paint_app: давление: force=...` line in logcat shows what
arrives.

## What was done for Android

The editor runs inside `android.app.NativeActivity`, so `paint-app` itself opens on the phone
instead of a cut-down viewer. Added: fullscreen that respects system insets, UI scale from 75 to
250% independent of the system setting, a SAI-style file browser over `/sdcard`, and pressure and
tilt taken from `MotionEvent`. `winit`, `rfd` and `arboard` had to be patched — details in
[`android/README.md`](android/README.md).

## Limits of this build

- Signed with the Android SDK debug key. Updates only install over this build, otherwise Android
  refuses them.
- arm64-v8a only; no `armeabi-v7a` or `x86_64`.
- All-files access (`MANAGE_EXTERNAL_STORAGE`) is needed on Android 11+, otherwise the file browser
  sees only the app folder.

If something breaks, open an [issue](https://github.com/avsiel/GrazySAI-on-android/issues). The file
that reproduces the problem and your device model help most.

## Building

You need Rust with `cargo-ndk`, the Android SDK (build-tools, platform android-34) and the NDK:

```powershell
.\android\build_apk.ps1              # arm64-v8a, debug-key signing
.\android\build_apk.ps1 -SkipRust    # repackage existing libraries
```

The script runs `cargo ndk build --release -p paint-android-full`, then `aapt2`, `zipalign` and
`apksigner`, and leaves `GrazySAI-android.apk` in the root.

## Repository

| Directory | Contains |
|---|---|
| `crates/paint-core` | Pixels, tiles, layers, blending, brushes, strokes, vector shapes, resampling, animation, puppet |
| `crates/paint-formats` | Reading and writing `.sai2`, PSD, OpenRaster and images |
| `crates/paint-app` | The egui editor window, translations in `lang/` |
| `crates/paint-android-full` | The `cdylib` with `android_main` for the APK |
| `crates/paint-3d` | 3D models: reading, poses, rendering, export to GLB and OBJ |
| `crates/paint-studio`, `paint-convert`, `paint-cli` | Brush workshop, converter, command line — desktop only, not in the APK |
| `android/` | Manifest, resources, `build_apk.ps1`, port notes |
| `vendor/` | Patched `winit`, `rfd`, `arboard` |
| `docs/` | Plan, `.sai2` format, the rules of the program, checks |
| `tools/` | Desktop release scripts |

The desktop version with its manual is at [grazyping/GrazySAI](https://github.com/grazyping/GrazySAI).

## License

Same as GrazySAI, see [LICENSE](LICENSE). The `.sai2` format was worked out from users' files;
addresses from the SAI binary never enter this repository.
