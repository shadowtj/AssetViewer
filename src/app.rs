#![allow(deprecated)]

use crate::config::{AppConfig, ConfigStore};
use crate::fs_model::{DirNode, FolderAction, FsEntry};
use crate::preview::{PreviewCache, PreviewKind, PreviewUi};
use egui::{CentralPanel, Context, Id, Layout, RichText, SidePanel, TopBottomPanel};
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub struct FolderProperties {
    pub total_size: u64,
    pub file_count: usize,
    pub folder_count: usize,
    pub empty_folders: Vec<PathBuf>,
}

impl FolderProperties {
    pub fn scan(path: &Path) -> Self {
        let mut total_size = 0u64;
        let mut file_count = 0usize;
        let mut folder_count = 0usize;
        let mut empty_folders = Vec::new();

        for entry in walkdir::WalkDir::new(path).min_depth(1) {
            let Ok(entry) = entry else { continue };
            if entry.file_type().is_dir() {
                folder_count += 1;
                // Check if this dir has any children
                let has_children = std::fs::read_dir(entry.path())
                    .map(|mut rd| rd.next().is_some())
                    .unwrap_or(false);
                if !has_children {
                    empty_folders.push(entry.path().to_path_buf());
                }
            } else {
                file_count += 1;
                total_size += entry.metadata().map(|m| m.len()).unwrap_or(0);
            }
        }

        Self {
            total_size,
            file_count,
            folder_count,
            empty_folders,
        }
    }
}

fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} bytes", bytes)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortMode {
    Name,
    Type,
    Date,
    Size,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    List,
    Grid,
}

// Collects UI actions from the center file list to avoid borrow issues in immediate-mode loops.
#[derive(Default)]
struct CenterListActions {
    clicked_dir: Option<PathBuf>,
    clicked_file: Option<PathBuf>,
    open_explorer: Option<PathBuf>,
    copy_path: Option<String>,
    rename_entry: Option<PathBuf>,
    delete_entry: Option<PathBuf>,
    new_folder_in: Option<PathBuf>,
    properties_entry: Option<PathBuf>,
}

fn populate_entry_context_menu(ui: &mut egui::Ui, entry: &crate::fs_model::FsEntry, actions: &mut CenterListActions) {
    if ui.button("Open").clicked() {
        if entry.is_dir {
            actions.clicked_dir = Some(entry.path().to_path_buf());
        } else {
            actions.clicked_file = Some(entry.path().to_path_buf());
        }
        ui.close_menu();
    }
    if ui.button("Open in Explorer").clicked() {
        actions.open_explorer = Some(entry.path().to_path_buf());
        ui.close_menu();
    }
    if ui.button("Copy Path").clicked() {
        actions.copy_path = Some(entry.path().to_string_lossy().to_string());
        ui.close_menu();
    }
    ui.separator();
    if entry.is_dir && ui.button("📁 Nieuwe map").clicked() {
        actions.new_folder_in = Some(entry.path().to_path_buf());
        ui.close_menu();
    }
    if ui.button("✏ Hernoemen").clicked() {
        actions.rename_entry = Some(entry.path().to_path_buf());
        ui.close_menu();
    }
    if ui.button("🗑 Verwijderen").clicked() {
        actions.delete_entry = Some(entry.path().to_path_buf());
        ui.close_menu();
    }
    if entry.is_dir {
        ui.separator();
        if ui.button("ℹ Eigenschappen").clicked() {
            actions.properties_entry = Some(entry.path().to_path_buf());
            ui.close_menu();
        }
    }
}

fn handle_entry_click(entry: &crate::fs_model::FsEntry, response: &egui::Response, actions: &mut CenterListActions) {
    if response.clicked() {
        if entry.is_dir {
            actions.clicked_dir = Some(entry.path().to_path_buf());
        } else {
            actions.clicked_file = Some(entry.path().to_path_buf());
        }
    }
    if response.double_clicked() && entry.is_dir {
        actions.clicked_dir = Some(entry.path().to_path_buf());
    }
}

pub struct AssetViewerApp {
    config_store: ConfigStore,
    config: AppConfig,

    root_path: PathBuf,
    tree: DirNode,
    selected_dir: PathBuf,
    dir_history: Vec<PathBuf>,
    dir_history_index: usize,
    entries: Vec<FsEntry>,
    filtered_entries: Vec<FsEntry>,
    selected_file: Option<PathBuf>,

    preview_cache: PreviewCache,
    show_settings: bool,

    // Favorites UI state
    show_add_favorite_modal: bool,
    new_favorite_name: String,
    new_favorite_path: PathBuf,

    // Phase 1 UI state
    search_query: String,
    address_bar: String,
    sort_mode: SortMode,
    sort_ascending: bool,
    view_mode: ViewMode,
    thumbnail_cache: std::collections::HashMap<PathBuf, egui_extras::RetainedImage>,
    thumbnail_rx: Option<std::sync::mpsc::Receiver<(PathBuf, Vec<u8>, String)>>,
    cache_status: Option<String>,

    // Audio output
    _audio_stream: Option<(rodio::OutputStream, rodio::OutputStreamHandle)>,

    // Folder/file management dialogs
    folder_new_parent: Option<PathBuf>,
    folder_new_name: String,
    folder_rename_path: Option<PathBuf>,
    folder_rename_name: String,
    folder_delete_path: Option<PathBuf>,
    // File management (center list)
    file_rename_path: Option<PathBuf>,
    file_rename_name: String,
    file_delete_path: Option<PathBuf>,
    // Properties dialog
    properties_path: Option<PathBuf>,
    properties_info: Option<FolderProperties>,
    properties_scan_rx: Option<std::sync::mpsc::Receiver<FolderProperties>>,

