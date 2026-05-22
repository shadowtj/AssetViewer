#![allow(deprecated)]

use anyhow::Context;
use egui::{
    vec2, Color32, ComboBox, Context as EguiContext, Layout, RichText, ScrollArea, Slider,
    TextureOptions,
};
use egui_extras::RetainedImage;
use egui_plot::{Line, Plot, PlotPoints};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::Instant;

type ActionPoints = HashMap<String, Vec<[f32; 2]>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewKind {
    Image,
    Heic,
    Audio,
    Video,
    Model3D,
    Blend,
    Sprite,
    Pdf,
    SevenZip,
    Rar,
    Font,
    Zip,
    Text,
    Unknown,
}

impl PreviewKind {
    pub fn from_path(path: &Path) -> Self {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        match ext.as_str() {
            "png" | "jpg" | "jpeg" | "gif" | "bmp" | "tga" | "tif" | "tiff" | "webp" => {
                PreviewKind::Image
            }
            "heic" | "heif" => PreviewKind::Heic,
            "wav" | "mp3" | "ogg" | "flac" => PreviewKind::Audio,
            "mp4" | "mov" | "mkv" | "webm" => PreviewKind::Video,
            "fbx" | "obj" | "stl" | "gltf" | "glb" => PreviewKind::Model3D,
            "blend" => PreviewKind::Blend,
            "sprite" => PreviewKind::Sprite,
            "pdf" => PreviewKind::Pdf,
            "7z" => PreviewKind::SevenZip,
            "rar" => PreviewKind::Rar,
            "ttf" | "otf" => PreviewKind::Font,
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
    Heic(HeicPreview),
    Audio(AudioPreview),
    Model3D(Model3DPreview),
    Blend(BlendPreview),
    Sprite(SpritePreview),
    Pdf(PdfPreview),
    SevenZip(SevenZipPreview),
    Rar(RarPreview),
    Font(FontPreview),
    Video(VideoPreview),
    Zip(ZipPreview),
    Text(TextPreview),
    Stub(StubPreview),
}

impl PreviewUi for Preview {
    fn ui(&mut self, ui: &mut egui::Ui) {
        match self {
            Preview::Image(p) => p.ui(ui),
            Preview::Heic(p) => p.ui(ui),
            Preview::Audio(p) => p.ui(ui),
            Preview::Model3D(p) => p.ui(ui),
            Preview::Blend(p) => p.ui(ui),
            Preview::Sprite(p) => p.ui(ui),
            Preview::Pdf(p) => p.ui(ui),
            Preview::SevenZip(p) => p.ui(ui),
            Preview::Rar(p) => p.ui(ui),
            Preview::Font(p) => p.ui(ui),
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
        let bytes =
            std::fs::read(path).with_context(|| format!("Reading image: {}", path.display()))?;
        let decoded = image::load_from_memory(&bytes)
            .with_context(|| format!("Decoding image: {}", path.display()))?;
        let rgba = decoded.to_rgba8();
        let image = RetainedImage::from_color_image(
            path.to_string_lossy(),
            egui::ColorImage::from_rgba_unmultiplied(
                [rgba.width() as usize, rgba.height() as usize],
                rgba.as_raw(),
            ),
        );
        Ok(Self {
            path: path.to_path_buf(),
            image,
            zoom: 1.0,
        })
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
                if ui.button("Reset").clicked() {
                    self.zoom = 1.0;
                }
                if ui.button("📸 Screenshot").clicked() {
                    let _ = rfd::FileDialog::new()
                        .add_filter("PNG", &["png"])
                        .save_file();
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
            ui.label(format!(
                "Dimensions: {}x{}",
                self.image.width(),
                self.image.height()
            ));
            ui.label(
                RichText::new(self.path.to_string_lossy())
                    .small()
                    .color(Color32::GRAY),
            );
        });
    }
}

// --- HEIC Preview ---

pub struct HeicPreview {
    path: PathBuf,
    image: RetainedImage,
    zoom: f32,
}

impl HeicPreview {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let mut preview = Self {
            path: path.to_path_buf(),
            image: RetainedImage::from_color_image(
                path.to_string_lossy(),
                egui::ColorImage {
                    size: [1, 1],
                    pixels: vec![Color32::TRANSPARENT],
                },
            ),
            zoom: 1.0,
        };
        preview.reload(path)?;
        Ok(preview)
    }

    fn reload(&mut self, path: &Path) -> anyhow::Result<()> {
        let bytes = std::fs::read(path)
            .with_context(|| format!("Reading HEIC image: {}", path.display()))?;
        let temp_dir = directories::ProjectDirs::from("com", "Dev_Row", "AssetViewer")
            .ok_or_else(|| anyhow::anyhow!("Unable to resolve cache directory"))?
            .cache_dir()
            .join("heic_preview");
        std::fs::create_dir_all(&temp_dir)
            .with_context(|| format!("Creating HEIC cache dir: {}", temp_dir.display()))?;
        let out_path = temp_dir.join("preview.png");
        let input_path = temp_dir.join("input.heic");
        std::fs::write(&input_path, bytes)
            .with_context(|| format!("Writing HEIC temp file: {}", input_path.display()))?;

        let status = std::process::Command::new("ffmpeg")
            .args([
                "-y",
                "-i",
                &input_path.to_string_lossy(),
                "-frames:v",
                "1",
                &out_path.to_string_lossy(),
            ])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();

        match status {
            Ok(s) if s.success() && out_path.exists() => {
                let png_bytes = std::fs::read(&out_path).with_context(|| {
                    format!("Reading decoded HEIC preview: {}", out_path.display())
                })?;
                self.image = RetainedImage::from_image_bytes(path.to_string_lossy(), &png_bytes)
                    .map_err(|err| anyhow::anyhow!(err))?;
                let _ = std::fs::remove_file(&input_path);
                let _ = std::fs::remove_file(&out_path);
                Ok(())
            }
            Ok(_) => Err(anyhow::anyhow!(
                "HEIC decoding requires ffmpeg with HEIC/HEIF support in PATH"
            )),
            Err(err) => Err(anyhow::anyhow!(err)),
        }
    }
}

impl PreviewUi for HeicPreview {
    fn ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Design Tools");

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
                if ui.button("Reset").clicked() {
                    self.zoom = 1.0;
                }
            });
        });

        ui.add_space(8.0);

        ScrollArea::both()
            .id_source("heic_preview_scroll")
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                let size = self.image.size_vec2() * self.zoom;
                self.image.show_size(ui, size);
            });

        ui.add_space(12.0);
        ui.separator();
        ui.heading("Asset Info");
        ui.group(|ui| {
            ui.label(
                RichText::new(self.path.to_string_lossy())
                    .small()
                    .color(Color32::GRAY),
            );
            ui.label("HEIC/HEIF via ffmpeg fallback");
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

struct AudioPeaks {
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
    loading_rx: Option<std::sync::mpsc::Receiver<anyhow::Result<AudioPeaks>>>,
}

impl AudioPreview {
    fn cache_dir() -> Option<PathBuf> {
        directories::ProjectDirs::from("com", "Dev_Row", "AssetViewer")
            .map(|pd| pd.cache_dir().join("audio_peaks"))
    }

    fn get_cache_path(path: &Path) -> Option<PathBuf> {
        let mut hasher = DefaultHasher::new();
        path.to_string_lossy().hash(&mut hasher);
        let hash = hasher.finish();
        Self::cache_dir().map(|cache_dir| {
            let _ = std::fs::create_dir_all(&cache_dir);
            cache_dir.join(format!("{:x}.json", hash))
        })
    }

    pub fn load(
        path: &Path,
        audio_handle: Option<&rodio::OutputStreamHandle>,
        ctx: &EguiContext,
    ) -> anyhow::Result<Self> {
        // Fast path: cache hit (synchronous, fast)
        if let Some(cache_path) = Self::get_cache_path(path) {
            if cache_path.exists() {
                if let Ok(raw) = std::fs::read_to_string(&cache_path) {
                    if let Ok(cached) = serde_json::from_str::<AudioCacheData>(&raw) {
                        let sink = audio_handle.and_then(|h| rodio::Sink::try_new(h).ok());
                        return Ok(Self {
                            path: path.to_path_buf(),
                            points_left: cached.points_left,
                            points_right: cached.points_right,
                            duration_s: cached.duration_s,
                            zoom: 1.0,
                            sink,
                            show_stereo: false,
                            loading_rx: None,
                        });
                    }
                }
            }
        }

        // Slow path: decode in background thread
        let (tx, rx) = std::sync::mpsc::channel::<anyhow::Result<AudioPeaks>>();
        let path_owned = path.to_path_buf();
        let ctx_clone = ctx.clone();
        std::thread::spawn(move || {
            let result = (|| -> anyhow::Result<AudioPeaks> {
                use symphonia::core::audio::SampleBuffer;
                use symphonia::core::codecs::DecoderOptions;
                use symphonia::core::formats::FormatOptions;
                use symphonia::core::io::MediaSourceStream;
                use symphonia::core::meta::MetadataOptions;
                use symphonia::core::probe::Hint;

                let file = std::fs::File::open(&path_owned)
                    .with_context(|| format!("Opening audio: {}", path_owned.display()))?;
                let mss = MediaSourceStream::new(Box::new(file), Default::default());
                let mut hint = Hint::new();
                if let Some(ext) = path_owned.extension().and_then(|e| e.to_str()) {
                    hint.with_extension(ext);
                }
                let probed = symphonia::default::get_probe()
                    .format(
                        &hint,
                        mss,
                        &FormatOptions::default(),
                        &MetadataOptions::default(),
                    )
                    .with_context(|| "Probing audio format")?;
                let mut format = probed.format;
                let track = format
                    .default_track()
                    .ok_or_else(|| anyhow::anyhow!("No default audio track"))?;
                let mut decoder = symphonia::default::get_codecs()
                    .make(&track.codec_params, &DecoderOptions::default())
                    .with_context(|| "Creating decoder")?;

                let sample_rate = track
                    .codec_params
                    .sample_rate
                    .ok_or_else(|| anyhow::anyhow!("Missing sample_rate"))?
                    as f64;
                let mut left_samples = Vec::new();
                let mut right_samples = Vec::new();
                let mut total_frames = 0u64;

                loop {
                    let packet = match format.next_packet() {
                        Ok(p) => p,
                        Err(err) => {
                            use symphonia::core::errors::Error;
                            if let Error::IoError(ref e) = err {
                                if e.kind() == std::io::ErrorKind::UnexpectedEof {
                                    break;
                                }
                            }
                            return Err(err).context("Reading audio packet");
                        }
                    };
                    let decoded = decoder
                        .decode(&packet)
                        .with_context(|| "Decoding audio packet")?;
                    let spec = *decoded.spec();
                    let duration = decoded.capacity() as u64;
                    let mut sample_buf = SampleBuffer::<f32>::new(duration, spec);
                    sample_buf.copy_interleaved_ref(decoded);
                    let chan_count = spec.channels.count();
                    let samples = sample_buf.samples();
                    for frame in samples.chunks(chan_count) {
                        if chan_count >= 2 {
                            left_samples.push(frame[0]);
                            right_samples.push(frame[1]);
                        } else {
                            left_samples.push(frame[0]);
                            right_samples.push(frame[0]);
                        }
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

                if let Some(cache_path) = AudioPreview::get_cache_path(&path_owned) {
                    let data = AudioCacheData {
                        points_left: pts_l.clone(),
                        points_right: pts_r.clone(),
                        duration_s,
                    };
                    if let Ok(raw) = serde_json::to_string(&data) {
                        let _ = std::fs::write(cache_path, raw);
                    }
                }

                Ok(AudioPeaks {
                    points_left: pts_l,
                    points_right: pts_r,
                    duration_s,
                })
            })();
            let _ = tx.send(result);
            ctx_clone.request_repaint();
        });

        let sink = audio_handle.and_then(|h| rodio::Sink::try_new(h).ok());
        Ok(Self {
            path: path.to_path_buf(),
            points_left: Vec::new(),
            points_right: Vec::new(),
            duration_s: 0.0,
            zoom: 1.0,
            sink,
            show_stereo: false,
            loading_rx: Some(rx),
        })
    }

    pub fn play(&mut self) {
        if let Some(sink) = &self.sink {
            if sink.empty() {
                if let Ok(file) = std::fs::File::open(&self.path) {
                    if let Ok(source) = rodio::Decoder::new(std::io::BufReader::new(file)) {
                        sink.append(source);
                    }
                }
            }
            sink.play();
        }
    }
    pub fn pause(&mut self) {
        if let Some(sink) = &self.sink {
            sink.pause();
        }
    }
    pub fn stop(&mut self) {
        if let Some(sink) = &self.sink {
            sink.stop();
        }
    }
    pub fn seek(&mut self, seconds: f64) {
        if let Some(sink) = &self.sink {
            let _ = sink.try_seek(std::time::Duration::from_secs_f64(seconds));
        }
    }
}

impl PreviewUi for AudioPreview {
    fn ui(&mut self, ui: &mut egui::Ui) {
        // Poll background decode result
        if let Some(rx) = &self.loading_rx {
            match rx.try_recv() {
                Ok(Ok(peaks)) => {
                    self.points_left = peaks.points_left;
                    self.points_right = peaks.points_right;
                    self.duration_s = peaks.duration_s;
                    self.loading_rx = None;
                }
                Ok(Err(_)) | Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.loading_rx = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            }
        }

        let mut current_pos = 0.0;
        if let Some(sink) = &self.sink {
            current_pos = sink.get_pos().as_secs_f64();
        }
        ui.horizontal(|ui| {
            ui.label(format!("Duration: {:.2}s", self.duration_s));
            ui.add_space(10.0);
            ui.add(Slider::new(&mut self.zoom, 0.25..=8.0).logarithmic(true));
            ui.checkbox(&mut self.show_stereo, "Stereo");
        });
        ui.horizontal(|ui| {
            if let Some(sink) = &self.sink {
                if sink.is_paused() || sink.empty() {
                    if ui.button("▶ Play").clicked() {
                        self.play();
                    }
                } else if ui.button("⏸ Pause").clicked() {
                    self.pause();
                }
                if ui.button("⏹ Stop").clicked() {
                    self.stop();
                }
            } else {
                ui.colored_label(Color32::RED, "Audio output not available");
            }
            let mut pos = current_pos;
            if ui
                .add(Slider::new(&mut pos, 0.0..=self.duration_s).show_value(false))
                .changed()
            {
                self.seek(pos);
            }
            ui.label(format!("{:.1}s / {:.1}s", current_pos, self.duration_s));
        });
        let view_end = (self.duration_s / self.zoom).max(0.01);
        if self.loading_rx.is_some() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Decoding audio…");
            });
        } else if self.show_stereo {
            let line_l = Line::new(PlotPoints::from(self.points_left.clone()))
                .color(Color32::from_rgb(100, 150, 255))
                .name("Left");
            let line_r = Line::new(PlotPoints::from(self.points_right.clone()))
                .color(Color32::from_rgb(255, 150, 100))
                .name("Right");
            Plot::new("waveform_plot_stereo")
                .height(260.0)
                .include_y(0.0)
                .include_y(1.0)
                .include_x(0.0)
                .include_x(view_end)
                .legend(egui_plot::Legend::default())
                .show(ui, |plot_ui| {
                    plot_ui.line(line_l);
                    plot_ui.line(line_r);
                });
        } else {
            let line = Line::new(PlotPoints::from(self.points_left.clone()));
            Plot::new("waveform_plot")
                .height(260.0)
                .include_y(0.0)
                .include_y(1.0)
                .include_x(0.0)
                .include_x(view_end)
                .show(ui, |plot_ui| {
                    plot_ui.line(line);
                });
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

fn deserialize_action_points<'de, D>(deserializer: D) -> Result<Option<ActionPoints>, D::Error>
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
                data.animations.insert(
                    "Default".to_string(),
                    BitWiseAnimation {
                        frames,
                        fps: 12.0,
                        looping: true,
                    },
                );
            }
        }
        let mut anim_names: Vec<String> = data.animations.keys().cloned().collect();
        anim_names.sort();
        let default_anim = anim_names
            .first()
            .cloned()
            .unwrap_or_else(|| "None".to_string());
        Ok(Self {
            path: path.to_path_buf(),
            data,
            selected_animation: default_anim,
            current_anim_frame_idx: 0,
            last_update: 0.0,
            use_detected_height: true,
            is_playing: false,
            zoom: 4.0,
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
        let height = if self.use_detected_height {
            self.get_effective_height(frame_idx)
        } else {
            self.data.height
        };
        if width == 0 || height == 0 {
            return None;
        }
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
                egui::ColorImage {
                    size: [width as usize, height as usize],
                    pixels,
                },
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
                if ui
                    .button(if self.is_playing {
                        "⏸ Pause"
                    } else {
                        "▶ Play"
                    })
                    .clicked()
                {
                    self.is_playing = !self.is_playing;
                }
                ui.separator();
                ui.label("Zoom:");
                ui.add(Slider::new(&mut self.zoom, 0.1..=64.0).logarithmic(true));
                if ui.button("Reset").clicked() {
                    self.zoom = 4.0;
                }
            });
            ui.horizontal(|ui| {
                ui.label("Animation:");
                ComboBox::from_id_source("s_anim")
                    .selected_text(&self.selected_animation)
                    .show_ui(ui, |ui| {
                        let mut names: Vec<_> = self.data.animations.keys().collect();
                        names.sort();
                        for n in names {
                            if ui
                                .selectable_label(&self.selected_animation == n, n)
                                .clicked()
                            {
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
                        self.current_anim_frame_idx =
                            (self.current_anim_frame_idx + 1) % anim.frames.len();
                        self.last_update = time;
                    }
                    ui.ctx().request_repaint();
                }
                anim.frames
                    .get(self.current_anim_frame_idx)
                    .cloned()
                    .unwrap_or(0)
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
                        let size =
                            vec2(self.data.width as f32 * self.zoom, eff_h as f32 * self.zoom);

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
            ui.label(format!(
                "Resolution: {}x{}",
                self.data.width, self.data.height
            ));
            ui.label(format!("Total Frames: {}", self.data.frames.len()));
            ui.label(
                RichText::new(self.path.to_string_lossy())
                    .small()
                    .color(Color32::GRAY),
            );
        });
    }
}

// --- 3D Model Preview ---

pub struct Model3DPreview {
    path: PathBuf,
    info: Option<ModelInfo>,
    error: Option<String>,
    stl_mesh: Option<Vec<[[f32; 3]; 3]>>,
    // Design Tools
    rotation: [f32; 3],
    scale: f32,
    stl_wireframe: bool,
    stl_solid: bool,
}
struct ModelInfo {
    mesh_count: usize,
    material_count: usize,
    vertex_count: usize,
    triangle_count: usize,
    min_bounds: [f32; 3],
    max_bounds: [f32; 3],
    meshes: Vec<String>,
}
impl Model3DPreview {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let mut preview = Self {
            path: path.to_path_buf(),
            info: None,
            error: None,
            stl_mesh: None,
            rotation: [0.0, 0.0, 0.0],
            scale: 1.0,
            stl_wireframe: true,
            stl_solid: false,
        };
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        if ext == "obj" {
            match tobj::load_obj(
                path,
                &tobj::LoadOptions {
                    single_index: true,
                    triangulate: true,
                    ..Default::default()
                },
            ) {
                Ok((models, materials)) => {
                    let mut vertex_count = 0;
                    let mut triangle_count = 0;
                    let mut min_b = [f32::MAX; 3];
                    let mut max_b = [f32::MIN; 3];
                    let mesh_names: Vec<String> = models.iter().map(|m| m.name.clone()).collect();
                    for model in models {
                        let mesh = &model.mesh;
                        vertex_count += mesh.positions.len() / 3;
                        triangle_count += mesh.indices.len() / 3;
                        for i in 0..(mesh.positions.len() / 3) {
                            let p = [
                                mesh.positions[i * 3],
                                mesh.positions[i * 3 + 1],
                                mesh.positions[i * 3 + 2],
                            ];
                            for j in 0..3 {
                                min_b[j] = min_b[j].min(p[j]);
                                max_b[j] = max_b[j].max(p[j]);
                            }
                        }
                    }
                    preview.info = Some(ModelInfo {
                        mesh_count: mesh_names.len(),
                        material_count: materials.map_or(0, |m| m.len()),
                        vertex_count,
                        triangle_count,
                        min_bounds: min_b,
                        max_bounds: max_b,
                        meshes: mesh_names,
                    });
                }
                Err(err) => preview.error = Some(format!("Error loading OBJ: {:?}", err)),
            }
        } else if ext == "stl" {
            match std::fs::File::open(path) {
                Ok(file) => {
                    let mut reader = std::io::BufReader::new(file);
                    match stl_io::read_stl(&mut reader) {
                        Ok(stl) => {
                            let mut vertex_count = 0usize;
                            let mut triangle_count = 0usize;
                            let mut min_b = [f32::MAX; 3];
                            let mut max_b = [f32::MIN; 3];
                            let mut tris = Vec::with_capacity(stl.faces.len());
                            for tri in &stl.faces {
                                triangle_count += 1;
                                let mut tri_pts = [[0.0; 3]; 3];
                                for (i, &vertex_idx) in tri.vertices.iter().enumerate() {
                                    vertex_count += 1;
                                    let p = stl.vertices[vertex_idx].0;
                                    tri_pts[i] = p;
                                    min_b[0] = min_b[0].min(p[0]);
                                    min_b[1] = min_b[1].min(p[1]);
                                    min_b[2] = min_b[2].min(p[2]);
                                    max_b[0] = max_b[0].max(p[0]);
                                    max_b[1] = max_b[1].max(p[1]);
                                    max_b[2] = max_b[2].max(p[2]);
                                }
                                tris.push(tri_pts);
                            }

                            preview.stl_mesh = Some(tris);
                            preview.info = Some(ModelInfo {
                                mesh_count: 1,
                                material_count: 0,
                                vertex_count,
                                triangle_count,
                                min_bounds: min_b,
                                max_bounds: max_b,
                                meshes: vec!["STL Mesh".to_string()],
                            });
                        }
                        Err(err) => preview.error = Some(format!("Error loading STL: {:?}", err)),
                    }
                }
                Err(err) => preview.error = Some(format!("Error opening STL: {:?}", err)),
            }
        } else {
            match easy_gltf::load(path) {
                Ok(scenes) => {
                    let mut vertex_count = 0;
                    let mut triangle_count = 0;
                    let mut min_b = [f32::MAX; 3];
                    let mut max_b = [f32::MIN; 3];
                    let mut mesh_names = Vec::new();
                    for scene in scenes {
                        for model in scene.models {
                            mesh_names.push("GLTF Mesh".to_string());
                            let vertices = model.vertices();
                            vertex_count += vertices.len();
                            triangle_count += model
                                .indices()
                                .map_or(vertices.len() / 3, |idx| idx.len() / 3);
                            for v in vertices {
                                let p = v.position;
                                min_b[0] = min_b[0].min(p.x);
                                min_b[1] = min_b[1].min(p.y);
                                min_b[2] = min_b[2].min(p.z);
                                max_b[0] = max_b[0].max(p.x);
                                max_b[1] = max_b[1].max(p.y);
                                max_b[2] = max_b[2].max(p.z);
                            }
                        }
                    }
                    preview.info = Some(ModelInfo {
                        mesh_count: mesh_names.len(),
                        material_count: mesh_names.len(),
                        vertex_count,
                        triangle_count,
                        min_bounds: min_b,
                        max_bounds: max_b,
                        meshes: mesh_names,
                    });
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
                self.scale = (self.scale * zoom_factor).clamp(0.001, 100.0);
            }
        }

        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.label("Rotation:");
                ui.add(
                    egui::DragValue::new(&mut self.rotation[0])
                        .speed(1.0)
                        .prefix("X: "),
                );
                ui.add(
                    egui::DragValue::new(&mut self.rotation[1])
                        .speed(1.0)
                        .prefix("Y: "),
                );
                ui.add(
                    egui::DragValue::new(&mut self.rotation[2])
                        .speed(1.0)
                        .prefix("Z: "),
                );
            });
            ui.horizontal(|ui| {
                ui.label("Scale:   ");
                ui.add(Slider::new(&mut self.scale, 0.001..=10.0).logarithmic(true));
                let wire_changed = ui.checkbox(&mut self.stl_wireframe, "Wireframe").changed();
                let solid_changed = ui.checkbox(&mut self.stl_solid, "Solid").changed();
                if wire_changed && self.stl_wireframe {
                    self.stl_solid = false;
                }
                if solid_changed && self.stl_solid {
                    self.stl_wireframe = false;
                }
                if ui.button("Reset").clicked() {
                    self.rotation = [0.0, 0.0, 0.0];
                    self.scale = 1.0;
                    self.stl_wireframe = true;
                    self.stl_solid = false;
                }
            });
        });

        ui.add_space(8.0);

        if let Some(tris) = &self.stl_mesh {
            let available_w = ui.available_width();
            let viewport_h = 320.0;
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(available_w, viewport_h), egui::Sense::drag());
            if response.dragged() {
                let delta = response.drag_delta();
                self.rotation[1] += delta.x * 0.35;
                self.rotation[0] += delta.y * 0.35;
                ui.ctx().request_repaint();
            }
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, 6.0, Color32::from_rgb(18, 20, 24));
            painter.rect_stroke(rect, 6.0, egui::Stroke::new(1.0, Color32::from_gray(70)));
            painter.text(
                rect.left_top() + egui::vec2(10.0, 10.0),
                egui::Align2::LEFT_TOP,
                "STL 3D View",
                egui::FontId::proportional(14.0),
                Color32::LIGHT_GRAY,
            );

            let mut projected: Vec<([egui::Pos2; 3], f32, f32)> = Vec::new();
            let rx = self.rotation[0].to_radians();
            let ry = self.rotation[1].to_radians();
            let rz = self.rotation[2].to_radians();
            let (sx, cx) = rx.sin_cos();
            let (sy, cy) = ry.sin_cos();
            let (sz, cz) = rz.sin_cos();

            let rotate = |p: [f32; 3]| {
                let (mut x, mut y, mut z) = (p[0], p[1], p[2]);
                let ny = y * cx - z * sx;
                let nz = y * sx + z * cx;
                y = ny;
                z = nz;

                let nx = x * cy + z * sy;
                let nz = -x * sy + z * cy;
                x = nx;
                z = nz;

                let nx = x * cz - y * sz;
                let ny = x * sz + y * cz;
                [nx, ny, z]
            };

            let mut center = [0.0f32; 3];
            let mut count = 0.0f32;
            for tri in tris {
                for p in tri {
                    center[0] += p[0];
                    center[1] += p[1];
                    center[2] += p[2];
                    count += 1.0;
                }
            }
            if count > 0.0 {
                center[0] /= count;
                center[1] /= count;
                center[2] /= count;
            }

            let mut max_extent = 1.0f32;
            if let Some(info) = &self.info {
                let extents = [
                    info.max_bounds[0] - info.min_bounds[0],
                    info.max_bounds[1] - info.min_bounds[1],
                    info.max_bounds[2] - info.min_bounds[2],
                ];
                max_extent = extents.into_iter().fold(0.0, f32::max).max(1.0);
            }

            for tri in tris {
                let mut pts = [egui::Pos2::ZERO; 3];
                let mut avg_z = 0.0f32;
                let mut world = [[0.0f32; 3]; 3];
                for (i, p) in tri.iter().enumerate() {
                    let mut v = [
                        (p[0] - center[0]) * self.scale,
                        (p[1] - center[1]) * self.scale,
                        (p[2] - center[2]) * self.scale,
                    ];
                    v = rotate(v);
                    world[i] = v;
                    avg_z += v[2];
                    let perspective = 2.5 / (2.5 + v[2] / max_extent.max(0.001));
                    let x = rect.center().x
                        + v[0] * rect.width().min(rect.height()) * 0.35 * perspective;
                    let y = rect.center().y
                        - v[1] * rect.width().min(rect.height()) * 0.35 * perspective;
                    pts[i] = egui::pos2(x, y);
                }
                let u = [
                    world[1][0] - world[0][0],
                    world[1][1] - world[0][1],
                    world[1][2] - world[0][2],
                ];
                let v = [
                    world[2][0] - world[0][0],
                    world[2][1] - world[0][1],
                    world[2][2] - world[0][2],
                ];
                let normal = [
                    u[1] * v[2] - u[2] * v[1],
                    u[2] * v[0] - u[0] * v[2],
                    u[0] * v[1] - u[1] * v[0],
                ];
                let normal_len =
                    (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2])
                        .sqrt()
                        .max(0.0001);
                let normal = [
                    normal[0] / normal_len,
                    normal[1] / normal_len,
                    normal[2] / normal_len,
                ];
                let light_dir = [0.35f32, 0.45f32, 0.82f32];
                let light_len = (light_dir[0] * light_dir[0]
                    + light_dir[1] * light_dir[1]
                    + light_dir[2] * light_dir[2])
                    .sqrt();
                let light_dir = [
                    light_dir[0] / light_len,
                    light_dir[1] / light_len,
                    light_dir[2] / light_len,
                ];
                let intensity = (normal[0] * light_dir[0]
                    + normal[1] * light_dir[1]
                    + normal[2] * light_dir[2])
                    .max(0.0)
                    .mul_add(0.75, 0.25);
                projected.push((pts, avg_z / 3.0, intensity));
            }
            projected.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

            for (pts, _, intensity) in projected {
                if self.stl_solid {
                    let shade = (intensity * 255.0).clamp(35.0, 255.0) as u8;
                    painter.add(egui::Shape::convex_polygon(
                        vec![pts[0], pts[1], pts[2]],
                        Color32::from_rgba_unmultiplied(shade / 2, shade, 255, 255),
                        egui::Stroke::NONE,
                    ));
                }
                if self.stl_wireframe {
                    let stroke = egui::Stroke::new(1.2, Color32::from_rgb(120, 180, 255));
                    painter.line_segment([pts[0], pts[1]], stroke);
                    painter.line_segment([pts[1], pts[2]], stroke);
                    painter.line_segment([pts[2], pts[0]], stroke);
                }
            }
        }

        ui.add_space(12.0);
        ui.separator();
        ui.add_space(4.0);

        ui.heading("Asset Info");
        ui.label(
            RichText::new(self.path.to_string_lossy())
                .small()
                .color(Color32::GRAY),
        );

        if let Some(info) = &self.info {
            ui.group(|ui| {
                ui.label(format!("Meshes: {}", info.mesh_count));
                ui.label(format!("Materials: {}", info.material_count));
                ui.label(format!("Vertices: {}", info.vertex_count));
                ui.label(format!("Triangles: {}", info.triangle_count));
                ui.separator();
                ui.label(format!(
                    "Min: [{:.2}, {:.2}, {:.2}]",
                    info.min_bounds[0], info.min_bounds[1], info.min_bounds[2]
                ));
                ui.label(format!(
                    "Max: [{:.2}, {:.2}, {:.2}]",
                    info.max_bounds[0], info.max_bounds[1], info.max_bounds[2]
                ));
            });
            ui.collapsing("List Meshes", |ui| {
                for name in &info.meshes {
                    ui.label(format!("• {}", name));
                }
            });
        } else if let Some(err) = &self.error {
            ui.colored_label(Color32::LIGHT_RED, err);
        }
    }
}

