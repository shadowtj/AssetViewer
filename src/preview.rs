#![allow(deprecated)]

use anyhow::Context;
use egui::{Context as EguiContext, vec2, Color32, RichText, ScrollArea, Slider, ComboBox, TextureOptions, Layout};
use egui_extras::RetainedImage;
use egui_plot::{Line, Plot, PlotPoints};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewKind {
    Image,
    Audio,
    Video,
    Model3D,
    Blend,
    Sprite,
    Zip,
    Text,
    Unknown,
}

impl PreviewKind {
    pub fn from_path(path: &Path) -> Self {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        match ext.as_str() {
            "png" | "jpg" | "jpeg" | "gif" | "bmp" | "tga" | "tif" | "tiff" | "webp" => PreviewKind::Image,
            "wav" | "mp3" | "ogg" | "flac" => PreviewKind::Audio,
            "mp4" | "mov" | "mkv" | "webm" => PreviewKind::Video,
            "fbx" | "obj" | "gltf" | "glb" => PreviewKind::Model3D,
            "blend" => PreviewKind::Blend,
            "sprite" => PreviewKind::Sprite,
            "zip" => PreviewKind::Zip,
            "txt" | "bat" | "rs" | "json" | "toml" | "md" | "yaml" | "yml" => PreviewKind::Text,
            _ => PreviewKind::Unknown,
        }
    }
}

pub trait PreviewUi {
    fn ui(&mut self, ui: &mut egui::Ui);
}

pub enum Preview {
    Image(ImagePreview),
    Audio(AudioPreview),
    Model3D(Model3DPreview),
    Blend(BlendPreview),
    Sprite(SpritePreview),
    Video(VideoPreview),
    Zip(ZipPreview),
    Text(TextPreview),
    Stub(StubPreview),
}

impl PreviewUi for Preview {
    fn ui(&mut self, ui: &mut egui::Ui) {
        match self {
            Preview::Image(p) => p.ui(ui),
            Preview::Audio(p) => p.ui(ui),
            Preview::Model3D(p) => p.ui(ui),
            Preview::Blend(p) => p.ui(ui),
            Preview::Sprite(p) => p.ui(ui),
            Preview::Video(p) => p.ui(ui),
            Preview::Zip(p) => p.ui(ui),
            Preview::Text(p) => p.ui(ui),
            Preview::Stub(p) => p.ui(ui),
        }
    }
}

// --- Image Preview ---

pub struct ImagePreview {
    pub path: PathBuf,
    image: RetainedImage,
    zoom: f32,
}

impl ImagePreview {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let bytes = std::fs::read(path).with_context(|| format!("Reading image: {}", path.display()))?;
        let image = RetainedImage::from_image_bytes(path.to_string_lossy(), &bytes)
            .map_err(|err| anyhow::anyhow!(err))?;
        Ok(Self { path: path.to_path_buf(), image, zoom: 1.0 })
    }
}

impl PreviewUi for ImagePreview {
    fn ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Design Tools");

        // Handle mouse wheel zoom
        if ui.rect_contains_pointer(ui.max_rect()) {
            let delta = ui.input(|i| i.smooth_scroll_delta.y);
            if delta != 0.0 {
                let zoom_factor = (delta / 200.0).exp();
                self.zoom = (self.zoom * zoom_factor).clamp(0.1, 32.0);
            }
        }

        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.label("Zoom:");
                ui.add(Slider::new(&mut self.zoom, 0.1..=32.0).logarithmic(true));
                if ui.button("Reset").clicked() { self.zoom = 1.0; }
                if ui.button("📸 Screenshot").clicked() {
                    let _ = rfd::FileDialog::new().add_filter("PNG", &["png"]).save_file();
                }
            });
        });

        ui.add_space(8.0);

        ScrollArea::both()
            .id_source("image_preview_scroll")
            .auto_shrink([false; 2])
            .max_height(ui.available_height() - 100.0) // Leave space for info
            .show(ui, |ui| {
                let size = self.image.size_vec2() * self.zoom;
                self.image.show_size(ui, size);
            });

        ui.add_space(12.0);
        ui.separator();
        ui.heading("Asset Info");
        ui.group(|ui| {
            ui.label(format!("Dimensions: {}x{}", self.image.width(), self.image.height()));
            ui.label(RichText::new(self.path.to_string_lossy()).small().color(Color32::GRAY));
        });
    }
}

// --- Audio Preview ---

#[derive(Serialize, Deserialize)]
struct AudioCacheData {
    points_left: Vec<[f64; 2]>,
    points_right: Vec<[f64; 2]>,
    duration_s: f64,
}

pub struct AudioPreview {
    path: PathBuf,
    points_left: Vec<[f64; 2]>,
    points_right: Vec<[f64; 2]>,
    duration_s: f64,
    zoom: f64,
    sink: Option<rodio::Sink>,
    show_stereo: bool,
}

