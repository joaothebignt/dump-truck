use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone)]
pub struct AppSettings {
    pub max_parallel_dumps: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_dump_dir: Option<String>,
    #[serde(default)]
    pub table_exclude_patterns: Vec<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        AppSettings {
            max_parallel_dumps: 5,
            custom_dump_dir: None,
            table_exclude_patterns: vec![],
        }
    }
}

fn settings_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("munhoz-dumper").join("settings.json"))
}

pub fn load() -> AppSettings {
    let path = match settings_path() {
        Some(p) => p,
        None => return AppSettings::default(),
    };

    let data = match std::fs::read_to_string(&path) {
        Ok(d) => d,
        Err(_) => return AppSettings::default(),
    };

    serde_json::from_str(&data).unwrap_or_default()
}

pub fn save(settings: &AppSettings) -> Result<(), String> {
    let path = settings_path()
        .ok_or_else(|| "Não foi possível resolver o diretório de configuração".to_string())?;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let data = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    std::fs::write(&path, data).map_err(|e| e.to_string())?;

    Ok(())
}