// --- Blender Preview ---

pub struct BlendPreview {
    path: PathBuf,
    thumbnail: Option<RetainedImage>,
    error: Option<String>,
    blender_path: Option<String>,
    is_generating: bool,
    render_rx: Option<std::sync::mpsc::Receiver<Result<Vec<u8>, String>>>,
}
impl BlendPreview {
    pub fn new(path: &Path, blender_path: Option<&str>) -> Self {
        let mut preview = Self {
            path: path.to_path_buf(),
            thumbnail: None,
            error: None,
            blender_path: blender_path.map(|s| s.to_string()),
            is_generating: false,
            render_rx: None,
        };
        if let Some(cp) = preview.cache_path() {
            if cp.exists() {
                if let Ok(bytes) = std::fs::read(&cp) {
                    preview.thumbnail =
                        RetainedImage::from_image_bytes(cp.to_string_lossy(), &bytes).ok();
                }
            } else if blender_path.is_none() {
                preview.error = Some("Set Blender path in Settings.".to_string());
            }
        }
        preview
    }
    fn cache_dir() -> Option<PathBuf> {
        directories::ProjectDirs::from("com", "Dev_Row", "AssetViewer")
            .map(|pd| pd.cache_dir().join("blender_thumbs"))
    }

    fn cache_path(&self) -> Option<PathBuf> {
        let mut hasher = DefaultHasher::new();
        self.path.to_string_lossy().hash(&mut hasher);
        Self::cache_dir().map(|cache_dir| {
            let _ = std::fs::create_dir_all(&cache_dir);
            cache_dir.join(format!("{:x}.png", hasher.finish()))
        })
    }
    pub fn generate_thumbnail(&mut self, ctx: &EguiContext) {
        let (Some(blender_exe), Some(out_path)) = (&self.blender_path, self.cache_path()) else {
            return;
        };
        if self.is_generating {
            return;
        }
        self.is_generating = true;
        self.error = None;
        let blender_exe = blender_exe.clone();
        let blend_path = self.path.clone();
        let (tx, rx) = std::sync::mpsc::channel::<Result<Vec<u8>, String>>();
        let ctx_clone = ctx.clone();
        std::thread::spawn(move || {
            let python_cmd = format!(
                "import bpy; bpy.ops.wm.open_mainfile(filepath=r'{}'); bpy.context.scene.render.filepath = r'{}'; bpy.ops.render.render(write_still=True)",
                blend_path.to_string_lossy(), out_path.to_string_lossy()
            );
            let result = std::process::Command::new(&blender_exe)
                .arg("--background")
                .arg("--python-expr")
                .arg(&python_cmd)
                .output();
            let msg = match result {
                Ok(output) if output.status.success() && out_path.exists() => {
                    std::fs::read(&out_path).map_err(|e| e.to_string())
                }
                Ok(_) => Err("Blender render failed.".to_string()),
                Err(e) => Err(e.to_string()),
            };
            let _ = tx.send(msg);
            ctx_clone.request_repaint();
        });
        self.render_rx = Some(rx);
    }
}
impl PreviewUi for BlendPreview {
    fn ui(&mut self, ui: &mut egui::Ui) {
        // Poll Blender render result
        if let Some(rx) = &self.render_rx {
            match rx.try_recv() {
                Ok(Ok(bytes)) => {
                    let label = self.path.to_string_lossy().to_string();
                    self.thumbnail = RetainedImage::from_image_bytes(label, &bytes).ok();
                    self.error = if self.thumbnail.is_none() {
                        Some("Failed to decode rendered image.".to_string())
                    } else {
                        None
                    };
                    self.is_generating = false;
                    self.render_rx = None;
                }
                Ok(Err(msg)) => {
                    self.error = Some(msg);
                    self.is_generating = false;
                    self.render_rx = None;
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.is_generating = false;
                    self.render_rx = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            }
        }

        ui.heading("Blender File");
        if self.is_generating {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Rendering… (this may take several minutes)");
            });
        } else if let Some(thumb) = &mut self.thumbnail {
            ScrollArea::both().show(ui, |ui| {
                thumb.show(ui);
            });
            if ui.button("Regenerate").clicked() {
                self.generate_thumbnail(ui.ctx());
            }
        } else {
            if let Some(err) = &self.error {
                ui.colored_label(Color32::YELLOW, err);
            }
            if self.blender_path.is_some() && ui.button("Generate").clicked() {
                self.generate_thumbnail(ui.ctx());
            }
        }
    }
}