impl AudioPreview {
    fn get_cache_path(path: &Path) -> Option<PathBuf> {
        let mut hasher = DefaultHasher::new();
        path.to_string_lossy().hash(&mut hasher);
        let hash = hasher.finish();
        directories::ProjectDirs::from("com", "Dev_Row", "AssetViewer").map(|pd| {
            let cache_dir = pd.cache_dir().join("audio_peaks");
            let _ = std::fs::create_dir_all(&cache_dir);
            cache_dir.join(format!("{:x}.json", hash))
        })
    }

    pub fn load(path: &Path, audio_handle: Option<&rodio::OutputStreamHandle>) -> anyhow::Result<Self> {
        if let Some(cache_path) = Self::get_cache_path(path) {
            if cache_path.exists() {
                if let Ok(raw) = std::fs::read_to_string(&cache_path) {
                    if let Ok(cached) = serde_json::from_str::<AudioCacheData>(&raw) {
                        let sink = audio_handle.and_then(|h| rodio::Sink::try_new(h).ok());
                        return Ok(Self {
                            path: path.to_path_buf(), points_left: cached.points_left, points_right: cached.points_right,
                            duration_s: cached.duration_s, zoom: 1.0, sink, show_stereo: false,
                        });
                    }
                }
            }
        }

        use symphonia::core::audio::SampleBuffer;
        use symphonia::core::codecs::DecoderOptions;
        use symphonia::core::formats::FormatOptions;
        use symphonia::core::io::MediaSourceStream;
        use symphonia::core::meta::MetadataOptions;
        use symphonia::core::probe::Hint;

        let file = std::fs::File::open(path).with_context(|| format!("Opening audio: {}", path.display()))?;
        let mss = MediaSourceStream::new(Box::new(file), Default::default());
        let mut hint = Hint::new();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) { hint.with_extension(ext); }
        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
            .with_context(|| "Probing audio format")?;
        let mut format = probed.format;
        let track = format.default_track().ok_or_else(|| anyhow::anyhow!("No default audio track"))?;
        let mut decoder = symphonia::default::get_codecs()
            .make(&track.codec_params, &DecoderOptions::default())
            .with_context(|| "Creating decoder")?;

        let sample_rate = track.codec_params.sample_rate.ok_or_else(|| anyhow::anyhow!("Missing sample_rate"))? as f64;
        let mut left_samples = Vec::new();
        let mut right_samples = Vec::new();
        let mut total_frames = 0u64;

        loop {
            let packet = match format.next_packet() {
                Ok(p) => p,
                Err(err) => {
                    use symphonia::core::errors::Error;
                    if let Error::IoError(ref e) = err { if e.kind() == std::io::ErrorKind::UnexpectedEof { break; } }
                    return Err(err).context("Reading audio packet");
                }
            };
            let decoded = decoder.decode(&packet).with_context(|| "Decoding audio packet")?;
            let spec = *decoded.spec();
            let duration = decoded.capacity() as u64;
            let mut sample_buf = SampleBuffer::<f32>::new(duration, spec);
            sample_buf.copy_interleaved_ref(decoded);
            let chan_count = spec.channels.count();
            let samples = sample_buf.samples();
            for frame in samples.chunks(chan_count) {
                if chan_count >= 2 { left_samples.push(frame[0]); right_samples.push(frame[1]); }
                else { left_samples.push(frame[0]); right_samples.push(frame[0]); }
            }
            total_frames += (samples.len() / chan_count) as u64;
        }

        let duration_s = total_frames as f64 / sample_rate;
        let target_points = 6000usize.min(left_samples.len().max(1));
        let bin = (left_samples.len() / target_points).max(1);
        let mut pts_l = Vec::new();
        let mut pts_r = Vec::new();
        for (i, chunk) in left_samples.chunks(bin).enumerate() {
            let m = chunk.iter().fold(0.0f32, |acc, &s| acc.max(s.abs()));
            pts_l.push([(i * bin) as f64 / sample_rate, m as f64]);
        }
        for (i, chunk) in right_samples.chunks(bin).enumerate() {
            let m = chunk.iter().fold(0.0f32, |acc, &s| acc.max(s.abs()));
            pts_r.push([(i * bin) as f64 / sample_rate, m as f64]);
        }

        if let Some(cache_path) = Self::get_cache_path(path) {
            let data = AudioCacheData { points_left: pts_l.clone(), points_right: pts_r.clone(), duration_s };
            if let Ok(raw) = serde_json::to_string(&data) { let _ = std::fs::write(cache_path, raw); }
        }

