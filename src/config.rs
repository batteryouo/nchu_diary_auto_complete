use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Field names remain compatible with the Python application's config.json.
/// Deliberately no Debug implementation: credentials must not reach logs.
#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Config {
    pub id: String,
    pub pw: String,
    pub school_value: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        match fs::read_to_string(path) {
            Ok(json) => serde_json::from_str(&json)
                .with_context(|| format!("設定檔格式錯誤：{}", path.display())),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(err) => Err(err).with_context(|| format!("無法讀取設定檔：{}", path.display())),
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        // A sibling temporary file avoids truncating working credentials on serialization/write failure.
        let temporary = path.with_extension("json.tmp");
        fs::write(&temporary, json)
            .with_context(|| format!("無法寫入設定檔：{}", path.display()))?;
        fs::rename(&temporary, path).with_context(|| format!("無法更新設定檔：{}", path.display()))
    }
}

/// cargo run uses the repository config; distributed binaries use their own directory.
pub fn default_path() -> Result<PathBuf> {
    let cwd = std::env::current_dir()?;
    let exe = std::env::current_exe()?;
    if exe.starts_with(cwd.join("target"))
        && cwd.join("Cargo.toml").is_file()
        && cwd.join("src/main.rs").is_file()
    {
        return Ok(cwd.join("config.json"));
    }
    Ok(exe
        .parent()
        .context("找不到執行檔目錄")?
        .join("config.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_dir() -> Result<tempfile::TempDir> {
        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test-tmp");
        fs::create_dir_all(&base)?;
        Ok(tempfile::Builder::new()
            .prefix("config-")
            .tempdir_in(base)?)
    }

    #[test]
    fn reads_python_config_and_preserves_unknown_settings() -> Result<()> {
        let dir = test_dir()?;
        let path = dir.path().join("config.json");
        fs::write(
            &path,
            r#"{"id":"test-id","pw":"900928","school_value":"P001","future_setting":15}"#,
        )?;
        let mut config = Config::load(&path)?;
        assert_eq!(config.school_value.as_deref(), Some("P001"));
        config.school_value = Some("P002".into());
        config.save(&path)?;
        // Replacing an existing config must work on Windows as well.
        config.save(&path)?;
        let restored = Config::load(&path)?;
        assert_eq!(restored.id, "test-id");
        assert_eq!(restored.pw, "900928");
        assert_eq!(restored.extra["future_setting"], 15);
        Ok(())
    }

    #[test]
    fn missing_file_is_first_run_but_malformed_file_is_an_error() -> Result<()> {
        let dir = test_dir()?;
        let path = dir.path().join("config.json");
        assert!(Config::load(&path)?.id.is_empty());
        fs::write(&path, "{")?;
        assert!(Config::load(&path).is_err());
        assert_eq!(fs::read_to_string(&path)?, "{");
        Ok(())
    }
}