// --- Video Preview ---

fn parse_ffmpeg_rate(rate: &str) -> Option<f64> {
    let rate = rate.trim();
    if let Some((num, den)) = rate.split_once('/') {
        let num = num.parse::<f64>().ok()?;
        let den = den.parse::<f64>().ok()?;
        if den > 0.0 && num.is_finite() && den.is_finite() {
            let value = num / den;
            return (value > 0.0 && value.is_finite()).then_some(value);
        }
        return None;
    }

    rate.parse::<f64>()
        .ok()
        .filter(|value| *value > 0.0 && value.is_finite())
}

struct VideoMetadata {
    width: u32,
    height: u32,
    duration: f64,
    fps: f64,
    first_frame_bytes: Option<Vec<u8>>,
    status: Option<String>,
}

struct VideoFrame {
    request_id: u64,
    timestamp: f64,
    image: egui::ColorImage,
}

pub struct VideoPreview {
    path: PathBuf,
    file_size: u64,

    // Playback state
    texture_handle: Option<egui::TextureHandle>,
    current_time: f64,
    duration: f64,
    is_playing: bool,
    last_frame_display: Instant,
    playback_fps: f64,

    // Info
    width: u32,
    height: u32,
    scaled_width: u32,
    scaled_height: u32,

    // Background metadata load (ffprobe + first frame)
    loading_rx: Option<std::sync::mpsc::Receiver<anyhow::Result<VideoMetadata>>>,