        let sink = audio_handle.and_then(|h| rodio::Sink::try_new(h).ok());
        Ok(Self { path: path.to_path_buf(), points_left: pts_l, points_right: pts_r, duration_s, zoom: 1.0, sink, show_stereo: false })
    }

    pub fn play(&mut self) {
        if let Some(sink) = &self.sink {
            if sink.empty() {
                if let Ok(file) = std::fs::File::open(&self.path) {
                    if let Ok(source) = rodio::Decoder::new(std::io::BufReader::new(file)) { sink.append(source); }
                }
            }
            sink.play();
        }
    }
    pub fn pause(&mut self) { if let Some(sink) = &self.sink { sink.pause(); } }
    pub fn stop(&mut self) { if let Some(sink) = &self.sink { sink.stop(); } }
    pub fn seek(&mut self, seconds: f64) { if let Some(sink) = &self.sink { let _ = sink.try_seek(std::time::Duration::from_secs_f64(seconds)); } }
}

impl PreviewUi for AudioPreview {
    fn ui(&mut self, ui: &mut egui::Ui) {
        let mut current_pos = 0.0;
        if let Some(sink) = &self.sink { current_pos = sink.get_pos().as_secs_f64(); }
        ui.horizontal(|ui| {
            ui.label(format!("Duration: {:.2}s", self.duration_s));
            ui.add_space(10.0);
            ui.add(Slider::new(&mut self.zoom, 0.25..=8.0).logarithmic(true));
            ui.checkbox(&mut self.show_stereo, "Stereo");
        });
        ui.horizontal(|ui| {
            if let Some(sink) = &self.sink {
                if sink.is_paused() || sink.empty() { if ui.button("▶ Play").clicked() { self.play(); } }
                else { if ui.button("⏸ Pause").clicked() { self.pause(); } }
                if ui.button("⏹ Stop").clicked() { self.stop(); }
            } else {
                ui.colored_label(Color32::RED, "Audio output not available");
            }
            let mut pos = current_pos;
            if ui.add(Slider::new(&mut pos, 0.0..=self.duration_s).show_value(false)).changed() { self.seek(pos); }
            ui.label(format!("{:.1}s / {:.1}s", current_pos, self.duration_s));
        });
        let view_end = (self.duration_s / self.zoom).max(0.01);
        if self.show_stereo {
            let line_l = Line::new(PlotPoints::from(self.points_left.clone())).color(Color32::from_rgb(100, 150, 255)).name("Left");
            let line_r = Line::new(PlotPoints::from(self.points_right.clone())).color(Color32::from_rgb(255, 150, 100)).name("Right");
            Plot::new("waveform_plot_stereo").height(260.0).include_y(0.0).include_y(1.0).include_x(0.0).include_x(view_end).legend(egui_plot::Legend::default())
                .show(ui, |plot_ui| { plot_ui.line(line_l); plot_ui.line(line_r); });
        } else {
            let line = Line::new(PlotPoints::from(self.points_left.clone()));
            Plot::new("waveform_plot").height(260.0).include_y(0.0).include_y(1.0).include_x(0.0).include_x(view_end)
                .show(ui, |plot_ui| { plot_ui.line(line); });
        }
    }
}

// --- Bitwise Sprite Preview (BW-SPRITE-1.0) ---

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BitWiseAnimation {
    pub frames: Vec<usize>,
    pub fps: f32,
    pub looping: bool,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum BitWiseFrames {
    Multi(Vec<Vec<u32>>),
    Single(Vec<u32>),
}

fn deserialize_frames<'de, D>(deserializer: D) -> Result<Vec<Vec<u32>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let frames = BitWiseFrames::deserialize(deserializer)?;
    Ok(match frames {
        BitWiseFrames::Multi(v) => v,
        BitWiseFrames::Single(v) => vec![v],
    })
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum PointOrPoints {
    Single([f32; 2]),
    Multi(Vec<[f32; 2]>),
}

fn deserialize_action_points<'de, D>(
    deserializer: D,
) -> Result<Option<HashMap<String, Vec<[f32; 2]>>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = Option::<HashMap<String, PointOrPoints>>::deserialize(deserializer)?;
    Ok(raw.map(|map| {
        map.into_iter()
            .map(|(key, value)| {
                let points = match value {
                    PointOrPoints::Single(point) => vec![point],
                    PointOrPoints::Multi(points) => points,
                };
                (key, points)
            })
            .collect()
    }))
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Offset2 {
    Int([i32; 2]),
    Float([f32; 2]),
}

fn deserialize_graph_offset<'de, D>(deserializer: D) -> Result<Option<[f32; 2]>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = Option::<Offset2>::deserialize(deserializer)?;
    Ok(raw.map(|value| match value {
        Offset2::Int([x, y]) => [x as f32, y as f32],
        Offset2::Float(offset) => offset,
    }))
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BitWiseSpriteJson {
    pub id: String,
    pub width: u32,
    pub height: u32,
    #[serde(deserialize_with = "deserialize_frames")]
    pub frames: Vec<Vec<u32>>,
    #[serde(default)]
    pub animations: HashMap<String, BitWiseAnimation>,
    #[serde(default, deserialize_with = "deserialize_graph_offset")]
    pub graph_offset: Option<[f32; 2]>,
    #[serde(default, deserialize_with = "deserialize_action_points")]
    pub action_points: Option<HashMap<String, Vec<[f32; 2]>>>,
}

