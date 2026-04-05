# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Run

```bash
cargo build           # debug build
cargo build --release # release build
cargo run             # run in dev mode
cargo clippy          # lint
```

No test suite is currently present. `scripts/run_dev.bat` is a convenience wrapper around `cargo run`.

## Architecture

Asset Viewer is a **three-panel Windows desktop file browser** for previewing creative assets (images, audio, 3D models, video, sprites, archives). Built with `egui`/`eframe` (immediate-mode UI).

### Module Overview

| File | Role |
|------|------|
| `main.rs` | Entry point — creates `eframe` window, instantiates `AssetViewerApp` |
| `app.rs` | Core state + UI orchestration: top bar, left folder tree, center file list, right preview panel |
| `config.rs` | JSON config persistence via `directories` crate (`AppData\Roaming\Dev_Row\AssetViewer\config.json`) |
| `fs_model.rs` | File system abstraction (`FsEntry`, `DirNode`), Windows drive enumeration via PowerShell, dependency-aware asset export |
| `preview.rs` | All preview types (image, audio, video, 3D, blend, sprite, zip, text) behind a `PreviewUi` trait + `PreviewCache` |

### Data Flow

1. User navigates folder tree (`DirNode` in `fs_model.rs`) → `app.rs` calls `select_dir()` → refreshes `filtered_entries`
2. User clicks a file → `select_file()` → `PreviewCache::get_or_load()` detects type by extension → returns boxed `PreviewUi`
3. Preview renders in right panel each frame (immediate-mode); audio/video state is owned inside the preview structs
4. On root/favorites change, config is serialized and saved immediately

### Preview Types & Dependencies

- **Image** — `egui_extras::RetainedImage`, mouse-wheel zoom
- **Audio** — `symphonia` decode + `rodio` playback; waveform peaks cached to disk (`AppData\Local\Dev_Row\AssetViewer\Cache\audio_peaks\`)
- **Video** — requires `ffprobe`/`ffmpeg` in PATH; extracts frames via CLI subprocess (blocking)
- **Model3D** — `tobj` for OBJ, `easy_gltf` for GLTF/GLB; shows mesh/material stats
- **Blend** — headless Blender executable required; renders thumbnail via Python script (blocking)
- **Sprite** — custom BitWise engine `.sprite` JSON format with animated playback
- **Zip** — `zip` crate, hierarchical tree view
- **Text** — read-only `egui::TextEdit`

### Key Design Notes

- **Synchronous I/O**: Blender and ffmpeg calls block the UI thread. No async runtime is used intentionally (see `codex_prompt.md`: "avoid async until UX baseline solid").
- **Windows-first**: Drive listing uses PowerShell; paths assume Windows separators.
- **Export logic** (`fs_model.rs::export_asset`): Copies the primary file plus sidecar discovery (`.mtl`, `_diffuse.png`, `textures/` subdirs, etc.). Optional flatten flag.
- **Favorites**: Stored in config, auto-deduplicated with `(2)` suffix pattern.
- **Thumbnail cache**: In-memory per session; audio peak cache is persisted to disk by file hash.
