use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

fn default_parallel_chunk_threshold() -> u64 { 50000 }
fn default_chunk_size() -> usize { 10000 }

#[derive(Serialize, Deserialize, Clone)]
pub struct AppSettings {
    pub max_parallel_dumps: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_dump_dir: Option<String>,
    #[serde(default)]
    pub table_exclude_patterns: Vec<String>,
    /// Mínimo de linhas para ativar dump paralelo por chunk em uma tabela
    #[serde(default = "default_parallel_chunk_threshold")]
    pub parallel_chunk_threshold: u64,
    /// Linhas por chunk no dump paralelo
    #[serde(default = "default_chunk_size")]
    pub chunk_size: usize,
}

impl Default for AppSettings {
    fn default() -> Self {
        AppSettings {
            max_parallel_dumps: 5,
            custom_dump_dir: None,
            table_exclude_patterns: vec![],
            parallel_chunk_threshold: default_parallel_chunk_threshold(),
            chunk_size: default_chunk_size(),
        }
    }
}

fn settings_file(config_dir: &Path) -> PathBuf {
    config_dir.join("settings.json")
}

pub fn load(config_dir: &Path) -> AppSettings {
    let path = settings_file(config_dir);

    let data = match std::fs::read_to_string(&path) {
        Ok(d) => d,
        Err(_) => return AppSettings::default(),
    };

    serde_json::from_str(&data).unwrap_or_default()
}

pub fn save(config_dir: &Path, settings: &AppSettings) -> Result<(), String> {
    std::fs::create_dir_all(config_dir).map_err(|e| e.to_string())?;

    let path = settings_file(config_dir);
    let data = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    std::fs::write(&path, data).map_err(|e| e.to_string())?;

    Ok(())
}