    // Streaming playback channel (bounded to 30 frames for backpressure during pause)
    playback_rx: Option<std::sync::mpsc::Receiver<egui::ColorImage>>,
    playback_child: Option<std::process::Child>,

    // Single-frame loading for paused seeking and frame stepping
    frame_rx: Option<std::sync::mpsc::Receiver<anyhow::Result<VideoFrame>>>,
    frame_request_id: u64,
    frame_error: Option<String>,
    video_status: Option<String>,
}

impl VideoPreview {
    /// Computes scaled dimensions capped at 640px wide, keeping aspect and even numbers.
    fn scaled_dims(width: u32, height: u32) -> (u32, u32) {
        const MAX_W: u32 = 640;
        if width == 0 || height == 0 {
            return (640, 360);
        }
        if width <= MAX_W {
            return (width, height);
        }
        let h = ((height as f64 * MAX_W as f64 / width as f64).round() as u32).max(2);
        let h = (h / 2) * 2;
        (MAX_W, h)
    }

    pub fn load(path: &Path, ctx: &EguiContext) -> anyhow::Result<Self> {
        let meta = std::fs::metadata(path)?;
        let path_owned = path.to_path_buf();
        let (tx, rx) = std::sync::mpsc::channel::<anyhow::Result<VideoMetadata>>();
        let ctx_clone = ctx.clone();
        std::thread::spawn(move || {
            let result = (|| -> anyhow::Result<VideoMetadata> {
                let output = std::process::Command::new("ffprobe")
                    .args([
                        "-v",
                        "error",
                        "-select_streams",
                        "v:0",
                        "-show_entries",
                        "stream=width,height,r_frame_rate,avg_frame_rate:format=duration",
                        "-of",
                        "default=noprint_wrappers=1:nokey=1",
                        &path_owned.to_string_lossy(),
                    ])
                    .output()
                    .with_context(|| {
                        "Running ffprobe. Install ffprobe/ffmpeg and make sure they are in PATH."
                    })?;

                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    anyhow::bail!(
                        "ffprobe failed for {}{}",
                        path_owned.display(),
                        if stderr.trim().is_empty() {
                            "".to_string()
                        } else {
                            format!(": {}", stderr.trim())
                        }
                    );
                }

                let out_str = String::from_utf8_lossy(&output.stdout);
                let mut lines = out_str.lines();
                let width: u32 = lines.next().unwrap_or("0").parse().unwrap_or(0);
                let height: u32 = lines.next().unwrap_or("0").parse().unwrap_or(0);
                let r_frame_rate = lines.next().unwrap_or("0/0");
                let avg_frame_rate = lines.next().unwrap_or("0/0");
                let duration: f64 = lines.next().unwrap_or("0").parse().unwrap_or(0.0);
                let fps = parse_ffmpeg_rate(avg_frame_rate)
                    .or_else(|| parse_ffmpeg_rate(r_frame_rate))
                    .unwrap_or(30.0);

                // Extract first frame as JPEG thumbnail
                let frame_out = std::process::Command::new("ffmpeg")
                    .args([
                        "-ss",
                        "0",
                        "-i",
                        &path_owned.to_string_lossy(),
                        "-frames:v",
                        "1",
                        "-f",
                        "image2",
                        "-vcodec",
                        "mjpeg",
                        "pipe:1",
                    ])
                    .stderr(std::process::Stdio::piped())
                    .output();
                let (first_frame_bytes, status) = match frame_out {
                    Ok(output) if output.status.success() && !output.stdout.is_empty() => {
                        (Some(output.stdout), None)
                    }
                    Ok(output) => {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        let detail = if stderr.trim().is_empty() {
                            "ffmpeg could not extract the first frame.".to_string()
                        } else {
                            format!("ffmpeg could not extract the first frame: {}", stderr.trim())
                        };
                        (None, Some(detail))
                    }
                    Err(err) => (
                        None,
                        Some(format!(
                            "Running ffmpeg failed. Install ffmpeg and make sure it is in PATH: {err}"
                        )),
                    ),
                };

                Ok(VideoMetadata {
                    width,
                    height,
                    duration,
                    fps,
                    first_frame_bytes,
                    status,
                })
            })();
            let _ = tx.send(result);
            ctx_clone.request_repaint();
        });