pub struct SpritePreview {
    pub path: PathBuf,
    pub data: BitWiseSpriteJson,
    pub selected_animation: String,
    pub current_anim_frame_idx: usize,
    pub last_update: f64,
    pub use_detected_height: bool,
    pub is_playing: bool,
    pub zoom: f32,
}

impl SpritePreview {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let mut data: BitWiseSpriteJson = serde_json::from_str(&content)?;
        if data.animations.is_empty() {
            let frames: Vec<usize> = (0..data.frames.len()).collect();
            if !frames.is_empty() {
                data.animations.insert("Default".to_string(), BitWiseAnimation { frames, fps: 12.0, looping: true });
            }
        }
        let mut anim_names: Vec<String> = data.animations.keys().cloned().collect();
        anim_names.sort();
        let default_anim = anim_names.get(0).cloned().unwrap_or_else(|| "None".to_string());
        Ok(Self {
            path: path.to_path_buf(), data, selected_animation: default_anim,
            current_anim_frame_idx: 0, last_update: 0.0, use_detected_height: true,
            is_playing: false, zoom: 4.0,
        })
    }

    fn get_effective_height(&self, frame_idx: usize) -> u32 {
        if let Some(frame) = self.data.frames.get(frame_idx) {
            if self.data.width > 0 {
                let pixel_count = frame.len() as u32 / 4;
                let detected = pixel_count / self.data.width;
                if detected > 0 {
                    return detected;
                }
            }
        }
        self.data.height
    }

    fn render_frame_to_texture(&self, frame_idx: usize) -> Option<RetainedImage> {
        let frame_data = self.data.frames.get(frame_idx)?;
        let width = self.data.width;
        let height = if self.use_detected_height { self.get_effective_height(frame_idx) } else { self.data.height };
        if width == 0 || height == 0 { return None; }
        let mut pixels = Vec::with_capacity((width * height) as usize);
        let total_pixels = (width * height) as usize;
        for i in 0..total_pixels {
            let base = i * 4;
            let r = frame_data.get(base).copied().unwrap_or(0).min(255) as u8;
            let g = frame_data.get(base + 1).copied().unwrap_or(0).min(255) as u8;
            let b = frame_data.get(base + 2).copied().unwrap_or(0).min(255) as u8;
            let a = frame_data.get(base + 3).copied().unwrap_or(0).min(255) as u8;
            pixels.push(Color32::from_rgba_unmultiplied(r, g, b, a));
        }
        Some(
            RetainedImage::from_color_image(
                format!("{}_f{}", self.data.id, frame_idx),
                egui::ColorImage { size: [width as usize, height as usize], pixels },
            )
            .with_options(TextureOptions::NEAREST),
        )
    }
}

impl PreviewUi for SpritePreview {
    fn ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Design Tools");

        // Handle mouse wheel zoom
        if ui.rect_contains_pointer(ui.max_rect()) {
            let delta = ui.input(|i| i.smooth_scroll_delta.y);
            if delta != 0.0 {
                let zoom_factor = (delta / 200.0).exp();
                self.zoom = (self.zoom * zoom_factor).clamp(0.1, 64.0);
            }
        }

        ui.group(|ui| {
            ui.horizontal(|ui| {
                if ui.button(if self.is_playing { "⏸ Pause" } else { "▶ Play" }).clicked() {
                    self.is_playing = !self.is_playing;
                }
                ui.separator();
                ui.label("Zoom:");
                ui.add(Slider::new(&mut self.zoom, 0.1..=64.0).logarithmic(true));
                if ui.button("Reset").clicked() { self.zoom = 4.0; }
            });
            ui.horizontal(|ui| {
                ui.label("Animation:");
                ComboBox::from_id_source("s_anim")
                    .selected_text(&self.selected_animation)
                    .show_ui(ui, |ui| {
                        let mut names: Vec<_> = self.data.animations.keys().collect();
                        names.sort();
                        for n in names {
                            if ui.selectable_label(&self.selected_animation == n, n).clicked() {
                                self.selected_animation = n.clone();
                                self.current_anim_frame_idx = 0;
                            }
                        }
                    });
                ui.checkbox(&mut self.use_detected_height, "Auto Height");
            });
        });

        ui.add_space(8.0);

        let frame_to_show = if let Some(anim) = self.data.animations.get(&self.selected_animation) {
            if !anim.frames.is_empty() {
                if self.is_playing {
                    let time = ui.input(|i| i.time);
                    if time - self.last_update > (1.0 / anim.fps.max(0.1) as f64) {
                        self.current_anim_frame_idx = (self.current_anim_frame_idx + 1) % anim.frames.len();
                        self.last_update = time;
                    }
                    ui.ctx().request_repaint();
                }
                anim.frames.get(self.current_anim_frame_idx).cloned().unwrap_or(0)
            } else {
                0
            }
        } else {
            0
        };

