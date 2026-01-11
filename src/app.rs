use crate::config::{AppConfig, ConfigStore};
use crate::fs_model::{DirNode, FsEntry};
use crate::preview::{PreviewCache, PreviewKind, PreviewUi};
use egui::{CentralPanel, Context, Id, Layout, RichText, SidePanel, TopBottomPanel};
use std::path::{Path, PathBuf};

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

pub struct AssetViewerApp {
    config_store: ConfigStore,
    config: AppConfig,

    root_path: PathBuf,
    tree: DirNode,
    selected_dir: PathBuf,
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

    // Audio output
    _audio_stream: Option<(rodio::OutputStream, rodio::OutputStreamHandle)>,
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

        let mut app = Self {
            config_store,
            config,
            root_path,
            tree,
            selected_dir,
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
            sort_mode: SortMode::Name,
            sort_ascending: true,
            view_mode: ViewMode::List,
            thumbnail_cache: std::collections::HashMap::new(),
            _audio_stream: audio_stream,
        };
        app.load_thumbnails();
        app.apply_filters();
        app
    }

    fn set_root_path(&mut self, new_root: PathBuf) {
        self.root_path = new_root;
        self.selected_dir = self.root_path.clone();
        self.address_bar = self.selected_dir.to_string_lossy().to_string();
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

    fn add_favorite(&mut self, name: String, path: String) {
        let mut name = name.trim().to_string();
        if name.is_empty() {
            name = Path::new(&path)
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "New Favorite".to_string());
        }

        let mut final_name = name.clone();
        let mut count = 2;
        while self.config.favorites.iter().any(|f| f.name == final_name) {
            final_name = format!("{} ({})", name, count);
            count += 1;
        }

        self.config.favorites.push(crate::config::FavoriteFolder {
            name: final_name,
            path,
        });
        self.save_config();
    }

    fn refresh_selected_dir(&mut self) {
        self.entries = crate::fs_model::list_dir_entries(&self.selected_dir).unwrap_or_default();
        self.load_thumbnails();
        self.apply_filters();
    }

    fn load_thumbnails(&mut self) {
        // Simple synchronous thumbnail loader for images
        for entry in &self.entries {
            if !entry.is_dir && !self.thumbnail_cache.contains_key(&entry.path) {
                let kind = crate::preview::PreviewKind::from_path(&entry.path);
                if kind == crate::preview::PreviewKind::Image {
                    if let Ok(bytes) = std::fs::read(&entry.path) {
                        if let Ok(image) = egui_extras::RetainedImage::from_image_bytes(entry.name.clone(), &bytes) {
                            self.thumbnail_cache.insert(entry.path.clone(), image);
                        }
                    }
                }
            }
        }
    }

    fn apply_filters(&mut self) {
        let query = self.search_query.to_lowercase();
        let query_parts: Vec<&str> = query.split_whitespace().collect();

        let mut filtered: Vec<FsEntry> = self.entries.iter()
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
                        query_parts.iter().all(|&p| name_lower.contains(p) || e.extension == p)
                    }
                }
            })
            .cloned()
            .collect();

        filtered.sort_by(|a, b| {
            let ord = match self.sort_mode {
                SortMode::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                SortMode::Type => a.extension.cmp(&b.extension),
                SortMode::Date => a.modified.cmp(&b.modified),
                SortMode::Size => a.size.cmp(&b.size),
            };
            if self.sort_ascending { ord } else { ord.reverse() }
        });

        // Always keep directories first if sorting by name or type
        if self.sort_mode == SortMode::Name || self.sort_mode == SortMode::Type {
            filtered.sort_by(|a, b| {
                match (a.is_dir, b.is_dir) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => std::cmp::Ordering::Equal,
                }
            });
        }

        self.filtered_entries = filtered;
    }

    fn select_dir(&mut self, dir: PathBuf) {
        self.selected_dir = dir;
        self.address_bar = self.selected_dir.to_string_lossy().to_string();
        self.selected_file = None;
        self.refresh_selected_dir();
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
                        self.config.last_favorite_index
                            .and_then(|i| self.config.favorites.get(i))
                            .map(|f| f.name.as_str())
                            .unwrap_or("Favorites")
                    )
                    .show_ui(ui, |ui| {
                        for i in 0..self.config.favorites.iter().len() {
                            let name = self.config.favorites[i].name.clone();
                            if ui.selectable_label(self.config.last_favorite_index == Some(i), name).clicked() {
                                self.jump_to_favorite(i);
                            }
                        }
                    });

                if ui.button("Add Favorite…").clicked() {
                    if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                        self.new_favorite_path = folder;
                        self.new_favorite_name = self.new_favorite_path
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
                            let _ = crate::fs_model::export_asset(&path, &target_dir, self.config.flatten_export);
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
                let response = ui.add(egui::TextEdit::singleline(&mut self.address_bar).desired_width(f32::INFINITY));
                if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
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
                        let is_active = self.config.last_favorite_index == Some(i) && self.root_path == PathBuf::from(&fav.path);
                        let response = ui.selectable_label(
                            is_active,
                            format!("⭐ {}", fav.name)
                        );
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
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.tree.ui_render(ui, &mut |selected| {
                        next_selected = Some(selected.to_path_buf());
                    }, &self.selected_dir);
                });

                if let Some(dir) = next_selected {
                    self.select_dir(dir);
                }
            });
    }

    fn render_center_list(&mut self, ctx: &Context) {
        CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Files");
                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(egui::TextEdit::singleline(&mut self.search_query).hint_text("Search...")).changed() {
                        self.apply_filters();
                    }
                    ui.label("🔍");
                });
            });

            ui.horizontal(|ui| {
                ui.label("Sort:");
                if ui.selectable_label(self.sort_mode == SortMode::Name, "Name").clicked() {
                    if self.sort_mode == SortMode::Name { self.sort_ascending = !self.sort_ascending; }
                    else { self.sort_mode = SortMode::Name; self.sort_ascending = true; }
                    self.apply_filters();
                }
                if ui.selectable_label(self.sort_mode == SortMode::Type, "Type").clicked() {
                    if self.sort_mode == SortMode::Type { self.sort_ascending = !self.sort_ascending; }
                    else { self.sort_mode = SortMode::Type; self.sort_ascending = true; }
                    self.apply_filters();
                }
                if ui.selectable_label(self.sort_mode == SortMode::Size, "Size").clicked() {
                    if self.sort_mode == SortMode::Size { self.sort_ascending = !self.sort_ascending; }
                    else { self.sort_mode = SortMode::Size; self.sort_ascending = true; }
                    self.apply_filters();
                }
                if ui.selectable_label(self.sort_mode == SortMode::Date, "Date").clicked() {
                    if self.sort_mode == SortMode::Date { self.sort_ascending = !self.sort_ascending; }
                    else { self.sort_mode = SortMode::Date; self.sort_ascending = true; }
                    self.apply_filters();
                }

                ui.separator();
                ui.label("View:");
                if ui.selectable_label(self.view_mode == ViewMode::List, "☰ List").clicked() {
                    self.view_mode = ViewMode::List;
                }
                if ui.selectable_label(self.view_mode == ViewMode::Grid, "▦ Grid").clicked() {
                    self.view_mode = ViewMode::Grid;
                }
            });

            ui.separator();

            let mut clicked_dir = None;
            let mut clicked_file = None;
            let mut open_explorer = None;
            let mut copy_path = None;

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

                        response.context_menu(|ui| {
                            if ui.button("Open").clicked() {
                                if entry.is_dir { clicked_dir = Some(entry.path().to_path_buf()); }
                                else { clicked_file = Some(entry.path().to_path_buf()); }
                                ui.close_menu();
                            }
                            if ui.button("Open in Explorer").clicked() {
                                open_explorer = Some(entry.path().to_path_buf());
                                ui.close_menu();
                            }
                            if ui.button("Copy Path").clicked() {
                                copy_path = Some(entry.path().to_string_lossy().to_string());
                                ui.close_menu();
                            }
                        });

                        if response.clicked() {
                            if entry.is_dir {
                                clicked_dir = Some(entry.path().to_path_buf());
                            } else {
                                clicked_file = Some(entry.path().to_path_buf());
                            }
                        }

                        if response.double_clicked() && entry.is_dir {
                            clicked_dir = Some(entry.path().to_path_buf());
                        }
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

                            let (rect, response) = ui.allocate_at_least(item_size, egui::Sense::click());
                            
                            if ui.is_rect_visible(rect) {
                                let visuals = ui.style().interact_selectable(&response, is_selected);
                                if is_selected || response.hovered() {
                                    ui.painter().rect(rect.expand(2.0), 4.0, visuals.bg_fill, visuals.bg_stroke);
                                }

                                // Draw thumbnail or icon
                                let thumb_rect = egui::Rect::from_min_size(rect.min + egui::vec2(10.0, 5.0), egui::vec2(80.0, 80.0));
                                if let Some(thumb) = self.thumbnail_cache.get(&entry.path) {
                                    ui.painter().image(thumb.texture_id(ctx), thumb_rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
                                } else {
                                    let icon = if entry.is_dir { "📁" } else { "📄" };
                                    ui.painter().text(thumb_rect.center(), egui::Align2::CENTER_CENTER, icon, egui::FontId::proportional(40.0), visuals.fg_stroke.color);
                                }

                                // Draw name
                                let name_rect = egui::Rect::from_min_max(egui::pos2(rect.min.x, rect.min.y + 90.0), rect.max);
                                ui.painter().text(name_rect.center(), egui::Align2::CENTER_CENTER, &entry.name, egui::FontId::proportional(12.0), visuals.fg_stroke.color);
                            }

                            response.context_menu(|ui| {
                                if ui.button("Open").clicked() {
                                    if entry.is_dir { clicked_dir = Some(entry.path().to_path_buf()); }
                                    else { clicked_file = Some(entry.path().to_path_buf()); }
                                    ui.close_menu();
                                }
                                if ui.button("Open in Explorer").clicked() {
                                    open_explorer = Some(entry.path().to_path_buf());
                                    ui.close_menu();
                                }
                                if ui.button("Copy Path").clicked() {
                                    copy_path = Some(entry.path().to_string_lossy().to_string());
                                    ui.close_menu();
                                }
                            });

                            if response.clicked() {
                                if entry.is_dir {
                                    clicked_dir = Some(entry.path().to_path_buf());
                                } else {
                                    clicked_file = Some(entry.path().to_path_buf());
                                }
                            }

                            if response.double_clicked() && entry.is_dir {
                                clicked_dir = Some(entry.path().to_path_buf());
                            }
                        }
                    });
                }
            });

            if let Some(dir) = clicked_dir {
                self.select_dir(dir);
            } else if let Some(file) = clicked_file {
                self.select_file(file);
            }

            if let Some(path) = open_explorer {
                #[cfg(target_os = "windows")]
                {
                    let _ = std::process::Command::new("explorer")
                        .arg("/select,")
                        .arg(path.to_string_lossy().to_string())
                        .spawn();
                }
            }

            if let Some(path) = copy_path {
                ui.output_mut(|o| o.copied_text = path);
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
                        ui.label("Tip: images and basic audio decoding are implemented. Video/3D is stubbed for now.");
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
                                        ui.label(RichText::new(&self.config.favorites[i].path).small().color(egui::Color32::GRAY));
                                    });
                                    
                                    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                                        if ui.button("🗑").on_hover_text("Remove").clicked() {
                                            to_remove = Some(i);
                                        }
                                        if ui.button("📂").on_hover_text("Change Path").clicked() {
                                            if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                                                self.config.favorites[i].path = folder.to_string_lossy().to_string();
                                            }
                                        }
                                    });
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
                        let name = folder.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "New Favorite".to_string());
                        self.add_favorite(name, path);
                    }
                }

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
                ui.checkbox(&mut self.config.flatten_export, "Flatten textures to root folder");
                ui.horizontal(|ui| {
                    ui.label("Profile:");
                    egui::ComboBox::from_id_source("export_profile")
                        .selected_text(&self.config.export_profile)
                        .show_ui(ui, |ui| {
                            let profiles = ["Generic", "Unreal Engine", "Unity", "Blender"];
                            for p in profiles {
                                if ui.selectable_label(self.config.export_profile == p, p).clicked() {
                                    self.config.export_profile = p.to_string();
                                    self.save_config();
                                }
                            }
                        });
                });

                ui.separator();
                ui.small("Phase 4 (GPU renderer) and Phase 3 (FFmpeg) are still in active development.");
            });

        self.show_settings = is_open;
        if let Some(root) = root_to_set {
            self.set_root_path(root);
        }
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

impl eframe::App for AssetViewerApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.render_top_bar(ctx);
        self.render_left_tree(ctx);
        self.render_right_preview(ctx);
        self.render_center_list(ctx);
        self.render_settings_window(ctx);
        self.render_add_favorite_modal(ctx);
    }
}