        Ok(Self {
            path: path.to_path_buf(),
            file_size: meta.len(),
            texture_handle: None,
            current_time: 0.0,
            duration: 0.0,
            is_playing: false,
            last_frame_display: Instant::now(),
            playback_fps: 30.0,
            width: 0,
            height: 0,
            scaled_width: 0,
            scaled_height: 0,
            loading_rx: Some(rx),
            playback_rx: None,
            playback_child: None,
            frame_rx: None,
            frame_request_id: 0,
            frame_error: None,
            video_status: None,
        })
    }

    fn frame_interval(&self) -> f64 {
        1.0 / self.playback_fps.max(1.0)
    }

    fn clamp_time(&self, timestamp: f64) -> f64 {
        timestamp.clamp(0.0, self.duration.max(0.0))
    }

    fn set_texture(&mut self, ctx: &EguiContext, name: &str, image: egui::ColorImage) {
        if let Some(handle) = &mut self.texture_handle {
            handle.set(image, egui::TextureOptions::default());
        } else {
            self.texture_handle =
                Some(ctx.load_texture(name, image, egui::TextureOptions::default()));
        }
    }

    fn seek_to(&mut self, timestamp: f64, should_play: bool, ctx: &EguiContext) {
        let timestamp = self.clamp_time(timestamp);
        self.current_time = timestamp;
        self.stop_streaming();
        self.is_playing = should_play;
        if should_play {
            self.start_streaming(timestamp, ctx);
        } else {
            self.request_frame(timestamp, ctx);
        }
    }

    fn step_frame(&mut self, direction: f64, ctx: &EguiContext) {
        let timestamp = self.current_time + (self.frame_interval() * direction);
        self.seek_to(timestamp, false, ctx);
    }

    fn request_frame(&mut self, timestamp: f64, ctx: &EguiContext) {
        if self.scaled_width == 0 || self.scaled_height == 0 {
            return;
        }

        self.frame_request_id = self.frame_request_id.wrapping_add(1);
        self.frame_error = None;

        let request_id = self.frame_request_id;
        let timestamp = self.clamp_time(timestamp);
        let path = self.path.clone();
        let sw = self.scaled_width;
        let sh = self.scaled_height;
        let frame_bytes = (sw * sh * 4) as usize;
        let (tx, rx) = std::sync::mpsc::channel::<anyhow::Result<VideoFrame>>();
        self.frame_rx = Some(rx);
        let ctx_clone = ctx.clone();

        std::thread::spawn(move || {
            let result = (|| -> anyhow::Result<VideoFrame> {
                let output = std::process::Command::new("ffmpeg")
                    .args([
                        "-ss",
                        &format!("{:.3}", timestamp),
                        "-i",
                        &path.to_string_lossy(),
                        "-frames:v",
                        "1",
                        "-f",
                        "rawvideo",
                        "-pix_fmt",
                        "rgba",
                        "-vf",
                        &format!("scale={}:{}", sw, sh),
                        "pipe:1",
                    ])
                    .stderr(std::process::Stdio::null())
                    .output()
                    .with_context(|| {
                        format!(
                            "Extracting video frame. Install ffmpeg and make sure it is in PATH: {}",
                            path.display()
                        )
                    })?;

                if !output.status.success() || output.stdout.len() < frame_bytes {
                    anyhow::bail!("Unable to extract frame at {:.2}s", timestamp);
                }

                let image = egui::ColorImage::from_rgba_unmultiplied(
                    [sw as usize, sh as usize],
                    &output.stdout[..frame_bytes],
                );
                Ok(VideoFrame {
                    request_id,
                    timestamp,
                    image,
                })
            })();
            let _ = tx.send(result);
            ctx_clone.request_repaint();
        });
    }

    /// Starts a streaming ffmpeg process from `timestamp`.
    /// Raw RGBA frames are sent over a bounded channel; the reader thread blocks on pause.
    fn start_streaming(&mut self, timestamp: f64, ctx: &EguiContext) {
        self.stop_streaming();
        if self.scaled_width == 0 || self.scaled_height == 0 {
            return;
        }

        let sw = self.scaled_width;
        let sh = self.scaled_height;
        let frame_bytes = (sw * sh * 4) as usize;

        let (tx, rx) = std::sync::mpsc::sync_channel::<egui::ColorImage>(30);
        self.playback_rx = Some(rx);

        let path = self.path.clone();
        let ctx_clone = ctx.clone();

        let mut child = match std::process::Command::new("ffmpeg")
            .args([
                "-ss",
                &format!("{:.3}", timestamp),
                "-i",
                &path.to_string_lossy(),
                "-f",
                "rawvideo",
                "-pix_fmt",
                "rgba",
                "-vf",
                &format!("scale={}:{}", sw, sh),
                "pipe:1",
            ])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            Ok(c) => c,
            Err(err) => {
                self.video_status = Some(format!(
                    "Unable to start ffmpeg playback. Install ffmpeg and make sure it is in PATH: {err}"
                ));
                self.is_playing = false;
                return;
            }
        };

        let stdout = match child.stdout.take() {
            Some(s) => s,
            None => {
                self.video_status = Some("Unable to read ffmpeg playback output.".to_string());
                self.is_playing = false;
                return;
            }
        };
        self.playback_child = Some(child);

        std::thread::spawn(move || {
            use std::io::Read;
            let mut reader = std::io::BufReader::new(stdout);
            let mut buf = vec![0u8; frame_bytes];
            loop {
                let mut total = 0;
                while total < frame_bytes {
                    match reader.read(&mut buf[total..]) {
                        Ok(0) => return,
                        Ok(n) => total += n,
                        Err(_) => return,
                    }
                }
                let image =
                    egui::ColorImage::from_rgba_unmultiplied([sw as usize, sh as usize], &buf);
                if tx.send(image).is_err() {
                    return;
                }
                ctx_clone.request_repaint();
            }
        });
    }

    /// Stops any active streaming process and clears the channel.
    fn stop_streaming(&mut self) {
        self.playback_rx = None;
        if let Some(mut child) = self.playback_child.take() {
            let _ = child.kill();
        }
    }
}

impl Drop for VideoPreview {
    fn drop(&mut self) {
        self.stop_streaming();
    }
}

impl PreviewUi for VideoPreview {
    fn ui(&mut self, ui: &mut egui::Ui) {
        // ── Poll ffprobe metadata ──────────────────────────────────────────
        if let Some(rx) = &self.loading_rx {
            match rx.try_recv() {
                Ok(Ok(meta)) => {
                    self.width = meta.width;
                    self.height = meta.height;
                    self.duration = meta.duration;
                    self.playback_fps = meta.fps;
                    self.video_status = meta.status;
                    let (sw, sh) = Self::scaled_dims(meta.width, meta.height);
                    self.scaled_width = sw;
                    self.scaled_height = sh;
                    // Show first frame thumbnail immediately
                    if let Some(bytes) = meta.first_frame_bytes {
                        if let Ok(img) = image::load_from_memory(&bytes) {
                            let img =
                                img.resize_exact(sw, sh, image::imageops::FilterType::Nearest);
                            let rgba = img.to_rgba8();
                            let color_image = egui::ColorImage::from_rgba_unmultiplied(
                                [sw as usize, sh as usize],
                                rgba.as_raw(),
                            );
                            self.texture_handle = Some(ui.ctx().load_texture(
                                "video_thumb",
                                color_image,
                                egui::TextureOptions::default(),
                            ));
                        }
                    }
                    self.loading_rx = None;
                    if self.texture_handle.is_none() {
                        self.request_frame(0.0, ui.ctx());
                    }
                }
                Ok(Err(err)) => {
                    self.video_status = Some(err.to_string());
                    self.loading_rx = None;
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.video_status = Some("Video metadata loading stopped.".to_string());
                    self.loading_rx = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            }
        }

        // ── Poll single-frame seek requests ───────────────────────────────
        if let Some(rx) = &self.frame_rx {
            match rx.try_recv() {
                Ok(Ok(frame)) => {
                    if frame.request_id == self.frame_request_id {
                        self.current_time = frame.timestamp;
                        self.set_texture(ui.ctx(), "video_seek_frame", frame.image);
                        self.frame_error = None;
                    }
                    self.frame_rx = None;
                }
                Ok(Err(err)) => {
                    self.frame_error = Some(err.to_string());
                    self.frame_rx = None;
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.frame_error = Some("Frame extraction stopped".to_string());
                    self.frame_rx = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            }
        }

        // ── Consume frames from streaming channel ──────────────────────────
        let mut stream_ended = false;
        let maybe_frame = if self.is_playing {
            if let Some(rx) = &self.playback_rx {
                let frame_interval = self.frame_interval();
                if self.last_frame_display.elapsed().as_secs_f64() >= frame_interval {
                    match rx.try_recv() {
                        Ok(img) => Some(img),
                        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                            stream_ended = true;
                            None
                        }
                        Err(std::sync::mpsc::TryRecvError::Empty) => None,
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        if stream_ended {
            self.stop_streaming();
            self.is_playing = false;
        }

        if let Some(image) = maybe_frame {
            let frame_interval = self.frame_interval();
            self.current_time = (self.current_time + frame_interval).min(self.duration);
            self.last_frame_display = Instant::now();
            self.set_texture(ui.ctx(), "video_frame", image);
            if self.current_time >= self.duration {
                self.stop_streaming();
                self.is_playing = false;
            }
        }

        if self.is_playing {
            ui.ctx().request_repaint();
        }

        // ── UI ────────────────────────────────────────────────────────────
        ui.heading("Design Tools");

        if self.loading_rx.is_some() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Loading video…");
            });
        }

        ui.group(|ui| {
            ui.horizontal(|ui| {
                let was_playing = self.is_playing;

                if ui
                    .button(if self.is_playing {
                        "⏸ Pause"
                    } else {
                        "▶ Play"
                    })
                    .clicked()
                {
                    self.is_playing = !self.is_playing;
                    if self.is_playing && self.playback_rx.is_none() {
                        let t = self.current_time;
                        self.start_streaming(t, ui.ctx());
                    } else if !self.is_playing {
                        self.stop_streaming();
                    }
                }

                if ui.button("⏹ Stop").clicked() {
                    self.seek_to(0.0, false, ui.ctx());
                }

                if ui.button("⏪ Frame").clicked() {
                    self.step_frame(-1.0, ui.ctx());
                }

                if ui.button("Frame ⏩").clicked() {
                    self.step_frame(1.0, ui.ctx());
                }

                let mut seek = self.current_time;
                if ui
                    .add(Slider::new(&mut seek, 0.0..=self.duration.max(0.01)).show_value(false))
                    .changed()
                {
                    self.seek_to(seek, was_playing, ui.ctx());
                }

                ui.label(format!("{:.1}s / {:.1}s", self.current_time, self.duration));
            });
        });

        if self.frame_rx.is_some() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Loading frame…");
            });
        }