        ScrollArea::both()
            .id_source("sprite_preview_scroll")
            .auto_shrink([false; 2])
            .max_height(ui.available_height() - 120.0) // Leave space for info
            .show(ui, |ui| {
                if self.data.frames.is_empty() {
                    ui.label("No frames to display.");
                } else {
                    let clamped_idx = frame_to_show.min(self.data.frames.len().saturating_sub(1));
                    if let Some(tex) = self.render_frame_to_texture(clamped_idx) {
                        let eff_h = if self.use_detected_height {
                            self.get_effective_height(clamped_idx)
                        } else {
                            self.data.height
                        };
                        let size = vec2(self.data.width as f32 * self.zoom, eff_h as f32 * self.zoom);
                        
                        // Use centered and justified layout within the scroll area
                        let available = ui.available_size();
                        let canvas_size = vec2(size.x.max(available.x), size.y.max(available.y));
                        ui.allocate_ui_with_layout(
                            canvas_size,
                            Layout::centered_and_justified(egui::Direction::LeftToRight),
                            |ui| {
                                tex.show_size(ui, size);
                            },
                        );
                    } else {
                        ui.label("Failed to render frame.");
                    }
                }
            });

        ui.add_space(12.0);
        ui.separator();
        ui.heading("Asset Info");
        ui.group(|ui| {
            ui.label(format!("ID: {}", self.data.id));
            ui.label(format!("Resolution: {}x{}", self.data.width, self.data.height));
            ui.label(format!("Total Frames: {}", self.data.frames.len()));
            ui.label(RichText::new(self.path.to_string_lossy()).small().color(Color32::GRAY));
        });
    }
}

// --- 3D Model Preview ---

pub struct Model3DPreview {
    path: PathBuf,
    info: Option<ModelInfo>,
    error: Option<String>,
    // Design Tools
    rotation: [f32; 3],
    scale: f32,
}
struct ModelInfo { mesh_count: usize, material_count: usize, vertex_count: usize, triangle_count: usize, min_bounds: [f32; 3], max_bounds: [f32; 3], meshes: Vec<String> }
impl Model3DPreview {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let mut preview = Self {
            path: path.to_path_buf(),
            info: None,
            error: None,
            rotation: [0.0, 0.0, 0.0],
            scale: 1.0,
        };
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        if ext == "obj" {
            match tobj::load_obj(path, &tobj::LoadOptions { single_index: true, triangulate: true, ..Default::default() }) {
                Ok((models, materials)) => {
                    let mut vertex_count = 0; let mut triangle_count = 0;
                    let mut min_b = [f32::MAX; 3]; let mut max_b = [f32::MIN; 3];
                    let mesh_names: Vec<String> = models.iter().map(|m| m.name.clone()).collect();
                    for model in models {
                        let mesh = &model.mesh;
                        vertex_count += mesh.positions.len() / 3; triangle_count += mesh.indices.len() / 3;
                        for i in 0..(mesh.positions.len() / 3) {
                            let p = [mesh.positions[i*3], mesh.positions[i*3+1], mesh.positions[i*3+2]];
                            for j in 0..3 { min_b[j] = min_b[j].min(p[j]); max_b[j] = max_b[j].max(p[j]); }
                        }
                    }
                    preview.info = Some(ModelInfo { mesh_count: mesh_names.len(), material_count: materials.map_or(0, |m| m.len()), vertex_count, triangle_count, min_bounds: min_b, max_bounds: max_b, meshes: mesh_names });
                }
                Err(err) => preview.error = Some(format!("Error loading OBJ: {:?}", err)),
            }
        } else {
            match easy_gltf::load(path) {
                Ok(scenes) => {
                    let mut vertex_count = 0; let mut triangle_count = 0;
                    let mut min_b = [f32::MAX; 3]; let mut max_b = [f32::MIN; 3];
                    let mut mesh_names = Vec::new();
                    for scene in scenes {
                        for model in scene.models {
                            mesh_names.push("GLTF Mesh".to_string());
                            let vertices = model.vertices();
                            vertex_count += vertices.len(); triangle_count += model.indices().map_or(vertices.len() / 3, |idx| idx.len() / 3);
                            for v in vertices {
                                let p = v.position;
                                min_b[0] = min_b[0].min(p.x); min_b[1] = min_b[1].min(p.y); min_b[2] = min_b[2].min(p.z);
                                max_b[0] = max_b[0].max(p.x); max_b[1] = max_b[1].max(p.y); max_b[2] = max_b[2].max(p.z);
                            }
                        }
                    }
                    preview.info = Some(ModelInfo { mesh_count: mesh_names.len(), material_count: mesh_names.len(), vertex_count, triangle_count, min_bounds: min_b, max_bounds: max_b, meshes: mesh_names });
                }
                Err(err) => preview.error = Some(format!("Error loading GLTF: {:?}", err)),
            }
        }
        Ok(preview)
    }
}
impl PreviewUi for Model3DPreview {
    fn ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Design Tools");
        
