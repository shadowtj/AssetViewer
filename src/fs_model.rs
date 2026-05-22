use anyhow::Context;
use std::cmp::Ordering;
use std::fs;
use std::path::{Path, PathBuf};

/// Action requested from folder tree context menu.
#[derive(Debug, Clone)]
pub enum FolderAction {
    NewFolder(PathBuf),
    Rename(PathBuf),
    Delete(PathBuf),
    Properties(PathBuf),
}

#[derive(Debug, Clone)]
pub struct FsEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub size: u64,
    pub modified: Option<std::time::SystemTime>,
    pub extension: String,
}

impl FsEntry {
    pub fn path(&self) -> &Path {
        &self.path
    }
}

pub fn list_dir_entries(dir: &Path) -> anyhow::Result<Vec<FsEntry>> {
    let mut out = Vec::new();
    for entry in fs::read_dir(dir).with_context(|| format!("Reading dir: {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        let meta = entry.metadata()?;
        let name = entry.file_name().to_string_lossy().to_string();
        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        out.push(FsEntry {
            name,
            path,
            is_dir: meta.is_dir(),
            size: meta.len(),
            modified: meta.modified().ok(),
            extension,
        });
    }

    // Default sort: dirs first, then by name
    out.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });

    Ok(out)
}

#[derive(Debug, Clone)]
pub struct DirNode {
    pub path: PathBuf,
    pub name: String,
    pub children: Vec<DirNode>,
    pub is_expanded: bool,
    pub has_loaded: bool,
}

impl DirNode {
    pub fn new(path: PathBuf, name: String) -> Self {
        Self {
            path,
            name,
            children: Vec::new(),
            is_expanded: false,
            has_loaded: false,
        }
    }