        if let Some(error) = &self.frame_error {
            ui.colored_label(Color32::RED, error);
        }

        if let Some(status) = &self.video_status {
            ui.colored_label(Color32::LIGHT_RED, status);
        }

        ui.add_space(8.0);

        if let Some(handle) = &self.texture_handle {
            let available_width = ui.available_width();
            let aspect = if self.scaled_height > 0 {
                self.scaled_width as f32 / self.scaled_height as f32
            } else {
                16.0 / 9.0
            };
            let draw_w = available_width.min(self.scaled_width as f32);
            let draw_size = vec2(draw_w, draw_w / aspect);
            ScrollArea::vertical()
                .id_source("vid_scroll")
                .max_height(ui.available_height() - 120.0)
                .show(ui, |ui| {
                    ui.add(egui::Image::from_texture(egui::load::SizedTexture::new(
                        handle.id(),
                        draw_size,
                    )));
                });
        }

        ui.add_space(12.0);
        ui.separator();
        ui.heading("Asset Info");
        ui.group(|ui| {
            ui.label(format!("Resolution: {}x{}", self.width, self.height));
            ui.label(format!("FPS: {:.2}", self.playback_fps));
            ui.label(format!(
                "Size: {:.2} MB",
                self.file_size as f64 / 1_048_576.0
            ));
            ui.label(
                RichText::new(self.path.to_string_lossy())
                    .small()
                    .color(Color32::GRAY),
            );
            if ui.button("🎬 Open in System Player").clicked() {
                let _ = open::that(&self.path);
            }
        });
    }
}
// --- PDF Preview ---

enum PdfRenderEvent {
    Page { index: usize, bytes: Vec<u8> },
    Error(String),
    Done,
}

#[derive(Deserialize)]
struct GitHubReleaseAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Deserialize)]
struct GitHubRelease {
    assets: Vec<GitHubReleaseAsset>,
}

pub struct PdfPreview {
    path: PathBuf,
    pages: Vec<Option<RetainedImage>>,
    page_count: usize,
    zoom: f32,
    error: Option<String>,
    loading_rx: Option<std::sync::mpsc::Receiver<PdfRenderEvent>>,
}

impl PdfPreview {
    fn library_dir() -> anyhow::Result<PathBuf> {
        directories::ProjectDirs::from("com", "Dev_Row", "AssetViewer")
            .map(|pd| pd.cache_dir().join("pdfium"))
            .ok_or_else(|| anyhow::anyhow!("Unable to determine PDFium cache directory"))
    }

    fn pdfium_asset_name() -> &'static str {
        match (std::env::consts::OS, std::env::consts::ARCH) {
            ("windows", "x86_64") => "pdfium-win-x64.tgz",
            ("windows", "x86") => "pdfium-win-x86.tgz",
            ("windows", "aarch64") => "pdfium-win-arm64.tgz",
            ("linux", "x86_64") => "pdfium-linux-x64.tgz",
            ("linux", "aarch64") => "pdfium-linux-arm64.tgz",
            ("macos", "aarch64") => "pdfium-mac-arm64.tgz",
            ("macos", "x86_64") => "pdfium-mac-x64.tgz",
            _ => "pdfium-win-x64.tgz",
        }
    }

    fn ensure_pdfium_library() -> anyhow::Result<PathBuf> {
        if let Ok(explicit) = std::env::var("PDFIUM_LIB_PATH") {
            let explicit_path = PathBuf::from(explicit);
            if explicit_path.is_file() {
                if let Some(dir) = explicit_path.parent() {
                    pdfium::set_library_location(dir.to_string_lossy().as_ref());
                }
                return Ok(explicit_path);
            }
            if explicit_path.is_dir() {
                let dll = explicit_path.join(if cfg!(target_os = "windows") {
                    "pdfium.dll"
                } else {
                    "libpdfium.so"
                });
                if dll.exists() {
                    pdfium::set_library_location(explicit_path.to_string_lossy().as_ref());
                    return Ok(dll);
                }
            }
        }

        let lib_dir = Self::library_dir()?;
        std::fs::create_dir_all(&lib_dir)
            .with_context(|| format!("Creating PDFium cache dir: {}", lib_dir.display()))?;

        let bundled_name = if cfg!(target_os = "windows") {
            "pdfium.dll"
        } else if cfg!(target_os = "macos") {
            "libpdfium.dylib"
        } else {
            "libpdfium.so"
        };
        let direct_path = lib_dir.join(bundled_name);
        if direct_path.exists() {
            pdfium::set_library_location(lib_dir.to_string_lossy().as_ref());
            return Ok(direct_path);
        }

        let client = reqwest::blocking::Client::builder()
            .user_agent("AssetViewer/0.1")
            .build()
            .context("Building HTTP client for PDFium download")?;

        let release: GitHubRelease = client
            .get("https://api.github.com/repos/bblanchon/pdfium-binaries/releases/latest")
            .send()
            .and_then(|r| r.error_for_status())
            .context("Fetching latest PDFium release metadata")?
            .json()
            .context("Parsing PDFium release metadata")?;

        let asset_name = Self::pdfium_asset_name();
        let asset = release
            .assets
            .into_iter()
            .find(|asset| asset.name == asset_name)
            .ok_or_else(|| anyhow::anyhow!("Could not find PDFium asset {asset_name}"))?;

        let archive_bytes = client
            .get(asset.browser_download_url)
            .send()
            .and_then(|r| r.error_for_status())
            .context("Downloading PDFium archive")?
            .bytes()
            .context("Reading PDFium archive bytes")?;

        let gz = flate2::read::GzDecoder::new(std::io::Cursor::new(archive_bytes));
        let mut archive = tar::Archive::new(gz);
        archive
            .unpack(&lib_dir)
            .with_context(|| format!("Extracting PDFium archive to {}", lib_dir.display()))?;

        let extracted = walkdir::WalkDir::new(&lib_dir)
            .into_iter()
            .filter_map(|entry| entry.ok())
            .find(|entry| {
                entry.file_type().is_file()
                    && entry
                        .file_name()
                        .to_string_lossy()
                        .eq_ignore_ascii_case(bundled_name)
            })
            .map(|entry| entry.into_path())
            .ok_or_else(|| anyhow::anyhow!("Extracted archive did not contain {bundled_name}"))?;

        if extracted != direct_path {
            std::fs::copy(&extracted, &direct_path)
                .with_context(|| format!("Copying PDFium library to {}", direct_path.display()))?;
        }

        pdfium::set_library_location(lib_dir.to_string_lossy().as_ref());
        Ok(direct_path)
    }

    fn bitmap_to_png_bytes(bitmap: &pdfium::PdfiumBitmap, key: &str) -> anyhow::Result<Vec<u8>> {
        let temp_dir = Self::library_dir()?.join("renders");
        std::fs::create_dir_all(&temp_dir)
            .with_context(|| format!("Creating PDF render dir: {}", temp_dir.display()))?;
        let temp_path = temp_dir.join(format!("{}.png", key));
        let temp_path_str = temp_path.to_string_lossy().to_string();
        bitmap
            .save(&temp_path_str, image::ImageFormat::Png)
            .with_context(|| format!("Saving PDF preview page to {}", temp_path.display()))?;
        let bytes = std::fs::read(&temp_path)
            .with_context(|| format!("Reading PDF preview page {}", temp_path.display()))?;
        let _ = std::fs::remove_file(&temp_path);
        Ok(bytes)
    }

    pub fn load(path: &Path, ctx: &EguiContext) -> anyhow::Result<Self> {
        let path_owned = path.to_path_buf();
        let (tx, rx) = std::sync::mpsc::channel::<PdfRenderEvent>();
        let ctx_clone = ctx.clone();

        std::thread::spawn(move || {
            let send_error = |tx: &std::sync::mpsc::Sender<PdfRenderEvent>, msg: String| {
                let _ = tx.send(PdfRenderEvent::Error(msg));
                let _ = tx.send(PdfRenderEvent::Done);
            };

            let result = (|| -> anyhow::Result<()> {
                let _ = Self::ensure_pdfium_library()?;
                let document = pdfium::PdfiumDocument::new_from_path(&path_owned, None)
                    .map_err(|err| anyhow::anyhow!("Failed to open PDF: {err}"))?;

                let render_config = pdfium::PdfiumRenderConfig::new().with_height(1600);
                let mut index = 0usize;
                loop {
                    let page = match document.page(index as i32) {
                        Ok(page) => page,
                        Err(_) => break,
                    };
                    let bitmap = page.render(&render_config).map_err(|err| {
                        anyhow::anyhow!("Failed to render page {}: {err}", index + 1)
                    })?;
                    let mut hasher = DefaultHasher::new();
                    path_owned.to_string_lossy().hash(&mut hasher);
                    index.hash(&mut hasher);
                    let key = format!("{:x}", hasher.finish());
                    let bytes = Self::bitmap_to_png_bytes(&bitmap, &key)?;
                    if tx.send(PdfRenderEvent::Page { index, bytes }).is_err() {
                        return Ok(());
                    }
                    ctx_clone.request_repaint();
                    index += 1;
                }

                Ok(())
            })();

            if let Err(err) = result {
                send_error(&tx, err.to_string());
            }

            let _ = tx.send(PdfRenderEvent::Done);
            ctx_clone.request_repaint();
        });

        Ok(Self {
            path: path.to_path_buf(),
            pages: Vec::new(),
            page_count: 0,
            zoom: 1.0,
            error: None,
            loading_rx: Some(rx),
        })
    }
}

