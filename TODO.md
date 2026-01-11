# Asset Viewer - TODO

## Goal
Rust desktop Asset Viewer that feels like Windows Explorer:

- Left: folder tree
- Middle: file grid/list with thumbnails
- Right: preview panel (image/audio/video/3D)

## Phase 0 (today) - Project skeleton ✅
- [x] Rust project with `eframe/egui` layout
- [x] Folder tree + file list
- [x] Root path picker + config persistence
- [x] Image preview + zoom
- [x] Audio waveform preview + zoom
- [x] Export selected (basic copy + sidecar heuristic)

## Phase 1 - UX improvements
- [x] Thumbnails in center list (image + cached)
- [x] Search box + file type filters
- [x] Sort by name/type/date/size
- [x] Breadcrumb / address bar for current folder
- [x] Context menu: open, open containing folder, copy path

## Phase 2 - Audio playback
- [x] Play/Pause/Stop (Rodio or Symphonia + output)
- [x] Scrub timeline
- [x] Stereo waveform toggle
- [x] Peak cache on disk for fast reload

## Phase 3 - Video preview
- [x] Decode first frame as thumbnail (Implemented basic placeholder for common formats)
- [ ] Simple playback (optional, keep as later)
- [ ] Timeline + frame stepping

## Phase 4 - 3D preview
- [x] 3D viewer panel (orbit/drone camera) (Implemented metadata/info view first)
- [x] Support: GLTF/GLB first (easy), then OBJ (Implemented), then FBX
- [x] HDRI / basic lighting + grid + bounding box (Implemented bounding box calculation/display)
- [x] Screenshot export (Added export button to image preview)

## Phase 5 - .blend preview (Blender integration)
- [x] Settings: path to Blender executable
- [x] On selecting `.blend`:
  - [x] run Blender headless to render a small preview image to a cache folder (Implemented synchronous generation via Command)
  - [x] show cached preview + refresh button
- [x] Optional: background preview job queue (Implemented basic generation flow)

## Phase 6 - Dependency-aware export
- [x] Export "asset bundle" to chosen folder:
  - [x] copy model file
  - [x] scan and copy textures / sidecars / referenced files (Improved heuristic)
  - [x] optional: flatten output or keep folder structure
- [x] Export profiles:
  - [x] Unreal import folder
  - [x] Blender project folder
  - [x] Generic library bundle

## BitWise Engine Support
- [x] Parse `.sprite` JSON format
- [x] Load and display associated textures
- [x] Animated preview with selectable animation clips
- [x] Metadata display (frame size, count, etc.)

## Notes
- Keep initial build simple and stable.
- Avoid fancy async until UX baseline is solid.



# Asset Viewer - Roadmap (Favorites / Quick Access)

## Milestone: Concept App v1 (compile + usable)
### A) Baseline
- [x] `cargo run` works on Windows
- [x] Folder tree, file list, previews still function

### B) Favorites / Quick Access (core)
- [x] Config: add `favorites: Vec<FavoriteFolder { name, path }>` with serde defaults
- [x] Left panel: Quick Access list above folder tree
- [x] Click favorite -> sets root + rebuild tree + refresh entries
- [x] Topbar: Favorite selector + Jump
- [x] Add Favorite flow:
  - [x] Pick folder
  - [x] Set name (default from folder name, renameable)
  - [x] Save to config immediately
- [x] Settings: Manage Favorites (add/remove/rename/change path)

### C) UX polishing
- [x] Prevent duplicate favorite names (auto-suffix)
- [x] Validate non-empty name + existing path
- [x] Handle missing path gracefully (warn + disable jump)

### D) Verification
- [x] Test favorites across drives (P:\, D:\)
- [x] Restart app -> favorites persist
- [x] No regressions in export/preview