        // Handle mouse wheel scale
        if ui.rect_contains_pointer(ui.max_rect()) {
            let delta = ui.input(|i| i.smooth_scroll_delta.y);
            if delta != 0.0 {
                let zoom_factor = (delta / 200.0).exp();
                self.scale = (self.scale * zoom_factor).clamp(0.01, 100.0);
            }
        }

        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.label("Rotation:");
                ui.add(egui::DragValue::new(&mut self.rotation[0]).speed(1.0).prefix("X: "));
                ui.add(egui::DragValue::new(&mut self.rotation[1]).speed(1.0).prefix("Y: "));
                ui.add(egui::DragValue::new(&mut self.rotation[2]).speed(1.0).prefix("Z: "));
            });
            ui.horizontal(|ui| {
                ui.label("Scale:   ");
                ui.add(Slider::new(&mut self.scale, 0.01..=10.0).logarithmic(true));
                if ui.button("Reset").clicked() {
                    self.rotation = [0.0, 0.0, 0.0];
                    self.scale = 1.0;
                }
            });
        });

        ui.add_space(12.0);
        ui.separator();
        ui.add_space(4.0);
        
        ui.heading("Asset Info");
        ui.label(RichText::new(self.path.to_string_lossy()).small().color(Color32::GRAY));
        
        if let Some(info) = &self.info {
            ui.group(|ui| {
                ui.label(format!("Meshes: {}", info.mesh_count));
                ui.label(format!("Materials: {}", info.material_count));
                ui.label(format!("Vertices: {}", info.vertex_count));
                ui.label(format!("Triangles: {}", info.triangle_count));
                ui.separator();
                ui.label(format!("Min: [{:.2}, {:.2}, {:.2}]", info.min_bounds[0], info.min_bounds[1], info.min_bounds[2]));
                ui.label(format!("Max: [{:.2}, {:.2}, {:.2}]", info.max_bounds[0], info.max_bounds[1], info.max_bounds[2]));
            });
            ui.collapsing("List Meshes", |ui| { for name in &info.meshes { ui.label(format!("• {}", name)); } });
        } else if let Some(err) = &self.error { ui.colored_label(Color32::LIGHT_RED, err); }
    }
}

// --- Blender Preview ---

pub struct BlendPreview { path: PathBuf, thumbnail: Option<RetainedImage>, error: Option<String>, blender_path: Option<String>, is_generating: bool }
impl BlendPreview {
    pub fn new(path: &Path, blender_path: Option<&str>) -> Self {
        let mut preview = Self { path: path.to_path_buf(), thumbnail: None, error: None, blender_path: blender_path.map(|s| s.to_string()), is_generating: false };
        if let Some(cp) = preview.cache_path() {
            if cp.exists() { if let Ok(bytes) = std::fs::read(&cp) { preview.thumbnail = RetainedImage::from_image_bytes(cp.to_string_lossy(), &bytes).ok(); } }
            else if blender_path.is_none() { preview.error = Some("Set Blender path in Settings.".to_string()); }
        }
        preview
    }
    fn cache_path(&self) -> Option<PathBuf> {
        let mut hasher = DefaultHasher::new(); self.path.to_string_lossy().hash(&mut hasher);
        directories::ProjectDirs::from("com", "Dev_Row", "AssetViewer").map(|pd| {
            let cache_dir = pd.cache_dir().join("blender_thumbs"); let _ = std::fs::create_dir_all(&cache_dir);
            cache_dir.join(format!("{:x}.png", hasher.finish()))
        })
    }
    pub fn generate_thumbnail(&mut self) {
        let (Some(blender_exe), Some(out_path)) = (&self.blender_path, self.cache_path()) else { return };
        self.is_generating = true;
        let python_cmd = format!("import bpy; bpy.ops.wm.open_mainfile(filepath=r'{}'); bpy.context.scene.render.filepath = r'{}'; bpy.ops.render.render(write_still=True)", self.path.to_string_lossy(), out_path.to_string_lossy());
        if let Ok(output) = std::process::Command::new(blender_exe).arg("--background").arg("--python-expr").arg(python_cmd).output() {
            if output.status.success() && out_path.exists() { if let Ok(bytes) = std::fs::read(&out_path) { self.thumbnail = RetainedImage::from_image_bytes(out_path.to_string_lossy(), &bytes).ok(); self.error = None; } }
            else { self.error = Some("Blender failed.".to_string()); }
        }
        self.is_generating = false;
    }
}
impl PreviewUi for BlendPreview {
    fn ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Blender File");
        if self.is_generating { ui.spinner(); }
        else if let Some(thumb) = &mut self.thumbnail { ScrollArea::both().show(ui, |ui| { thumb.show(ui); }); if ui.button("Regenerate").clicked() { self.generate_thumbnail(); } }
        else { if let Some(err) = &self.error { ui.colored_label(Color32::YELLOW, err); } if self.blender_path.is_some() && ui.button("Generate").clicked() { self.generate_thumbnail(); } }
    }
}