    // Error feedback for file/folder operations
    op_error: Option<String>,

    // Stored context for background threads
    ctx: egui::Context,
}

impl AssetViewerApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Egui image loaders for egui_extras::RetainedImage
        egui_extras::install_image_loaders(&cc.egui_ctx);

        let config_store = ConfigStore::new().unwrap_or_default();
        let config = config_store.load().unwrap_or_default();

        let root_path = config
            .root_path
            .clone()
            .map(PathBuf::from)
            .filter(|p| p.exists())
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

        let tree = DirNode::this_pc();
        let selected_dir = root_path.clone();
        let entries = crate::fs_model::list_dir_entries(&selected_dir).unwrap_or_default();
        let address_bar = selected_dir.to_string_lossy().to_string();

        let audio_stream = rodio::OutputStream::try_default().ok();

        let sort_mode = match config.sort_mode.as_str() {
            "Type" => SortMode::Type,
            "Date" => SortMode::Date,
            "Size" => SortMode::Size,
            _ => SortMode::Name,
        };
        let sort_ascending = config.sort_ascending;
        let view_mode = if config.view_mode == "Grid" { ViewMode::Grid } else { ViewMode::List };

        let mut app = Self {
            config_store,
            config,
            root_path,
            tree,
            selected_dir: selected_dir.clone(),
            dir_history: vec![selected_dir.clone()],
            dir_history_index: 0,
            entries,
            filtered_entries: Vec::new(),
            selected_file: None,
            preview_cache: PreviewCache::default(),
            show_settings: false,
            show_add_favorite_modal: false,
            new_favorite_name: String::new(),
            new_favorite_path: PathBuf::new(),
            search_query: String::new(),
            address_bar,
            sort_mode,
            sort_ascending,
            view_mode,
            thumbnail_cache: std::collections::HashMap::new(),
            thumbnail_rx: None,
            cache_status: None,
            _audio_stream: audio_stream,
            folder_new_parent: None,
            folder_new_name: String::new(),
            folder_rename_path: None,
            folder_rename_name: String::new(),
            folder_delete_path: None,
            file_rename_path: None,
            file_rename_name: String::new(),
            file_delete_path: None,
            properties_path: None,
            properties_info: None,
            properties_scan_rx: None,
            op_error: None,
            ctx: cc.egui_ctx.clone(),
        };
        app.load_thumbnails();
        app.apply_filters();
        app
    }

    fn set_root_path(&mut self, new_root: PathBuf) {
        self.root_path = new_root;
        self.selected_dir = self.root_path.clone();
        self.address_bar = self.selected_dir.to_string_lossy().to_string();
        self.dir_history = vec![self.selected_dir.clone()];
        self.dir_history_index = 0;
        self.refresh_selected_dir();
        self.selected_file = None;

        self.config.root_path = Some(self.root_path.to_string_lossy().to_string());
        self.save_config();
    }

    fn jump_to_favorite(&mut self, index: usize) {
        if let Some(fav) = self.config.favorites.get(index) {
            let path = PathBuf::from(&fav.path);
            if path.exists() {
                self.config.last_favorite_index = Some(index);
                self.set_root_path(path);
            }
        }
    }

    fn save_config(&self) {
        let _ = self.config_store.save(&self.config);
    }

    fn start_properties_scan(&mut self, path: PathBuf) {
        self.properties_path = Some(path.clone());
        self.properties_info = None;
        let (tx, rx) = std::sync::mpsc::channel();
        self.properties_scan_rx = Some(rx);
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let props = FolderProperties::scan(&path);
            let _ = tx.send(props);
            ctx.request_repaint();
        });
    }

    fn unique_favorite_name(
        existing: &[crate::config::FavoriteFolder],
        name: &str,
        path: &str,
    ) -> String {
        let mut name = name.trim().to_string();
        if name.is_empty() {
            name = Path::new(&path)
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "New Favorite".to_string());
        }

        let mut final_name = name.clone();
        let mut count = 2;
        while existing.iter().any(|f| f.name == final_name) {
            final_name = format!("{} ({})", name, count);
            count += 1;
        }
        final_name
    }

    fn add_favorite(&mut self, name: String, path: String) {
        let final_name = Self::unique_favorite_name(&self.config.favorites, &name, &path);
        self.config.favorites.push(crate::config::FavoriteFolder {
            name: final_name,
            path,
        });
        self.save_config();
    }

    fn refresh_selected_dir(&mut self) {
        self.entries = crate::fs_model::list_dir_entries(&self.selected_dir).unwrap_or_default();
        // Evict thumbnails that belong to a different directory
        let current_paths: std::collections::HashSet<&PathBuf> =
            self.entries.iter().map(|e| &e.path).collect();
        self.thumbnail_cache.retain(|k, _| current_paths.contains(k));
        self.load_thumbnails();
        self.apply_filters();
    }

    fn load_thumbnails(&mut self) {
        // Cancel any in-flight thumbnail load for the previous directory
        self.thumbnail_rx = None;

        let paths_to_load: Vec<(PathBuf, String)> = self
            .entries
            .iter()
            .filter(|e| {
                !e.is_dir
                    && !self.thumbnail_cache.contains_key(&e.path)
                    && crate::preview::PreviewKind::from_path(&e.path)
                        == crate::preview::PreviewKind::Image
            })
            .map(|e| (e.path.clone(), e.name.clone()))
            .collect();

        if paths_to_load.is_empty() {
            return;
        }

        let (tx, rx) = std::sync::mpsc::channel::<(PathBuf, Vec<u8>, String)>();
        self.thumbnail_rx = Some(rx);

        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            for (path, name) in paths_to_load {
                if let Ok(bytes) = std::fs::read(&path) {
                    if tx.send((path, bytes, name)).is_err() {
                        break;
                    }
                    ctx.request_repaint();
                }
            }
        });
    }

    fn apply_filters(&mut self) {
        let query = self.search_query.to_lowercase();
        let query_parts: Vec<&str> = query.split_whitespace().collect();

        let mut filtered: Vec<FsEntry> = self
            .entries
            .iter()
            .filter(|e| {
                if query_parts.is_empty() {
                    true
                } else {
                    // If it's a multi-part query (like extension filters), check if any part matches extension
                    // or if all parts are contained in the name (standard search)
                    let name_lower = e.name.to_lowercase();
                    if query_parts.len() > 1 && query_parts.iter().any(|&p| e.extension == p) {
                        true
                    } else {
                        query_parts
                            .iter()
                            .all(|&p| name_lower.contains(p) || e.extension == p)
                    }
                }
            })
            .cloned()
            .collect();

        filtered.sort_by(|a, b| {
            // Directories always come first for name/type sorts
            if matches!(self.sort_mode, SortMode::Name | SortMode::Type) {
                match (a.is_dir, b.is_dir) {
                    (true, false) => return std::cmp::Ordering::Less,
                    (false, true) => return std::cmp::Ordering::Greater,
                    _ => {}
                }
            }
            let ord = match self.sort_mode {
                SortMode::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                SortMode::Type => a.extension.cmp(&b.extension),
                SortMode::Date => a.modified.cmp(&b.modified),
                SortMode::Size => a.size.cmp(&b.size),
            };
            if self.sort_ascending { ord } else { ord.reverse() }
        });

        self.filtered_entries = filtered;
    }

    fn select_dir(&mut self, dir: PathBuf) {
        self.selected_dir = dir;
        self.address_bar = self.selected_dir.to_string_lossy().to_string();
        self.selected_file = None;
        if self
            .dir_history
            .get(self.dir_history_index)
            .map(|current| current != &self.selected_dir)
            .unwrap_or(true)
        {
            self.dir_history.truncate(self.dir_history_index + 1);
            self.dir_history.push(self.selected_dir.clone());
            self.dir_history_index = self.dir_history.len().saturating_sub(1);
        }
        self.refresh_selected_dir();
    }

    fn go_back(&mut self) -> bool {
        if self.dir_history_index == 0 {
            return false;
        }
        self.dir_history_index -= 1;
        if let Some(dir) = self.dir_history.get(self.dir_history_index).cloned() {
            self.selected_dir = dir;
            self.address_bar = self.selected_dir.to_string_lossy().to_string();
            self.selected_file = None;
            self.refresh_selected_dir();
            return true;
        }
        false
    }

    fn go_forward(&mut self) -> bool {
        if self.dir_history_index + 1 >= self.dir_history.len() {
            return false;
        }
        self.dir_history_index += 1;
        if let Some(dir) = self.dir_history.get(self.dir_history_index).cloned() {
            self.selected_dir = dir;
            self.address_bar = self.selected_dir.to_string_lossy().to_string();
            self.selected_file = None;
            self.refresh_selected_dir();
            return true;
        }
        false
    }

    fn handle_mouse_navigation(&mut self, ctx: &Context) {
        ctx.input(|i| {
            if i.pointer.button_pressed(egui::PointerButton::Extra1) {
                let _ = self.go_back();
            }
            if i.pointer.button_pressed(egui::PointerButton::Extra2) {
                let _ = self.go_forward();
            }
        });
    }

    fn select_file(&mut self, file: PathBuf) {
        self.selected_file = Some(file);
    }

    fn render_top_bar(&mut self, ctx: &Context) {
        TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Asset Viewer");

                ui.separator();

                if ui.button("Set Root…").clicked() {
                    if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                        self.set_root_path(folder);
                    }
                }

                if ui.button("Refresh").clicked() {
                    self.refresh_selected_dir();
                }

                ui.separator();

                // Favorites Dropdown
                egui::ComboBox::from_id_source("fav_combo")
                    .selected_text(
                        self.config
                            .last_favorite_index
                            .and_then(|i| self.config.favorites.get(i))
                            .map(|f| f.name.as_str())
                            .unwrap_or("Favorites"),
                    )
                    .show_ui(ui, |ui| {
                        for i in 0..self.config.favorites.iter().len() {
                            let name = self.config.favorites[i].name.clone();
                            if ui
                                .selectable_label(self.config.last_favorite_index == Some(i), name)
                                .clicked()
                            {
                                self.jump_to_favorite(i);
                            }
                        }
                    });

                if ui.button("Add Favorite…").clicked() {
                    if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                        self.new_favorite_path = folder;
                        self.new_favorite_name = self
                            .new_favorite_path
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default();
                        self.show_add_favorite_modal = true;
                    }
                }

                ui.separator();

                if ui.button("Export Selected…").clicked() {
                    if let Some(path) = self.selected_file.clone() {
                        if let Some(target_dir) = rfd::FileDialog::new().pick_folder() {
                            match crate::fs_model::export_asset(
                                &path,
                                &target_dir,
                                self.config.flatten_export,
                            ) {
                                Ok(warnings) if !warnings.is_empty() => {
                                    self.op_error = Some(format!(
                                        "Export voltooid met waarschuwingen:\n{}",
                                        warnings.join("\n")
                                    ));
                                }
                                Err(e) => {
                                    self.op_error = Some(format!("Export mislukt: {e:#}"));
                                }
                                _ => {}
                            }
                        }
                    }
                }

                ui.separator();

                if ui.button("Settings").clicked() {
                    self.show_settings = !self.show_settings;
                }
            });

            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("📍");
                let response = ui.add(
                    egui::TextEdit::singleline(&mut self.address_bar).desired_width(f32::INFINITY),
                );
                if response.lost_focus() && (ui.input(|i| i.key_pressed(egui::Key::Enter))) {
                    let new_path = PathBuf::from(&self.address_bar);
                    if new_path.exists() {
                        if new_path.is_dir() {
                            self.select_dir(new_path);
                        } else if let Some(parent) = new_path.parent() {
                            self.select_dir(parent.to_path_buf());
                            self.select_file(new_path);
                        }
                    }
                }
            });
        });
    }

    fn render_left_tree(&mut self, ctx: &Context) {
        SidePanel::left("left_tree")
            .resizable(true)
            .default_width(280.0)
            .show(ctx, |ui| {
                ui.heading("Quick Access");

                let mut jump_to = None;
                if self.config.favorites.is_empty() {
                    ui.label(RichText::new("No favorites yet").small().italics());
                } else {
                    for i in 0..self.config.favorites.len() {
                        let fav = &self.config.favorites[i];
                        let is_active = self.config.last_favorite_index == Some(i)
                            && self.root_path == PathBuf::from(&fav.path);
                        let response = ui.selectable_label(is_active, format!("⭐ {}", fav.name));
                        if response.clicked() {
                            jump_to = Some(i);
                        }
                        response.on_hover_text(&fav.path);
                    }
                }

                if let Some(i) = jump_to {
                    self.jump_to_favorite(i);
                }

                ui.add_space(8.0);

                ui.heading("Folders");
                ui.separator();

                let mut next_selected = None;
                let mut folder_actions = Vec::new();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.tree.ui_render(
                        ui,
                        &mut |selected| {
                            next_selected = Some(selected.to_path_buf());
                        },
                        &self.selected_dir,
                        &mut folder_actions,
                    );
                });

                if let Some(dir) = next_selected {
                    self.select_dir(dir);
                }

                for action in folder_actions {
                    match action {
                        FolderAction::NewFolder(parent) => {
                            self.folder_new_name = "Nieuwe map".to_string();
                            self.folder_new_parent = Some(parent);
                        }
                        FolderAction::Rename(path) => {
                            let name = path
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_default();
                            self.folder_rename_name = name;
                            self.folder_rename_path = Some(path);
                        }
                        FolderAction::Delete(path) => {
                            self.folder_delete_path = Some(path);
                        }
                        FolderAction::Properties(path) => {
                            self.start_properties_scan(path);
                        }
                    }
                }
            });
    }

    fn render_center_list(&mut self, ctx: &Context) {
        CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Files");
                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(
                            egui::TextEdit::singleline(&mut self.search_query)
                                .hint_text("Search..."),
                        )
                        .changed()
                    {
                        self.apply_filters();
                    }
                    ui.label("🔍");
                });
            });

            ui.horizontal(|ui| {
                ui.label("Sort:");
                for (label, mode, key) in [
                    ("Name", SortMode::Name, "Name"),
                    ("Type", SortMode::Type, "Type"),
                    ("Size", SortMode::Size, "Size"),
                    ("Date", SortMode::Date, "Date"),
                ] {
                    if ui.selectable_label(self.sort_mode == mode, label).clicked() {
                        if self.sort_mode == mode {
                            self.sort_ascending = !self.sort_ascending;
                        } else {
                            self.sort_mode = mode;
                            self.sort_ascending = true;
                        }
                        self.config.sort_mode = key.to_string();
                        self.config.sort_ascending = self.sort_ascending;
                        self.save_config();
                        self.apply_filters();
                    }
                }

                ui.separator();
                ui.label("View:");
                if ui.selectable_label(self.view_mode == ViewMode::List, "☰ List").clicked() {
                    self.view_mode = ViewMode::List;
                    self.config.view_mode = "List".to_string();
                    self.save_config();
                }
                if ui.selectable_label(self.view_mode == ViewMode::Grid, "▦ Grid").clicked() {
                    self.view_mode = ViewMode::Grid;
                    self.config.view_mode = "Grid".to_string();
                    self.save_config();
                }
            });

            ui.separator();

            let mut actions = CenterListActions::default();

            egui::ScrollArea::vertical().show(ui, |ui| {
                if self.view_mode == ViewMode::List {
                    for entry in &self.filtered_entries {
                        let is_selected = self
                            .selected_file
                            .as_ref()
                            .map(|p| p == entry.path())
                            .unwrap_or(false);

                        let label = if entry.is_dir {
                            format!("📁 {}", entry.name)
                        } else {
                            format!("📄 {}", entry.name)
                        };

                        let response = ui.selectable_label(is_selected, label);
                        response.context_menu(|ui| populate_entry_context_menu(ui, entry, &mut actions));
                        handle_entry_click(entry, &response, &mut actions);
                    }
                } else {
                    // Grid View
                    ui.horizontal_wrapped(|ui| {
                        let item_size = egui::vec2(100.0, 120.0);
                        for entry in &self.filtered_entries {
                            let is_selected = self
                                .selected_file
                                .as_ref()
                                .map(|p| p == entry.path())
                                .unwrap_or(false);

                            let (rect, response) =
                                ui.allocate_at_least(item_size, egui::Sense::click());

                            if ui.is_rect_visible(rect) {
                                let visuals =
                                    ui.style().interact_selectable(&response, is_selected);
                                if is_selected || response.hovered() {
                                    ui.painter().rect(
                                        rect.expand(2.0),
                                        4.0,
                                        visuals.bg_fill,
                                        visuals.bg_stroke,
                                    );
                                }

                                let thumb_rect = egui::Rect::from_min_size(
                                    rect.min + egui::vec2(10.0, 5.0),
                                    egui::vec2(80.0, 80.0),
                                );
                                if let Some(thumb) = self.thumbnail_cache.get(&entry.path) {
                                    ui.painter().image(
                                        thumb.texture_id(ctx),
                                        thumb_rect,
                                        egui::Rect::from_min_max(
                                            egui::pos2(0.0, 0.0),
                                            egui::pos2(1.0, 1.0),
                                        ),
                                        egui::Color32::WHITE,
                                    );
                                } else {
                                    let icon = if entry.is_dir { "📁" } else { "📄" };
                                    ui.painter().text(
                                        thumb_rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        icon,
                                        egui::FontId::proportional(40.0),
                                        visuals.fg_stroke.color,
                                    );
                                }

                                let name_rect = egui::Rect::from_min_max(
                                    egui::pos2(rect.min.x, rect.min.y + 90.0),
                                    rect.max,
                                );
                                ui.painter().text(
                                    name_rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    &entry.name,
                                    egui::FontId::proportional(12.0),
                                    visuals.fg_stroke.color,
                                );
                            }

                            response.context_menu(|ui| populate_entry_context_menu(ui, entry, &mut actions));
                            handle_entry_click(entry, &response, &mut actions);
                        }
                    });
                }
            });

            if let Some(dir) = actions.clicked_dir {
                self.select_dir(dir);
            } else if let Some(file) = actions.clicked_file {
                self.select_file(file);
            }

            if let Some(path) = actions.open_explorer {
                #[cfg(target_os = "windows")]
                {
                    let _ = std::process::Command::new("explorer")
                        .arg("/select,")
                        .arg(path.to_string_lossy().to_string())
                        .spawn();
                }
            }

            if let Some(path) = actions.copy_path {
                ui.output_mut(|o| o.copied_text = path);
            }

            if let Some(parent) = actions.new_folder_in {
                self.folder_new_name = "Nieuwe map".to_string();
                self.folder_new_parent = Some(parent);
            }
            if let Some(path) = actions.rename_entry {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                if path.is_dir() {
                    self.folder_rename_name = name;
                    self.folder_rename_path = Some(path);
                } else {
                    self.file_rename_name = name;
                    self.file_rename_path = Some(path);
                }
            }
            if let Some(path) = actions.delete_entry {
                if path.is_dir() {
                    self.folder_delete_path = Some(path);
                } else {
                    self.file_delete_path = Some(path);
                }
            }
            if let Some(path) = actions.properties_entry {
                self.start_properties_scan(path);
            }
        });
    }

    fn render_right_preview(&mut self, ctx: &Context) {
        SidePanel::right("right_preview")
            .resizable(true)
            .default_width(420.0)
            .show(ctx, |ui| {
                ui.heading("Preview");
                ui.separator();

                let Some(path) = self.selected_file.clone() else {
                    ui.label("Select a file to preview.");
                    return;
                };

                ui.label(path.to_string_lossy().to_string());
                ui.separator();

                let kind = PreviewKind::from_path(&path);
                let audio_handle = self._audio_stream.as_ref().map(|(_, h)| h);
                let blender_path = self.config.blender_path.as_deref();
                match self.preview_cache.get_or_build(ctx, &path, kind, audio_handle, blender_path) {
                    Ok(preview) => preview.ui(ui),
                    Err(err) => {
                        ui.colored_label(egui::Color32::from_rgb(255, 120, 120), format!("Preview error: {err:#}"));
                        ui.label("Tip: some previews depend on external tools such as ffmpeg, Blender, or PDFium.");
                    }
                }
            });
    }

    fn render_settings_window(&mut self, ctx: &Context) {
        if !self.show_settings {
            return;
        }

        let mut is_open = self.show_settings;
        let mut root_to_set = None;

        egui::Window::new("Settings")
            .id(Id::new("settings_window"))
            .open(&mut is_open)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label("Root path is saved to a local config file.");
                ui.separator();

                ui.horizontal(|ui| {
                    ui.label("Root:");
                    ui.monospace(self.root_path.to_string_lossy().to_string());
                });

                ui.horizontal(|ui| {
                    if ui.button("Pick Root…").clicked() {
                        if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                            root_to_set = Some(folder);
                        }
                    }
                    if ui.button("Open Root in Explorer").clicked() {
                        #[cfg(target_os = "windows")]
                        {
                            let _ = std::process::Command::new("explorer")
                                .arg(self.root_path.to_string_lossy().to_string())
                                .spawn();
                        }
                    }
                });

                ui.separator();
                ui.heading("Favorites");
                ui.add_space(4.0);

                let mut to_remove = None;

                egui::ScrollArea::vertical()
                    .id_source("fav_settings_scroll")
                    .max_height(200.0)
                    .show(ui, |ui| {
                        for i in 0..self.config.favorites.iter().len() {
                            ui.group(|ui| {
                                ui.horizontal(|ui| {
                                    ui.vertical(|ui| {
                                        ui.text_edit_singleline(&mut self.config.favorites[i].name);
                                        ui.label(
                                            RichText::new(&self.config.favorites[i].path)
                                                .small()
                                                .color(egui::Color32::GRAY),
                                        );
                                    });

                                    ui.with_layout(
                                        Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if ui.button("🗑").on_hover_text("Remove").clicked() {
                                                to_remove = Some(i);
                                            }
                                            if ui
                                                .button("📂")
                                                .on_hover_text("Change Path")
                                                .clicked()
                                            {
                                                if let Some(folder) =
                                                    rfd::FileDialog::new().pick_folder()
                                                {
                                                    self.config.favorites[i].path =
                                                        folder.to_string_lossy().to_string();
                                                }
                                            }
                                        },
                                    );
                                });
                            });
                        }
                    });

                if let Some(i) = to_remove {
                    self.config.favorites.remove(i);
                    self.config.last_favorite_index = None;
                    self.save_config();
                }

                if ui.button("Add New Favorite…").clicked() {
                    if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                        let path = folder.to_string_lossy().to_string();
                        let name = folder
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| "New Favorite".to_string());
                        self.add_favorite(name, path);
                    }
                }

                ui.separator();
                ui.heading("Cache");
                ui.horizontal(|ui| {
                    if ui.button("Clear Preview Cache").clicked() {
                        self.preview_cache.clear();
                        self.thumbnail_cache.clear();
                        self.cache_status = match crate::preview::clear_disk_preview_caches() {
                            Ok(()) => Some("Cache cleared".to_string()),
                            Err(err) => Some(format!("Cache clear failed: {err:#}")),
                        };
                    }
                    if let Some(status) = &self.cache_status {
                        ui.label(status);
                    }
                });

                ui.separator();
                ui.heading("Blender Integration");
                ui.horizontal(|ui| {
                    ui.label("Blender Exe:");
                    let mut path = self.config.blender_path.clone().unwrap_or_default();
                    if ui.text_edit_singleline(&mut path).changed() {
                        self.config.blender_path = Some(path);
                        self.save_config();
                    }
                    if ui.button("...").clicked() {
                        if let Some(file) = rfd::FileDialog::new().pick_file() {
                            self.config.blender_path = Some(file.to_string_lossy().to_string());
                            self.save_config();
                        }
                    }
                });

                ui.separator();
                ui.heading("Export Settings");
                ui.checkbox(
                    &mut self.config.flatten_export,
                    "Flatten textures to root folder",
                );
                ui.horizontal(|ui| {
                    ui.label("Profile:");
                    egui::ComboBox::from_id_source("export_profile")
                        .selected_text(&self.config.export_profile)
                        .show_ui(ui, |ui| {
                            let profiles = ["Generic", "Unreal Engine", "Unity", "Blender"];
                            for p in profiles {
                                if ui
                                    .selectable_label(self.config.export_profile == p, p)
                                    .clicked()
                                {
                                    self.config.export_profile = p.to_string();
                                    self.save_config();
                                }
                            }
                        });
                });

                ui.separator();
                ui.small(
                    "Phase 4 (GPU renderer) and Phase 3 (FFmpeg) are still in active development.",
                );
            });

        self.show_settings = is_open;
        if let Some(root) = root_to_set {
            self.set_root_path(root);
        }
    }
    fn render_folder_new_dialog(&mut self, ctx: &Context) {
        let Some(parent) = self.folder_new_parent.clone() else {
            return;
        };

        let mut open = true;
        egui::Window::new("Nieuwe map")
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(format!("In: {}", parent.display()));
                ui.horizontal(|ui| {
                    ui.label("Naam:");
                    let re = ui.text_edit_singleline(&mut self.folder_new_name);
                    if re.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        let new_path = parent.join(&self.folder_new_name);
                        match std::fs::create_dir(&new_path) {
                            Ok(()) => {
                                self.tree.reload_at(&parent);
                                if self.selected_dir == parent {
                                    self.refresh_selected_dir();
                                }
                            }
                            Err(e) => self.op_error = Some(format!("Kan map niet aanmaken: {e}")),
                        }
                        self.folder_new_parent = None;
                    }
                });
                ui.horizontal(|ui| {
                    if ui.button("Aanmaken").clicked() {
                        let new_path = parent.join(&self.folder_new_name);
                        match std::fs::create_dir(&new_path) {
                            Ok(()) => {
                                self.tree.reload_at(&parent);
                                if self.selected_dir == parent {
                                    self.refresh_selected_dir();
                                }
                            }
                            Err(e) => self.op_error = Some(format!("Kan map niet aanmaken: {e}")),
                        }
                        self.folder_new_parent = None;
                    }
                    if ui.button("Annuleren").clicked() {
                        self.folder_new_parent = None;
                    }
                });
            });
        if !open {
            self.folder_new_parent = None;
        }
    }

    fn render_folder_rename_dialog(&mut self, ctx: &Context) {
        let Some(old_path) = self.folder_rename_path.clone() else {
            return;
        };

        let mut open = true;
        egui::Window::new("Map hernoemen")
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(format!("Map: {}", old_path.display()));
                ui.horizontal(|ui| {
                    ui.label("Nieuwe naam:");
                    let re = ui.text_edit_singleline(&mut self.folder_rename_name);
                    if re.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        self.do_folder_rename(&old_path);
                    }
                });
                ui.horizontal(|ui| {
                    if ui.button("Hernoemen").clicked() {
                        self.do_folder_rename(&old_path);
                    }
                    if ui.button("Annuleren").clicked() {
                        self.folder_rename_path = None;
                    }
                });
            });
        if !open {
            self.folder_rename_path = None;
        }
    }

    fn do_folder_rename(&mut self, old_path: &Path) {
        if let Some(parent) = old_path.parent() {
            let new_path = parent.join(&self.folder_rename_name);
            match std::fs::rename(old_path, &new_path) {
                Ok(()) => {
                    self.tree.reload_at(parent);
                    if self.selected_dir == old_path {
                        self.select_dir(new_path);
                    } else if self.selected_dir == parent {
                        self.refresh_selected_dir();
                    }
                }
                Err(e) => self.op_error = Some(format!("Kan map niet hernoemen: {e}")),
            }
        }
        self.folder_rename_path = None;
    }

    fn render_folder_delete_dialog(&mut self, ctx: &Context) {
        let Some(path) = self.folder_delete_path.clone() else {
            return;
        };

        let mut open = true;
        egui::Window::new("Map verwijderen")
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label("Weet je zeker dat je deze map wilt verwijderen?");
                ui.monospace(path.to_string_lossy().to_string());
                ui.label(
                    RichText::new("Dit verwijdert de map en alle inhoud!")
                        .color(egui::Color32::from_rgb(255, 120, 120)),
                );
                ui.horizontal(|ui| {
                    if ui.button("Verwijderen").clicked() {
                        if let Some(parent) = path.parent() {
                            match std::fs::remove_dir_all(&path) {
                                Ok(()) => {
                                    self.tree.reload_at(parent);
                                    if self.selected_dir == path
                                        || self.selected_dir.starts_with(&path)
                                    {
                                        self.select_dir(parent.to_path_buf());
                                    } else if self.selected_dir == parent {
                                        self.refresh_selected_dir();
                                    }
                                }
                                Err(e) => {
                                    self.op_error = Some(format!("Kan map niet verwijderen: {e}"))
                                }
                            }
                        }
                        self.folder_delete_path = None;
                    }
                    if ui.button("Annuleren").clicked() {
                        self.folder_delete_path = None;
                    }
                });
            });
        if !open {
            self.folder_delete_path = None;
        }
    }

    fn render_file_rename_dialog(&mut self, ctx: &Context) {
        let Some(old_path) = self.file_rename_path.clone() else {
            return;
        };

        let mut open = true;
        egui::Window::new("Bestand hernoemen")
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(format!("Bestand: {}", old_path.display()));
                ui.horizontal(|ui| {
                    ui.label("Nieuwe naam:");
                    let re = ui.text_edit_singleline(&mut self.file_rename_name);
                    if re.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        if let Some(parent) = old_path.parent() {
                            let new_path = parent.join(&self.file_rename_name);
                            match std::fs::rename(&old_path, &new_path) {
                                Ok(()) => {
                                    if self.selected_file.as_ref() == Some(&old_path) {
                                        self.selected_file = Some(new_path);
                                    }
                                    self.refresh_selected_dir();
                                }
                                Err(e) => {
                                    self.op_error =
                                        Some(format!("Kan bestand niet hernoemen: {e}"))
                                }
                            }
                        }
                        self.file_rename_path = None;
                    }
                });
                ui.horizontal(|ui| {
                    if ui.button("Hernoemen").clicked() {
                        if let Some(parent) = old_path.parent() {
                            let new_path = parent.join(&self.file_rename_name);
                            match std::fs::rename(&old_path, &new_path) {
                                Ok(()) => {
                                    if self.selected_file.as_ref() == Some(&old_path) {
                                        self.selected_file = Some(new_path);
                                    }
                                    self.refresh_selected_dir();
                                }
                                Err(e) => {
                                    self.op_error =
                                        Some(format!("Kan bestand niet hernoemen: {e}"))
                                }
                            }
                        }
                        self.file_rename_path = None;
                    }
                    if ui.button("Annuleren").clicked() {
                        self.file_rename_path = None;
                    }
                });
            });
        if !open {
            self.file_rename_path = None;
        }
    }

    fn render_file_delete_dialog(&mut self, ctx: &Context) {
        let Some(path) = self.file_delete_path.clone() else {
            return;
        };

        let mut open = true;
        egui::Window::new("Bestand verwijderen")
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label("Weet je zeker dat je dit wilt verwijderen?");
                ui.monospace(path.to_string_lossy().to_string());
                ui.horizontal(|ui| {
                    if ui.button("Verwijderen").clicked() {
                        let is_dir = path.is_dir();
                        let result = if is_dir {
                            std::fs::remove_dir_all(&path)
                        } else {
                            std::fs::remove_file(&path)
                        };
                        match result {
                            Ok(()) => {
                                if self.selected_file.as_ref() == Some(&path) {
                                    self.selected_file = None;
                                }
                                self.refresh_selected_dir();
                                if is_dir {
                                    if let Some(parent) = path.parent() {
                                        self.tree.reload_at(parent);
                                    }
                                }
                            }
                            Err(e) => self.op_error = Some(format!("Kan niet verwijderen: {e}")),
                        }
                        self.file_delete_path = None;
                    }
                    if ui.button("Annuleren").clicked() {
                        self.file_delete_path = None;
                    }
                });
            });
        if !open {
            self.file_delete_path = None;
        }
    }

    fn render_properties_dialog(&mut self, ctx: &Context) {
        let Some(path) = self.properties_path.clone() else {
            return;
        };

        let mut open = true;
        egui::Window::new("Eigenschappen")
            .collapsible(false)
            .resizable(true)
            .open(&mut open)
            .default_width(400.0)
            .show(ctx, |ui| {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| path.to_string_lossy().to_string());
                ui.heading(format!("📁 {}", name));
                ui.monospace(path.to_string_lossy().to_string());
                ui.separator();

                if let Some(info) = self.properties_info.clone() {
                    egui::Grid::new("properties_grid")
                        .num_columns(2)
                        .spacing([20.0, 4.0])
                        .show(ui, |ui| {
                            ui.label("Totale grootte:");
                            ui.label(format_size(info.total_size));
                            ui.end_row();

                            ui.label("Bestanden:");
                            ui.label(format!("{}", info.file_count));
                            ui.end_row();

                            ui.label("Mappen:");
                            ui.label(format!("{}", info.folder_count));
                            ui.end_row();

                            ui.label("Lege mappen:");
                            ui.label(format!("{}", info.empty_folders.len()));
                            ui.end_row();
                        });

                    if !info.empty_folders.is_empty() {
                        ui.separator();
                        ui.label(RichText::new("Lege mappen:").strong());
                        egui::ScrollArea::vertical()
                            .max_height(200.0)
                            .id_source("empty_folders_scroll")
                            .show(ui, |ui| {
                                for folder in &info.empty_folders {
                                    let rel = folder.strip_prefix(&path).unwrap_or(folder);
                                    ui.label(format!("  📁 {}", rel.display()));
                                }
                            });

                        if ui.button("🗑 Lege mappen verwijderen").clicked() {
                            for folder in &info.empty_folders {
                                let _ = std::fs::remove_dir(folder);
                            }
                            self.tree.reload_at(&path);
                            if self.selected_dir == path || self.selected_dir.starts_with(&path) {
                                self.refresh_selected_dir();
                            }
                            self.start_properties_scan(path.clone());
                        }
                    }

                    ui.separator();
                    if ui.button("Vernieuwen").clicked() {
                        self.start_properties_scan(path.clone());
                    }
                } else {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Bezig met laden…");
                    });
                    ctx.request_repaint();
                }
            });

        if !open {
            self.properties_path = None;
            self.properties_info = None;
            self.properties_scan_rx = None;
        }
    }

    fn render_op_error_dialog(&mut self, ctx: &Context) {
        let Some(err) = self.op_error.clone() else {
            return;
        };
        egui::Window::new("Fout")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.colored_label(egui::Color32::from_rgb(255, 120, 120), &err);
                ui.add_space(8.0);
                if ui.button("OK").clicked() {
                    self.op_error = None;
                }
            });
    }

    fn render_add_favorite_modal(&mut self, ctx: &Context) {
        if !self.show_add_favorite_modal {
            return;
        }

        egui::Window::new("Add Favorite")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label("Name your favorite:");
                ui.text_edit_singleline(&mut self.new_favorite_name);
                ui.add_space(8.0);
                ui.label(format!("Path: {}", self.new_favorite_path.display()));
                ui.add_space(12.0);

                ui.horizontal(|ui| {
                    if ui.button("Add").clicked() {
                        let path = self.new_favorite_path.to_string_lossy().to_string();
                        let name = self.new_favorite_name.clone();
                        self.add_favorite(name, path);
                        self.show_add_favorite_modal = false;
                    }
                    if ui.button("Cancel").clicked() {
                        self.show_add_favorite_modal = false;
                    }
                });
            });
    }
}

