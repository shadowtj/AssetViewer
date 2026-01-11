# Codex CLI Prompt (local)

You are working on a Rust desktop app in the folder:
`asset_viewer\`

Constraints:
- Make changes only inside this repository.
- Prefer simple, production-ready code (no mock data).
- Keep the app compiling on Windows.
- Do NOT introduce internet calls or API keys.
- Implement features incrementally and keep existing behavior intact.

Task:
1) Ensure the project builds and runs: `cargo run`.
2) Improve the center file list:
   - add a toggle for "List" vs "Grid"
   - in Grid mode show thumbnails for images (cached in memory for now)
3) Add a search box (filters current folder entries by substring).
4) Add a file type filter dropdown: All / Images / Audio / Video / 3D / Other.
5) Add a right-click context menu on file entries:
   - Copy full path to clipboard
   - Open containing folder (Windows Explorer)
6) Keep the right preview panel as-is (image + audio waveform), but add a "Clear Preview Cache" button in Settings.

Deliverables:
- Update the existing files in `asset_viewer\src\`
- Update `Cargo.toml` only if needed
- Keep changes minimal and readable
