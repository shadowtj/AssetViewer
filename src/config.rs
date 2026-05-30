use anyhow::Context;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FavoriteFolder {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    pub root_path: Option<String>,
    #[serde(default)]
    pub favorites: Vec<FavoriteFolder>,
    #[serde(default)]
    pub last_favorite_index: Option<usize>,
    #[serde(default)]
    pub blender_path: Option<String>,
    #[serde(default)]
    pub flatten_export: bool,
    #[serde(default)]
    pub export_profile: String,
    #[serde(default)]
    pub view_mode: String,
    #[serde(default)]
    pub sort_mode: String,
    #[serde(default = "default_sort_ascending")]
    pub sort_ascending: bool,
}

fn default_sort_ascending() -> bool {
    true
}

#[derive(Debug, Clone)]
pub struct ConfigStore {
    path: PathBuf,
}

impl Default for ConfigStore {
    fn default() -> Self {
        let path = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("asset_viewer_config.json");
        Self { path }
    }
}

impl ConfigStore {
    pub fn new() -> anyhow::Result<Self> {
        let proj_dirs = ProjectDirs::from("com", "Dev_Row", "AssetViewer");
        let path = if let Some(pd) = proj_dirs {
            let cfg_dir = pd.config_dir();
            fs::create_dir_all(cfg_dir).ok();
            cfg_dir.join("config.json")
        } else {
            std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join("asset_viewer_config.json")
        };

        Ok(Self { path })
    }

    pub fn load(&self) -> anyhow::Result<AppConfig> {
        if !self.path.exists() {
            return Ok(AppConfig::default());
        }
        let raw = fs::read_to_string(&self.path)
            .with_context(|| format!("Reading config: {}", self.path.display()))?;
        let cfg: AppConfig = serde_json::from_str(&raw).with_context(|| "Parsing config JSON")?;
        Ok(cfg)
    }

    pub fn save(&self, config: &AppConfig) -> anyhow::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).ok();
        }
        let raw = serde_json::to_string_pretty(config)?;
        fs::write(&self.path, raw)
            .with_context(|| format!("Writing config: {}", self.path.display()))?;
        Ok(())
    }
}