// --- Video Preview ---

pub struct VideoPreview {
    path: PathBuf,
    file_size: u64,
    
    // Playback state
    texture: Option<RetainedImage>,
    current_time: f64,
    duration: f64,
    is_playing: bool,
    last_frame_update: Instant,
    
    // Info
    width: u32,
    height: u32,
}

impl VideoPreview {
    pub fn load(path: &Path, _ctx: &EguiContext) -> anyhow::Result<Self> {
        let meta = std::fs::metadata(path)?;
        
        // Get duration and resolution using ffprobe
        let output = std::process::Command::new("ffprobe")
            .args([
                "-v", "error",
                "-show_entries", "format=duration:stream=width,height",
                "-of", "default=noprint_wrappers=1:nokey=1",
                &path.to_string_lossy(),
            ])
            .output()?;
            
        let out_str = String::from_utf8_lossy(&output.stdout);
        let mut lines = out_str.lines();
        let width: u32 = lines.next().unwrap_or("0").parse().unwrap_or(0);
        let height: u32 = lines.next().unwrap_or("0").parse().unwrap_or(0);
        let duration: f64 = lines.next().unwrap_or("0").parse().unwrap_or(0.0);

        let mut preview = Self {
            path: path.to_path_buf(),
            file_size: meta.len(),
            texture: None,
            current_time: 0.0,
            duration,
            is_playing: false,
            last_frame_update: Instant::now(),
            width,
            height,
        };
        
        // Load first frame
        preview.extract_frame(0.0);
        
        Ok(preview)
    }

    fn extract_frame(&mut self, timestamp: f64) {
        let output = std::process::Command::new("ffmpeg")
            .args([
                "-ss", &timestamp.to_string(),
                "-i", &self.path.to_string_lossy(),
                "-frames:v", "1",
                "-f", "image2",
                "-vcodec", "mjpeg",
                "pipe:1",
            ])
            .output();

        if let Ok(output) = output {
            if let Ok(image) = RetainedImage::from_image_bytes(
                format!("{}_{}", self.path.display(), timestamp),
                &output.stdout
            ) {
                self.texture = Some(image);
            }
        }
    }
}

impl PreviewUi for VideoPreview {
    fn ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Design Tools");

        if self.is_playing {
            let now = Instant::now();
            let elapsed = now.duration_since(self.last_frame_update).as_secs_f64();
            if elapsed > 0.1 { // ~10 FPS for the preview
                self.current_time += elapsed;
                if self.current_time > self.duration {
                    self.current_time = 0.0;
                }
                self.extract_frame(self.current_time);
                self.last_frame_update = now;
            }
            ui.ctx().request_repaint();
        }

        ui.group(|ui| {
            ui.horizontal(|ui| {
                if ui.button(if self.is_playing { "⏸ Pause" } else { "▶ Play" }).clicked() {
                    self.is_playing = !self.is_playing;
                    self.last_frame_update = Instant::now();
                }
                
                if ui.button("⏹ Stop").clicked() {
                    self.is_playing = false;
                    self.current_time = 0.0;
                    self.extract_frame(0.0);
                }

                let mut seek = self.current_time;
                if ui.add(Slider::new(&mut seek, 0.0..=self.duration).show_value(false)).changed() {
                    self.current_time = seek;
                    self.extract_frame(self.current_time);
                    self.is_playing = false;
                }
                
                ui.label(format!("{:.1}s / {:.1}s", self.current_time, self.duration));
            });
        });

        ui.add_space(8.0);
        
        if let Some(tex) = &self.texture {
            let available_width = ui.available_width();
            let aspect = self.width as f32 / self.height as f32;
            let draw_size = vec2(available_width, available_width / aspect);
            
            ScrollArea::vertical()
                .id_source("vid_scroll")
                .max_height(ui.available_height() - 120.0)
                .show(ui, |ui| {
                    tex.show_size(ui, draw_size);
                });
        }

        ui.add_space(12.0);
        ui.separator();
        ui.heading("Asset Info");
        ui.group(|ui| {
            ui.label(format!("Resolution: {}x{}", self.width, self.height));
            ui.label(format!("Size: {:.2} MB", self.file_size as f64 / 1_048_576.0));
            ui.label(RichText::new(self.path.to_string_lossy()).small().color(Color32::GRAY));
            if ui.button("🎬 Open in System Player").clicked() {
                let _ = open::that(&self.path);
            }
        });
    }
}
// --- ZIP Preview ---

