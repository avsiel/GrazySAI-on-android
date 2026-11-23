# GrazySAI for Android

[Русский](README.md) | **English**

**A free painting and animation app for phones and tablets: a brush engine modelled on
PaintTool SAI 2, the full desktop GrazySAI interface, stylus support and `.sai2` files.**
This is a port of the GrazySAI editor ([grazyping/GrazySAI](https://github.com/grazyping/GrazySAI),
engine version 1.4.6) to Android: the same Rust code and the same interface, with a mobile layout.

![GrazySAI on Android](docs/screenshot.png)

## Try it in a minute

1. Open the [**Releases**](https://github.com/avsiel/GrazySAI-on-android/releases/latest) page and
   download `GrazySAI-<version>-android-arm64-v8a.apk`.
2. Allow your file manager or browser to install apps from unknown sources and install the APK.
3. Run it and draw. To open a drawing, use the built-in file viewer (FileViewer in SAI style).

Requires Android 7.0+ (minSdk 24) and an **arm64-v8a** CPU. The APK is about 48 MB: the whole
native engine is inside.

## What you can do

Everything the desktop GrazySAI does.

**Paint**
- Brushes that behave like SAI's: pencil, airbrush, brush, watercolor, marker, eraser. They mix
  paint with the canvas — blending, dilution, persistence. A stabilizer for smooth lines.
- Layers with folders, clipping, masks, all 28 blend modes, paper texture, layer effects (Fringe,
  Binary Color) and layer styles (stroke, shadow, glow).
- Selections (rectangle, lasso, magic wand), transformation, bucket fill, filters, rulers (line,
  ellipse), curves and shapes on linework layers, text layers.
- A library of brushes with your own shapes and textures; brushes from SAI 2, Photoshop, Krita and
  Clip Studio can be imported.

**Animate**
- A timeline with animation folders and cels, onion skin, playback, and export to GIF, APNG, PNG
  frames and MP4.
- **Puppet** animation like After Effects: pin a picture and drag it, the whole mesh follows.

**Work with 3D**
- 3D layers from glTF, VRM, OBJ, FBX, PMX and COLLADA models: poses, camera and conversion to
  lines for tracing.

**Make the window yours**
- Panels that dock like in Clip Studio, workspaces, command search (Ctrl+Shift+P), a quick-access
  bar, your own shortcuts and pen buttons, auto actions.

**Exchange files**
- Reads and writes `.sai2` (SAI 2 opens these files too), PSD and PSB with layers, OpenRaster, PNG,
  JPEG, WebP, TIFF, BMP, TGA and QOI.

## What is specific to the Android build

- **The full interface, not a viewer.** `paint-app` runs inside `android.app.NativeActivity`
  (Rust + winit + egui), so brushes, layers, the timeline, 3D and settings are all there.
- **Mobile layout:** fullscreen with correct system insets, adaptive panels, UI scale 75–250%
  independent of the system setting.
- **Stylus:** pressure from `MotionEvent.getPressure()`, tilt and orientation from `MotionEvent`
  (tested with a Lenovo Precision Pen 3). Pressure debugging in logcat:
  `GrazySAI: paint_app: давление: force=...`.
- **Files:** the built-in FileViewer works with `/sdcard` (`Download`, `Pictures`, `Documents`,
  `DCIM`) through `MANAGE_EXTERNAL_STORAGE` on Android 11+. Without it only the app folder is
  reachable.

## Limitations

- The APK is currently signed with an Android SDK **debug key**. Install updates only over builds
  signed with the same key; a store-ready signature comes later.
- Only `arm64-v8a` is built (all current phones and tablets). `armeabi-v7a` and `x86_64` are not
  shipped.
- Version 0.1.0 is early: the mobile UI is still being polished and crashes are possible. Please
  report in [Issues](https://github.com/avsiel/GrazySAI-on-android/issues); a short description and,
  if possible, the file that shows the problem help most.

## Building the APK

You need Rust with `cargo-ndk`, the Android SDK (build-tools, platform android-34) and the NDK.
The script does the rest:

```powershell
.\android\build_apk.ps1                    # arm64-v8a, signed with the debug key
.\android\build_apk.ps1 -SkipRust          # repackage already built libraries
```

It runs `cargo ndk build --release -p paint-android-full`, then `aapt2` → `zipalign` →
`apksigner`, and leaves `GrazySAI-android.apk` in the repository root.

Architecture details (which crates, why `winit`, `rfd` and `arboard` are patched) are in
[`android/README.md`](android/README.md).

## Project layout

| Directory | What |
|---|---|
| `crates/paint-core` | Pixels, tiles, layers, blending, brush engine, strokes, vector shapes, resampling, animation, puppet |
| `crates/paint-formats` | `.sai2`, PSD, OpenRaster and image reading and writing |
| `crates/paint-app` | The editor window (egui), translations in `lang/` |
| `crates/paint-android-full` | `cdylib` with `android_main` for the APK |
| `crates/paint-3d` | 3D models: reading, poses, rendering, export to GLB and OBJ |
| `crates/paint-studio`, `paint-convert`, `paint-cli` | Brush workshop, converter, command line (desktop) |
| `android/` | Manifest, resources, `build_apk.ps1`, port documentation |
| `vendor/` | Patched `winit`, `rfd`, `arboard` for Android |
| `docs/` | Plan, `.sai2` format, the rules of the program, manual checks |
| `tools/` | Desktop release and checks |

## Related repositories

- [grazyping/GrazySAI](https://github.com/grazyping/GrazySAI) — the desktop program (Windows, Linux,
  FreeBSD) and the user manual.
- This repository is the Android port: [avsiel/GrazySAI-on-android](https://github.com/avsiel/GrazySAI-on-android).

## License

Same as GrazySAI — see [LICENSE](LICENSE). The program is free software; the `.sai2` format was
worked out from users' files, and addresses from the SAI binary never enter the public repository.