impl PreviewUi for PdfPreview {
    fn ui(&mut self, ui: &mut egui::Ui) {
        if let Some(rx) = &self.loading_rx {
            let mut clear_rx = false;
            loop {
                match rx.try_recv() {
                    Ok(PdfRenderEvent::Page { index, bytes }) => {
                        let label = format!("{}_page_{}", self.path.display(), index + 1);
                        if index == self.pages.len() {
                            self.pages
                                .push(RetainedImage::from_image_bytes(label, &bytes).ok());
                        } else if index < self.pages.len() {
                            self.pages[index] = RetainedImage::from_image_bytes(label, &bytes).ok();
                        } else {
                            self.pages.resize_with(index + 1, || None);
                            self.pages[index] = RetainedImage::from_image_bytes(label, &bytes).ok();
                        }
                        self.page_count = self.pages.len();
                    }
                    Ok(PdfRenderEvent::Error(err)) => {
                        self.error = Some(err);
                    }
                    Ok(PdfRenderEvent::Done) => {
                        clear_rx = true;
                        break;
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        clear_rx = true;
                        break;
                    }
                }
            }

            if clear_rx {
                self.loading_rx = None;
            }
        }

        ui.heading("PDF");

        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.label(format!("Pages: {}", self.page_count.max(self.pages.len())));
                ui.separator();
                ui.label("Zoom:");
                ui.add(Slider::new(&mut self.zoom, 0.25..=4.0).logarithmic(true));
                if ui.button("Reset").clicked() {
                    self.zoom = 1.0;
                }
                if ui.button("Open in System Viewer").clicked() {
                    let _ = open::that(&self.path);
                }
            });
        });

        if let Some(err) = &self.error {
            ui.colored_label(Color32::LIGHT_RED, err);
        }

        if self.loading_rx.is_some() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Rendering PDF pages...");
            });
        }

        ui.add_space(8.0);

        ScrollArea::vertical()
            .id_source("pdf_preview_scroll")
            .show(ui, |ui| {
                if self.pages.is_empty() {
                    ui.label("Waiting for the first page...");
                }

                for (index, page) in self.pages.iter().enumerate() {
                    ui.label(format!("Page {}", index + 1));
                    if let Some(page) = page {
                        let size = page.size_vec2() * self.zoom;
                        page.show_size(ui, size);
                    } else {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label("Rendering...");
                        });
                    }
                    ui.add_space(12.0);
                    if index + 1 < self.pages.len() {
                        ui.separator();
                        ui.add_space(12.0);
                    }
                }
            });

        ui.add_space(12.0);
        ui.separator();
        ui.heading("Asset Info");
        ui.group(|ui| {
            ui.label(format!("Pages: {}", self.page_count.max(self.pages.len())));
            ui.label(
                RichText::new(self.path.to_string_lossy())
                    .small()
                    .color(Color32::GRAY),
            );
        });
    }
}

// --- ZIP Preview ---

pub struct ZipEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub children: HashMap<String, ZipEntry>,
}
pub struct ZipPreview {
    pub root: ZipEntry,
    pub file_count: usize,
    pub total_size: u64,
}
impl ZipPreview {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let file = std::fs::File::open(path)?;
        let mut archive = zip::ZipArchive::new(file)?;
        let mut root = ZipEntry {
            name: "/".into(),
            is_dir: true,
            size: 0,
            children: HashMap::new(),
        };
        let mut total_size = 0u64;
        for i in 0..archive.len() {
            let file = archive.by_index(i)?;
            let parts: Vec<&str> = file.name().split('/').filter(|s| !s.is_empty()).collect();
            let mut current = &mut root;
            for (idx, part) in parts.iter().enumerate() {
                let is_last = idx == parts.len() - 1;
                current = current
                    .children
                    .entry(part.to_string())
                    .or_insert_with(|| ZipEntry {
                        name: part.to_string(),
                        is_dir: !is_last || file.is_dir(),
                        size: if is_last { file.size() } else { 0 },
                        children: HashMap::new(),
                    });
            }
            if !file.is_dir() {
                total_size = total_size.saturating_add(file.size());
            }
        }
        Ok(Self {
            root,
            file_count: archive.len(),
            total_size,
        })
    }
    fn render_entry(ui: &mut egui::Ui, entry: &ZipEntry) {
        if entry.is_dir {
            ui.collapsing(format!("📁 {}", entry.name), |ui| {
                let mut keys: Vec<_> = entry.children.keys().collect();
                keys.sort();
                for k in keys {
                    Self::render_entry(ui, &entry.children[k]);
                }
            });
        } else {
            ui.label(format!("📄 {} ({} bytes)", entry.name, entry.size));
        }
    }
}
impl PreviewUi for ZipPreview {
    fn ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("ZIP Archive");
        ui.label(format!("Files: {}", self.file_count));
        ui.label(format!("Total size: {} bytes", self.total_size));
        ScrollArea::vertical().id_source("z").show(ui, |ui| {
            let mut keys: Vec<_> = self.root.children.keys().collect();
            keys.sort();
            for k in keys {
                Self::render_entry(ui, &self.root.children[k]);
            }
        });
    }
}

// --- 7Z Preview ---

pub struct SevenZipPreview {
    root: ZipEntry,
    file_count: usize,
    total_size: u64,
}

impl SevenZipPreview {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let archive = sevenz_rust2::Archive::open(path)
            .with_context(|| format!("Opening 7z archive: {}", path.display()))?;

        let mut root = ZipEntry {
            name: "/".into(),
            is_dir: true,
            size: 0,
            children: HashMap::new(),
        };
        let mut total_size = 0u64;
        let mut file_count = 0usize;

        for entry in &archive.files {
            let parts: Vec<&str> = entry.name().split('/').filter(|s| !s.is_empty()).collect();
            if parts.is_empty() {
                continue;
            }

            let mut current = &mut root;
            for (idx, part) in parts.iter().enumerate() {
                let is_last = idx == parts.len() - 1;
                current = current
                    .children
                    .entry(part.to_string())
                    .or_insert_with(|| ZipEntry {
                        name: part.to_string(),
                        is_dir: !is_last || entry.is_directory(),
                        size: if is_last { entry.size() } else { 0 },
                        children: HashMap::new(),
                    });
            }

            if !entry.is_directory() {
                file_count += 1;
                total_size = total_size.saturating_add(entry.size());
            }
        }

        Ok(Self {
            root,
            file_count,
            total_size,
        })
    }
}

impl PreviewUi for SevenZipPreview {
    fn ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("7Z Archive");
        ui.label(format!("Files: {}", self.file_count));
        ui.label(format!("Total size: {} bytes", self.total_size));
        ScrollArea::vertical().id_source("7z").show(ui, |ui| {
            let mut keys: Vec<_> = self.root.children.keys().collect();
            keys.sort();
            for k in keys {
                ZipPreview::render_entry(ui, &self.root.children[k]);
            }
        });
    }
}

// --- RAR Preview ---

pub struct RarPreview {
    root: ZipEntry,
    file_count: usize,
    total_size: u64,
}

impl RarPreview {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let path_string = path.to_string_lossy().to_string();
        let archive = unrar::Archive::new(&path_string)
            .open_for_listing()
            .with_context(|| format!("Opening RAR archive: {}", path.display()))?;

        let mut root = ZipEntry {
            name: "/".into(),
            is_dir: true,
            size: 0,
            children: HashMap::new(),
        };
        let mut file_count = 0usize;
        let mut total_size = 0u64;

        for item in archive {
            let header = item.with_context(|| format!("Reading RAR header: {}", path.display()))?;
            let filename = header.filename.to_string_lossy();
            let parts: Vec<&str> = filename.split('/').filter(|s| !s.is_empty()).collect();
            if parts.is_empty() {
                continue;
            }

            let mut current = &mut root;
            for (idx, part) in parts.iter().enumerate() {
                let is_last = idx == parts.len() - 1;
                current = current
                    .children
                    .entry(part.to_string())
                    .or_insert_with(|| ZipEntry {
                        name: part.to_string(),
                        is_dir: !is_last || filename.ends_with('/'),
                        size: if is_last { header.unpacked_size } else { 0 },
                        children: HashMap::new(),
                    });
            }

            if !filename.ends_with('/') {
                file_count += 1;
                total_size = total_size.saturating_add(header.unpacked_size);
            }
        }

        Ok(Self {
            root,
            file_count,
            total_size,
        })
    }
}

impl PreviewUi for RarPreview {
    fn ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("RAR Archive");
        ui.label(format!("Files: {}", self.file_count));
        ui.label(format!("Total size: {} bytes", self.total_size));
        ScrollArea::vertical().id_source("rar").show(ui, |ui| {
            let mut keys: Vec<_> = self.root.children.keys().collect();
            keys.sort();
            for k in keys {
                ZipPreview::render_entry(ui, &self.root.children[k]);
            }
        });
    }
}