pub struct ZipEntry { pub name: String, pub is_dir: bool, pub size: u64, pub children: HashMap<String, ZipEntry> }
pub struct ZipPreview { pub root: ZipEntry, pub file_count: usize, pub total_size: u64 }
impl ZipPreview {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let file = std::fs::File::open(path)?; let mut archive = zip::ZipArchive::new(file)?;
        let mut root = ZipEntry { name: "/".into(), is_dir: true, size: 0, children: HashMap::new() };
        let mut total_size = 0u64;
        for i in 0..archive.len() {
            let file = archive.by_index(i)?; let parts: Vec<&str> = file.name().split('/').filter(|s| !s.is_empty()).collect();
            let mut current = &mut root;
            for (idx, part) in parts.iter().enumerate() {
                let is_last = idx == parts.len() - 1;
                current = current.children.entry(part.to_string()).or_insert_with(|| ZipEntry { name: part.to_string(), is_dir: !is_last || file.is_dir(), size: if is_last { file.size() } else { 0 }, children: HashMap::new() });
            }
            if !file.is_dir() {
                total_size = total_size.saturating_add(file.size());
            }
        }
        Ok(Self { root, file_count: archive.len(), total_size })
    }
    fn render_entry(ui: &mut egui::Ui, entry: &ZipEntry) {
        if entry.is_dir { ui.collapsing(format!("📁 {}", entry.name), |ui| { let mut keys: Vec<_> = entry.children.keys().collect(); keys.sort(); for k in keys { Self::render_entry(ui, &entry.children[k]); } }); }
        else { ui.label(format!("📄 {} ({} bytes)", entry.name, entry.size)); }
    }
}
impl PreviewUi for ZipPreview {
    fn ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("ZIP Archive");
        ui.label(format!("Files: {}", self.file_count));
        ui.label(format!("Total size: {} bytes", self.total_size));
        ScrollArea::vertical().id_source("z").show(ui, |ui| { let mut keys: Vec<_> = self.root.children.keys().collect(); keys.sort(); for k in keys { Self::render_entry(ui, &self.root.children[k]); } });
    }
}

// --- Text Preview ---

pub struct TextPreview { pub path: PathBuf, pub content: String }
impl TextPreview { pub fn load(path: &Path) -> anyhow::Result<Self> { Ok(Self { path: path.to_path_buf(), content: std::fs::read_to_string(path)? }) } }
impl PreviewUi for TextPreview { fn ui(&mut self, ui: &mut egui::Ui) { ui.heading("Text"); ui.label(RichText::new(self.path.to_string_lossy()).small().color(Color32::GRAY)); ScrollArea::both().show(ui, |ui| { ui.add(egui::TextEdit::multiline(&mut self.content).font(egui::TextStyle::Monospace).desired_width(f32::INFINITY)); }); } }

// --- Stub Preview ---

pub struct StubPreview { title: String, detail: String }
impl StubPreview { pub fn new(title: impl Into<String>, detail: impl Into<String>) -> Self { Self { title: title.into(), detail: detail.into() } } }
impl PreviewUi for StubPreview { fn ui(&mut self, ui: &mut egui::Ui) { ui.heading(&self.title); ui.label(&self.detail); } }

// --- Preview Cache ---

#[derive(Default)] pub struct PreviewCache { items: HashMap<PathBuf, (Instant, Preview)> }
impl PreviewCache {
    pub fn get_or_build(&mut self, _ctx: &EguiContext, path: &Path, kind: PreviewKind, audio_handle: Option<&rodio::OutputStreamHandle>, blender_path: Option<&str>) -> anyhow::Result<&mut Preview> {
        let key = path.to_path_buf();
        if !self.items.contains_key(&key) {
            let preview = match kind {
                PreviewKind::Image => Preview::Image(ImagePreview::load(path)?),
                PreviewKind::Audio => Preview::Audio(AudioPreview::load(path, audio_handle)?),
                PreviewKind::Model3D => Preview::Model3D(Model3DPreview::load(path)?),
                PreviewKind::Blend => Preview::Blend(BlendPreview::new(path, blender_path)),
                PreviewKind::Sprite => Preview::Sprite(SpritePreview::load(path)?),
                PreviewKind::Video => Preview::Video(VideoPreview::load(path, _ctx)?),
                PreviewKind::Zip => Preview::Zip(ZipPreview::load(path)?),
                PreviewKind::Text => Preview::Text(TextPreview::load(path)?),
                PreviewKind::Unknown => Preview::Stub(StubPreview::new("Unknown", "No preview.")),
            };
            self.items.insert(key.clone(), (Instant::now(), preview));
        }
        if let Some((t, _)) = self.items.get_mut(&key) { *t = Instant::now(); }
        Ok(&mut self.items.get_mut(&key).unwrap().1)
    }
}