#[cfg(test)]
mod tests {
    use super::AssetViewerApp;
    use crate::config::FavoriteFolder;

    #[test]
    fn favorite_names_are_deduplicated_with_suffixes() {
        let existing = vec![
            FavoriteFolder {
                name: "Assets".to_string(),
                path: "P:\\Assets".to_string(),
            },
            FavoriteFolder {
                name: "Assets (2)".to_string(),
                path: "D:\\Assets".to_string(),
            },
        ];

        assert_eq!(
            AssetViewerApp::unique_favorite_name(&existing, "Assets", "E:\\Assets"),
            "Assets (3)"
        );
    }

    #[test]
    fn empty_favorite_name_falls_back_to_folder_name() {
        let existing = Vec::new();
        assert_eq!(
            AssetViewerApp::unique_favorite_name(&existing, "  ", "P:\\projects\\Rust"),
            "Rust"
        );
    }
}

impl eframe::App for AssetViewerApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.handle_mouse_navigation(ctx);

        // Drain async properties scan result
        if let Some(rx) = &self.properties_scan_rx {
            if let Ok(props) = rx.try_recv() {
                self.properties_info = Some(props);
                self.properties_scan_rx = None;
            }
        }

        // Drain thumbnail results from background thread
        if let Some(rx) = &self.thumbnail_rx {
            let mut done = false;
            loop {
                match rx.try_recv() {
                    Ok((path, bytes, name)) => {
                        if let Ok(image) =
                            egui_extras::RetainedImage::from_image_bytes(name, &bytes)
                        {
                            self.thumbnail_cache.insert(path, image);
                        }
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        done = true;
                        break;
                    }
                }
            }
            if done {
                self.thumbnail_rx = None;
            }
        }

        self.render_top_bar(ctx);
        self.render_left_tree(ctx);
        self.render_right_preview(ctx);
        self.render_center_list(ctx);
        self.render_settings_window(ctx);
        self.render_add_favorite_modal(ctx);
        self.render_folder_new_dialog(ctx);
        self.render_folder_rename_dialog(ctx);
        self.render_folder_delete_dialog(ctx);
        self.render_file_rename_dialog(ctx);
        self.render_file_delete_dialog(ctx);
        self.render_properties_dialog(ctx);
        self.render_op_error_dialog(ctx);
    }
}