// --- Text Preview ---

// --- Font Preview ---

pub struct FontPreview {
    path: PathBuf,
    font_name: String,
    font: fontdue::Font,
    sample_text: String,
    preview: Option<RetainedImage>,
    error: Option<String>,
    font_size: f32,
    zoom: f32,
}

impl FontPreview {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let bytes =
            std::fs::read(path).with_context(|| format!("Reading font: {}", path.display()))?;
        let font = fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default())
            .map_err(|err| anyhow::anyhow!("Failed to parse font: {err}"))?;
        let font_name = font
            .horizontal_line_metrics(32.0)
            .map(|_| {
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Font")
                    .to_string()
            })
            .unwrap_or_else(|| {
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Font")
                    .to_string()
            });

        let mut preview = Self {
            path: path.to_path_buf(),
            font_name,
            font,
            sample_text: "Aa Bb Cc 0123456789".to_string(),
            preview: None,
            error: None,
            font_size: 56.0,
            zoom: 1.0,
        };
        preview.refresh_preview();
        Ok(preview)
    }

    fn refresh_preview(&mut self) {
        let text = self.sample_text.clone();
        let size = self.font_size * self.zoom;
        let padding = 20usize;
        let line_gap = 12usize;

        let mut glyphs = Vec::new();
        let mut cursor_x = padding as i32;
        let mut baseline_y = padding as i32;
        let mut max_x = 0i32;
        let mut max_y = 0i32;

        let line_metrics =
            self.font
                .horizontal_line_metrics(size)
                .unwrap_or(fontdue::LineMetrics {
                    ascent: size * 0.8,
                    descent: -size * 0.2,
                    line_gap: 0.0,
                    new_line_size: size,
                });
        let line_height = (line_metrics.ascent - line_metrics.descent + line_gap as f32) as i32;

        for ch in text.chars() {
            if ch == '\n' {
                cursor_x = padding as i32;
                baseline_y += line_height;
                continue;
            }

            let (metrics, bitmap) = self.font.rasterize(ch, size);
            let x = cursor_x + metrics.xmin;
            let y = baseline_y - metrics.ymin;
            glyphs.push((x, y, metrics.width, metrics.height, bitmap));
            cursor_x += metrics.advance_width.round() as i32;
            max_x = max_x.max(cursor_x);
            max_y = max_y.max(baseline_y + metrics.height as i32);
        }

        let width = (max_x + padding as i32).max(320) as usize;
        let height = (max_y + padding as i32)
            .max((baseline_y + line_height + padding as i32).max(220))
            as usize;
        let mut pixels = vec![Color32::from_rgb(24, 24, 28); width * height];

        for (x, y, gw, gh, bitmap) in glyphs {
            for row in 0..gh {
                for col in 0..gw {
                    let alpha = bitmap[row * gw + col];
                    if alpha == 0 {
                        continue;
                    }
                    let px = x + col as i32;
                    let py = y + row as i32;
                    if px < 0 || py < 0 {
                        continue;
                    }
                    let px = px as usize;
                    let py = py as usize;
                    if px >= width || py >= height {
                        continue;
                    }
                    let idx = py * width + px;
                    pixels[idx] = Color32::from_rgba_unmultiplied(240, 240, 240, alpha);
                }
            }
        }

        let image = egui::ColorImage {
            size: [width, height],
            pixels,
        };
        self.preview = Some(RetainedImage::from_color_image("font_preview", image));
    }
}

impl PreviewUi for FontPreview {
    fn ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Font");
        ui.label(RichText::new(self.font_name.clone()).strong());
        ui.label(
            RichText::new(self.path.to_string_lossy())
                .small()
                .color(Color32::GRAY),
        );

        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.label("Sample:");
                let changed = ui.text_edit_singleline(&mut self.sample_text).changed();
                ui.label("Size:");
                let size_changed = ui
                    .add(Slider::new(&mut self.font_size, 8.0..=128.0))
                    .changed();
                ui.label("Zoom:");
                let zoom_changed = ui
                    .add(Slider::new(&mut self.zoom, 0.25..=8.0).logarithmic(true))
                    .changed();
                if ui.button("Refresh").clicked() || changed || size_changed {
                    self.refresh_preview();
                }
                if zoom_changed {
                    self.refresh_preview();
                }
            });
        });

        ui.add_space(8.0);

        if ui.rect_contains_pointer(ui.max_rect()) {
            let delta = ui.input(|i| i.smooth_scroll_delta.y);
            if delta != 0.0 {
                let zoom_factor = (delta / 200.0).exp();
                self.zoom = (self.zoom * zoom_factor).clamp(0.25, 8.0);
                self.refresh_preview();
            }
        }

        if let Some(preview) = &self.preview {
            ScrollArea::both()
                .id_source("font_preview_scroll")
                .show(ui, |ui| {
                    preview.show(ui);
                });
        } else if let Some(err) = &self.error {
            ui.colored_label(Color32::LIGHT_RED, err);
        } else {
            ui.label("No preview available.");
        }
    }
}

pub struct TextPreview {
    pub path: PathBuf,
    pub content: String,
}
impl TextPreview {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        Ok(Self {
            path: path.to_path_buf(),
            content: std::fs::read_to_string(path)?,
        })
    }
}
impl PreviewUi for TextPreview {
    fn ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Text");
        ui.label(
            RichText::new(self.path.to_string_lossy())
                .small()
                .color(Color32::GRAY),
        );
        ScrollArea::both().show(ui, |ui| {
            ui.add(
                egui::TextEdit::multiline(&mut self.content)
                    .font(egui::TextStyle::Monospace)
                    .desired_width(f32::INFINITY),
            );
        });
    }
}

// --- Stub Preview ---

pub struct StubPreview {
    title: String,
    detail: String,
}
impl StubPreview {
    pub fn new(title: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            detail: detail.into(),
        }
    }
}
impl PreviewUi for StubPreview {
    fn ui(&mut self, ui: &mut egui::Ui) {
        ui.heading(&self.title);
        ui.label(&self.detail);
    }
}

// --- Preview Cache ---

pub fn clear_disk_preview_caches() -> anyhow::Result<()> {
    for dir in [AudioPreview::cache_dir(), BlendPreview::cache_dir()]
        .into_iter()
        .flatten()
    {
        if dir.exists() {
            std::fs::remove_dir_all(&dir)
                .with_context(|| format!("Removing cache directory: {}", dir.display()))?;
        }
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("Recreating cache directory: {}", dir.display()))?;
    }
    Ok(())
}

#[derive(Default)]
pub struct PreviewCache {
    items: HashMap<PathBuf, (Instant, Preview)>,
}
impl PreviewCache {
    pub fn clear(&mut self) {
        self.items.clear();
    }

    pub fn get_or_build(
        &mut self,
        ctx: &EguiContext,
        path: &Path,
        kind: PreviewKind,
        audio_handle: Option<&rodio::OutputStreamHandle>,
        blender_path: Option<&str>,
    ) -> anyhow::Result<&mut Preview> {
        let key = path.to_path_buf();
        if !self.items.contains_key(&key) {
            let preview = match kind {
                PreviewKind::Image => Preview::Image(ImagePreview::load(path)?),
                PreviewKind::Heic => Preview::Heic(HeicPreview::load(path)?),
                PreviewKind::Audio => Preview::Audio(AudioPreview::load(path, audio_handle, ctx)?),
                PreviewKind::Model3D => Preview::Model3D(Model3DPreview::load(path)?),
                PreviewKind::Blend => Preview::Blend(BlendPreview::new(path, blender_path)),
                PreviewKind::Sprite => Preview::Sprite(SpritePreview::load(path)?),
                PreviewKind::Pdf => Preview::Pdf(PdfPreview::load(path, ctx)?),
                PreviewKind::SevenZip => Preview::SevenZip(SevenZipPreview::load(path)?),
                PreviewKind::Rar => Preview::Rar(RarPreview::load(path)?),
                PreviewKind::Font => Preview::Font(FontPreview::load(path)?),
                PreviewKind::Video => Preview::Video(VideoPreview::load(path, ctx)?),
                PreviewKind::Zip => Preview::Zip(ZipPreview::load(path)?),
                PreviewKind::Text => Preview::Text(TextPreview::load(path)?),
                PreviewKind::Unknown => Preview::Stub(StubPreview::new("Unknown", "No preview.")),
            };
            self.items.insert(key.clone(), (Instant::now(), preview));
        }
        if let Some((t, _)) = self.items.get_mut(&key) {
            *t = Instant::now();
        }
        Ok(&mut self.items.get_mut(&key).unwrap().1)
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_ffmpeg_rate, PreviewKind};
    use std::path::Path;

    #[test]
    fn preview_kind_detects_supported_extensions() {
        let cases = [
            ("image.PNG", PreviewKind::Image),
            ("photo.heic", PreviewKind::Heic),
            ("sound.flac", PreviewKind::Audio),
            ("clip.webm", PreviewKind::Video),
            ("mesh.glb", PreviewKind::Model3D),
            ("scene.blend", PreviewKind::Blend),
            ("anim.sprite", PreviewKind::Sprite),
            ("doc.pdf", PreviewKind::Pdf),
            ("archive.7z", PreviewKind::SevenZip),
            ("packed.rar", PreviewKind::Rar),
            ("font.otf", PreviewKind::Font),
            ("bundle.zip", PreviewKind::Zip),
            ("readme.md", PreviewKind::Text),
            ("unknown.asset", PreviewKind::Unknown),
        ];

        for (name, expected) in cases {
            assert_eq!(PreviewKind::from_path(Path::new(name)), expected);
        }
    }

    #[test]
    fn ffmpeg_rate_parser_handles_ratios_and_bad_values() {
        assert_eq!(parse_ffmpeg_rate("30/1"), Some(30.0));
        assert_eq!(
            parse_ffmpeg_rate("30000/1001").map(|v| (v * 100.0).round() / 100.0),
            Some(29.97)
        );
        assert_eq!(parse_ffmpeg_rate("24"), Some(24.0));
        assert_eq!(parse_ffmpeg_rate("0/0"), None);
        assert_eq!(parse_ffmpeg_rate("bad"), None);
    }
}
