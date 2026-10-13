use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::theme::{self, fallback_id};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_theme")]
    pub theme: String,
}

fn default_theme() -> String {
    "aurora".into()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: default_theme(),
        }
    }
}

impl Config {
    pub fn path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("disktui")
            .join("config.toml")
    }

    pub fn load(path: &Path, omarchy_installed: bool) -> Self {
        let text = std::fs::read_to_string(path).unwrap_or_default();
        if text.trim().is_empty() {
            return Self {
                theme: fallback_id(omarchy_installed).into(),
            };
        }
        let mut cfg: Self = toml::from_str(&text).unwrap_or_default();
        if !theme::known(&cfg.theme) {
            cfg.theme = fallback_id(omarchy_installed).into();
        }
        cfg
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, toml::to_string_pretty(self).unwrap_or_default())
    }
}

#[cfg(test)]
mod aurora_persistence {
    use super::*;
    #[test]
    fn defaults_and_explicit_choices() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        assert_eq!(Config::default().theme, "aurora");
        std::fs::write(&path, "theme = \"terminal\"\n").unwrap();
        assert_eq!(Config::load(&path, true).theme, "terminal");
    }
}
