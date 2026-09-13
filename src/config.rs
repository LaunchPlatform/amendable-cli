use std::env;
use std::fs;
use std::path::PathBuf;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use serde::Deserialize;
use serde::Serialize;

use crate::error::Error;

pub const DEFAULT_API_URL: &str = "https://api.amendable.io";
pub const DEFAULT_APP_URL: &str = "https://amendable.io";
pub const STAGING_API_URL: &str = "https://stage.api.amendable.io";
pub const STAGING_APP_URL: &str = "https://stage.amendable.io";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct FileConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    api_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    app_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    username: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub api_url: String,
    pub app_url: String,
    pub token: Option<String>,
    pub username: Option<String>,
}

impl Config {
    pub fn load() -> Result<Self, Error> {
        let mut data = FileConfig::default();
        let path = config_path();
        if path.is_file() {
            let text = fs::read_to_string(&path)?;
            data = toml::from_str(&text)?;
        }
        let api_url = env::var("AMENDABLE_API_URL")
            .ok()
            .filter(|v| !v.is_empty())
            .or(data.api_url)
            .unwrap_or_else(|| DEFAULT_API_URL.to_string());
        let app_url = env::var("AMENDABLE_APP_URL")
            .ok()
            .filter(|v| !v.is_empty())
            .or(data.app_url)
            .unwrap_or_else(|| DEFAULT_APP_URL.to_string());
        let token = env::var("AMENDABLE_TOKEN")
            .ok()
            .filter(|v| !v.is_empty())
            .or(data.token);
        let username = env::var("AMENDABLE_USERNAME")
            .ok()
            .filter(|v| !v.is_empty())
            .or(data.username);
        Ok(Self {
            api_url: trim_slash(&api_url),
            app_url: trim_slash(&app_url),
            token,
            username,
        })
    }

    pub fn save(&self) -> Result<(), Error> {
        let path = config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let file = FileConfig {
            api_url: Some(self.api_url.clone()),
            app_url: Some(self.app_url.clone()),
            token: self.token.clone(),
            username: self.username.clone(),
        };
        let text = toml::to_string(&file)?;
        fs::write(&path, text)?;
        #[cfg(unix)]
        {
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
        }
        Ok(())
    }

    pub fn require_token(&self) -> Result<&str, Error> {
        self.token.as_deref().ok_or(Error::NotLoggedIn)
    }

    pub fn use_staging(&mut self) {
        self.api_url = STAGING_API_URL.to_string();
        self.app_url = STAGING_APP_URL.to_string();
    }
}

pub fn config_dir() -> PathBuf {
    if let Ok(xdg) = env::var("XDG_CONFIG_HOME") {
        if !xdg.is_empty() {
            return PathBuf::from(xdg).join("amendable");
        }
    }
    home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".config")
        .join("amendable")
}

pub fn config_path() -> PathBuf {
    if let Ok(override_path) = env::var("AMENDABLE_CONFIG") {
        if !override_path.is_empty() {
            return PathBuf::from(override_path);
        }
    }
    config_dir().join("config.toml")
}

pub fn apply_staging_env() {
    env::set_var("AMENDABLE_API_URL", STAGING_API_URL);
    env::set_var("AMENDABLE_APP_URL", STAGING_APP_URL);
}

pub fn git_clone_url(app_url: &str, username: &str, name: &str) -> String {
    format!(
        "{}/r/{}/{}.git",
        app_url.trim_end_matches('/'),
        username,
        name
    )
}

fn trim_slash(value: &str) -> String {
    value.trim_end_matches('/').to_string()
}

fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn unique_config() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("config.toml");
        (dir, path)
    }

    #[test]
    fn default_hosts_are_production() {
        let _guard = ENV_LOCK.lock().unwrap();
        let (_dir, path) = unique_config();
        env::set_var("AMENDABLE_CONFIG", &path);
        env::remove_var("AMENDABLE_API_URL");
        env::remove_var("AMENDABLE_APP_URL");
        env::remove_var("AMENDABLE_TOKEN");
        env::remove_var("AMENDABLE_USERNAME");
        let cfg = Config::load().unwrap();
        assert_ne!(cfg.api_url, STAGING_API_URL);
        assert_ne!(cfg.app_url, STAGING_APP_URL);
        assert_eq!(cfg.api_url, DEFAULT_API_URL);
        assert_eq!(cfg.app_url, DEFAULT_APP_URL);
    }

    #[test]
    fn use_staging_sets_hosts() {
        let _guard = ENV_LOCK.lock().unwrap();
        let (_dir, path) = unique_config();
        env::set_var("AMENDABLE_CONFIG", &path);
        env::remove_var("AMENDABLE_API_URL");
        env::remove_var("AMENDABLE_APP_URL");
        env::remove_var("AMENDABLE_TOKEN");
        env::remove_var("AMENDABLE_USERNAME");
        let mut cfg = Config::load().unwrap();
        cfg.use_staging();
        assert_eq!(cfg.api_url, STAGING_API_URL);
        assert_eq!(cfg.app_url, STAGING_APP_URL);
        cfg.save().unwrap();
        let loaded = Config::load().unwrap();
        assert_eq!(loaded.api_url, STAGING_API_URL);
        assert_eq!(loaded.app_url, STAGING_APP_URL);
    }

    #[test]
    fn apply_staging_env_overrides_file() {
        let _guard = ENV_LOCK.lock().unwrap();
        let (_dir, path) = unique_config();
        fs::write(
            &path,
            "api_url = \"https://api.amendable.io\"\napp_url = \"https://amendable.io\"\n",
        )
        .unwrap();
        env::set_var("AMENDABLE_CONFIG", &path);
        env::remove_var("AMENDABLE_TOKEN");
        env::remove_var("AMENDABLE_USERNAME");
        apply_staging_env();
        let cfg = Config::load().unwrap();
        assert_eq!(cfg.api_url, STAGING_API_URL);
        assert_eq!(cfg.app_url, STAGING_APP_URL);
        env::remove_var("AMENDABLE_API_URL");
        env::remove_var("AMENDABLE_APP_URL");
    }
}
