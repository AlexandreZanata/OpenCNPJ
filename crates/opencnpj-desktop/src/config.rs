//! Persistent preferences. Only the server URL and, optionally, the last username are written;
//! the login key and the session token never touch the disk.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const APP_DIR: &str = "opencnpj-admin";
pub const FILE_NAME: &str = "config.json";
pub const DEFAULT_URL: &str = "http://127.0.0.1:8081";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub server_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self { server_url: DEFAULT_URL.to_string(), username: None }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Windows,
    MacOs,
    Unix,
}

impl Platform {
    pub fn current() -> Self {
        if cfg!(windows) {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::MacOs
        } else {
            Self::Unix
        }
    }
}

/// Configuration directory for `platform`, reading environment variables through `env`:
/// `%APPDATA%\opencnpj-admin` on Windows, `~/Library/Application Support/opencnpj-admin` on
/// macOS, and `$XDG_CONFIG_HOME/opencnpj-admin` (absolute paths only) or
/// `~/.config/opencnpj-admin` elsewhere.
pub fn config_dir_for(platform: Platform, env: impl Fn(&str) -> Option<String>) -> Option<PathBuf> {
    let var = |k: &str| env(k).filter(|v| !v.trim().is_empty()).map(PathBuf::from);
    let base = match platform {
        Platform::Windows => var("APPDATA").or_else(|| var("USERPROFILE").map(|h| h.join("AppData").join("Roaming")))?,
        Platform::MacOs => var("HOME")?.join("Library").join("Application Support"),
        Platform::Unix => var("XDG_CONFIG_HOME").filter(|p| p.is_absolute()).or_else(|| var("HOME").map(|h| h.join(".config")))?,
    };
    Some(base.join(APP_DIR))
}

/// Configuration file path on this machine.
pub fn config_path() -> Option<PathBuf> {
    config_dir_for(Platform::current(), |k| std::env::var(k).ok()).map(|d| d.join(FILE_NAME))
}

impl Config {
    /// Reads `path`; a missing or unreadable file yields the defaults.
    pub fn load_from(path: &Path) -> Self {
        let mut c: Config = std::fs::read_to_string(path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        if c.server_url.trim().is_empty() {
            c.server_url = DEFAULT_URL.into();
        }
        c
    }

    pub fn save_to(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(path, text + "\n")
    }

    pub fn load() -> Self {
        config_path().map(|p| Self::load_from(&p)).unwrap_or_default()
    }

    pub fn save(&self) -> std::io::Result<()> {
        let path = config_path().ok_or_else(|| std::io::Error::other("no configuration directory (HOME/APPDATA not set)"))?;
        self.save_to(&path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let m: HashMap<String, String> = pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        move |k| m.get(k).cloned()
    }

    #[test]
    fn unix_prefers_absolute_xdg_config_home() {
        let d = config_dir_for(Platform::Unix, env(&[("XDG_CONFIG_HOME", "/xdg"), ("HOME", "/home/ana")]));
        assert_eq!(d, Some(PathBuf::from("/xdg/opencnpj-admin")));
        let d = config_dir_for(Platform::Unix, env(&[("XDG_CONFIG_HOME", "relative"), ("HOME", "/home/ana")]));
        assert_eq!(d, Some(PathBuf::from("/home/ana/.config/opencnpj-admin")));
        let d = config_dir_for(Platform::Unix, env(&[("XDG_CONFIG_HOME", ""), ("HOME", "/home/ana")]));
        assert_eq!(d, Some(PathBuf::from("/home/ana/.config/opencnpj-admin")));
        assert_eq!(config_dir_for(Platform::Unix, env(&[])), None);
    }

    #[test]
    fn windows_uses_appdata() {
        let d = config_dir_for(Platform::Windows, env(&[("APPDATA", r"C:\Users\ana\AppData\Roaming")])).unwrap();
        assert!(d.ends_with(APP_DIR));
        assert!(d.starts_with(r"C:\Users\ana\AppData\Roaming"));
        let d = config_dir_for(Platform::Windows, env(&[("USERPROFILE", r"C:\Users\ana")])).unwrap();
        assert!(d.ends_with(Path::new("AppData").join("Roaming").join(APP_DIR)));
        assert_eq!(config_dir_for(Platform::Windows, env(&[("HOME", "/home/ana")])), None);
    }

    #[test]
    fn macos_uses_application_support() {
        let d = config_dir_for(Platform::MacOs, env(&[("HOME", "/Users/ana")]));
        assert_eq!(d, Some(PathBuf::from("/Users/ana/Library/Application Support/opencnpj-admin")));
    }

    #[test]
    fn round_trip_and_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join(FILE_NAME);
        assert_eq!(Config::load_from(&path), Config::default());
        let c = Config { server_url: "https://admin.example.com".into(), username: Some("ana".into()) };
        c.save_to(&path).unwrap();
        assert_eq!(Config::load_from(&path), c);
        std::fs::write(&path, "not json").unwrap();
        assert_eq!(Config::load_from(&path), Config::default());
        std::fs::write(&path, r#"{"server_url":""}"#).unwrap();
        assert_eq!(Config::load_from(&path).server_url, DEFAULT_URL);
    }

    #[test]
    fn only_url_and_username_are_written() {
        let v = serde_json::to_value(Config { server_url: "http://x".into(), username: None }).unwrap();
        assert_eq!(v.as_object().unwrap().keys().collect::<Vec<_>>(), vec!["server_url"]);
        let v = serde_json::to_value(Config { server_url: "http://x".into(), username: Some("ana".into()) }).unwrap();
        let mut keys: Vec<_> = v.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        assert_eq!(keys, vec!["server_url", "username"]);
    }
}