    pub fn this_pc() -> Self {
        let mut node = Self::new(PathBuf::from("this_pc"), "Deze pc".to_string());
        node.has_loaded = true;
        node.is_expanded = true;

        #[cfg(target_os = "windows")]
        {
            let drive_labels = get_drive_labels();
            for letter in b'A'..=b'Z' {
                let letter_char = letter as char;
                let drive_path = PathBuf::from(format!("{}:\\", letter_char));
                if drive_path.exists() {
                    let drive_id = format!("{}:", letter_char);
                    let name = if let Some(label) = drive_labels.get(&drive_id) {
                        if label.is_empty() {
                            drive_id
                        } else {
                            format!("{} ({})", label, drive_id)
                        }
                    } else {
                        drive_id
                    };
                    node.children.push(Self::new(drive_path.clone(), name));
                }
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            node.children
                .push(Self::new(PathBuf::from("/"), "/".to_string()));
        }

        node
    }

    pub fn load_children(&mut self) {
        if self.has_loaded || self.path == PathBuf::from("this_pc") {
            return;
        }

        if let Ok(entries) = std::fs::read_dir(&self.path) {
            let mut kids = Vec::new();
            for entry in entries.flatten() {
                if let Ok(meta) = entry.metadata() {
                    if meta.is_dir() {
                        let path = entry.path();
                        let name = path
                            .file_name()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_else(|| path.to_string_lossy().to_string());
                        kids.push(Self::new(path, name));
                    }
                }
            }
            kids.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
            self.children = kids;
        }
        self.has_loaded = true;
    }

    pub fn ui_render(
        &mut self,
        ui: &mut egui::Ui,
        on_select: &mut dyn FnMut(&Path),
        selected_dir: &Path,
        actions: &mut Vec<FolderAction>,
    ) {
        let is_selected = self.path == selected_dir;
        let is_this_pc = self.path == PathBuf::from("this_pc");

        let label = if is_this_pc {
            "🖥 Deze pc".to_string()
        } else if is_selected {
            format!("📁 {}", self.name)
        } else {
            format!("  {}", self.name)
        };

        let header = egui::CollapsingHeader::new(label)
            .id_source(&self.path)
            .default_open(self.is_expanded);

        let response = header.show(ui, |ui| {
            if !self.has_loaded {
                self.load_children();
            }
            for child in &mut self.children {
                child.ui_render(ui, on_select, selected_dir, actions);
            }
        });

        // Context menu on folder nodes (not "This PC")
        if !is_this_pc {
            let path = self.path.clone();
            response.header_response.context_menu(|ui| {
                if ui.button("📁 Nieuwe map").clicked() {
                    actions.push(FolderAction::NewFolder(path.clone()));
                    ui.close_menu();
                }
                if ui.button("✏ Hernoemen").clicked() {
                    actions.push(FolderAction::Rename(path.clone()));
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("🗑 Verwijderen").clicked() {
                    actions.push(FolderAction::Delete(path.clone()));
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("ℹ Eigenschappen").clicked() {
                    actions.push(FolderAction::Properties(path.clone()));
                    ui.close_menu();
                }
            });
        }

        if response.header_response.clicked() && !is_this_pc {
            on_select(&self.path);
        }
    }

    /// Reload children after a filesystem change (new folder, rename, delete).
    pub fn reload_children(&mut self) {
        self.has_loaded = false;
        self.children.clear();
        self.load_children();
    }

    /// Recursively find the node for a given path and reload its children.
    pub fn reload_at(&mut self, target: &Path) {
        if self.path == target {
            self.reload_children();
            return;
        }
        for child in &mut self.children {
            if target.starts_with(&child.path) {
                child.reload_at(target);
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::export_asset;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new(name: &str) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!("assetviewer_{name}_{nonce}"));
            fs::create_dir_all(&path).unwrap();
            Self { path }
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn export_asset_copies_primary_sidecars_and_nested_textures() {
        let source = TempDir::new("export_source_nested");
        let target = TempDir::new("export_target_nested");
        let asset = source.path.join("ship.obj");
        fs::write(&asset, "obj").unwrap();
        fs::write(source.path.join("ship.mtl"), "mtl").unwrap();
        fs::write(source.path.join("ship_diffuse.png"), "png").unwrap();
        fs::create_dir(source.path.join("textures")).unwrap();
        fs::write(
            source.path.join("textures").join("ship_normal.png"),
            "normal",
        )
        .unwrap();

        export_asset(&asset, &target.path, false).unwrap();

        assert!(target.path.join("ship.obj").exists());
        assert!(target.path.join("ship.mtl").exists());
        assert!(target.path.join("ship_diffuse.png").exists());
        assert!(target
            .path
            .join("textures")
            .join("ship_normal.png")
            .exists());
    }

    #[test]
    fn export_asset_flattens_nested_textures_when_requested() {
        let source = TempDir::new("export_source_flat");
        let target = TempDir::new("export_target_flat");
        let asset = source.path.join("crate.gltf");
        fs::write(&asset, "gltf").unwrap();
        fs::create_dir(source.path.join("textures")).unwrap();
        fs::write(source.path.join("textures").join("crate_color.jpg"), "jpg").unwrap();

        export_asset(&asset, &target.path, true).unwrap();

        assert!(target.path.join("crate.gltf").exists());
        assert!(target.path.join("crate_color.jpg").exists());
        assert!(!target
            .path
            .join("textures")
            .join("crate_color.jpg")
            .exists());
    }
}

/// Export asset with dependency discovery.
/// Scans for textures and sidecar files based on the primary asset stem.
pub fn export_asset(file: &Path, target_dir: &Path, flatten: bool) -> anyhow::Result<()> {
    fs::create_dir_all(target_dir)
        .with_context(|| format!("Creating export dir: {}", target_dir.display()))?;

    let file_name = file
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("Invalid filename"))?;
    let dest = target_dir.join(file_name);
    fs::copy(file, &dest)
        .with_context(|| format!("Copying {} -> {}", file.display(), dest.display()))?;

    let parent = file.parent().unwrap_or_else(|| Path::new("."));
    let stem = file.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    if stem.is_empty() {
        return Ok(());
    }

    // Dependency discovery:
    let sidecar_exts = [
        "png", "jpg", "jpeg", "tga", "tif", "tiff", "exr", "hdr", "mtl", "json", "txt", "bin",
        "dds",
    ];

    // Direct sidecars
    for ext in &sidecar_exts {
        let side = parent.join(format!("{stem}.{ext}"));
        if side.exists() && side != file {
            let dest_side = target_dir.join(side.file_name().unwrap());
            let _ = fs::copy(&side, &dest_side);
        }

        // Suffix matches like _diffuse, _normal, etc.
        let suffixes = [
            "_diff", "_diffuse", "_n", "_normal", "_rough", "_r", "_metal", "_m", "_ao", "_spec",
            "_s", "_col", "_color",
        ];
        for suffix in suffixes {
            let side = parent.join(format!("{stem}{suffix}.{ext}"));
            if side.exists() {
                let dest_side = target_dir.join(side.file_name().unwrap());
                let _ = fs::copy(&side, &dest_side);
            }
        }
    }

    // Subfolder textures
    for sub in &["textures", "maps", "tex", "images"] {
        let sub_dir = parent.join(sub);
        if sub_dir.is_dir() {
            if let Ok(entries) = fs::read_dir(sub_dir) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
                    if name.to_lowercase().contains(&stem.to_lowercase()) {
                        let final_dest_dir = if flatten {
                            target_dir.to_path_buf()
                        } else {
                            let d = target_dir.join(sub);
                            fs::create_dir_all(&d).ok();
                            d
                        };
                        let dest_file = final_dest_dir.join(entry.file_name());
                        let _ = fs::copy(&p, &dest_file);
                    }
                }
            }
        }
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn get_drive_labels() -> std::collections::HashMap<String, String> {
    let mut labels = std::collections::HashMap::new();
    let output = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "Get-CimInstance Win32_LogicalDisk | ForEach-Object { $_.DeviceID + '|' + $_.VolumeName }",
        ])
        .output();

    if let Ok(output) = output {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            let line = line.trim();
            if let Some((id, label)) = line.split_once('|') {
                // id is like "C:", label might be empty
                labels.insert(id.to_uppercase(), label.trim().to_string());
            }
        }
    }
    labels
}
