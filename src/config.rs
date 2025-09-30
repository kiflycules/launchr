use anyhow::{Context, Result};
use dirs::config_dir;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct BookmarkConfig {
    pub name: String,
    pub path: String,
    pub bookmark_type: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct SSHHostConfig {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub user: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ScriptConfig {
    pub name: String,
    pub command: String,
    pub description: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ConfigFile {
    pub bookmarks: Vec<BookmarkConfig>,
    pub ssh_hosts: Vec<SSHHostConfig>,
    pub scripts: Vec<ScriptConfig>,
}

#[derive(Clone, Debug)]
pub struct Config {
    pub path: PathBuf,
    pub bookmarks: Vec<BookmarkConfig>,
    pub ssh_hosts: Vec<SSHHostConfig>,
    pub scripts: Vec<ScriptConfig>,
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = config_path()?;
        if !path.exists() {
            let default = ConfigFile::default();
            let toml = toml::to_string_pretty(&default)?;
            if let Some(parent) = path.parent() { fs::create_dir_all(parent)?; }
            fs::write(&path, toml)?;
        }
        let content = fs::read_to_string(&path).with_context(|| format!("Reading {:?}", &path))?;
        let cfg: ConfigFile = toml::from_str(&content).with_context(|| "Parsing config TOML")?;
        Ok(Self { path, bookmarks: cfg.bookmarks, ssh_hosts: cfg.ssh_hosts, scripts: cfg.scripts })
    }

    fn save(&self) -> Result<()> {
        let cfg = ConfigFile { bookmarks: self.bookmarks.clone(), ssh_hosts: self.ssh_hosts.clone(), scripts: self.scripts.clone() };
        let toml = toml::to_string_pretty(&cfg)?;
        if let Some(parent) = self.path.parent() { fs::create_dir_all(parent)?; }
        fs::write(&self.path, toml)?;
        Ok(())
    }

    pub fn add_bookmark(&mut self, b: BookmarkConfig) -> Result<()> { self.bookmarks.push(b); self.save() }
    pub fn remove_bookmark(&mut self, index: usize) -> Result<()> { if index < self.bookmarks.len() { self.bookmarks.remove(index); } self.save() }
    pub fn add_ssh_host(&mut self, h: SSHHostConfig) -> Result<()> { self.ssh_hosts.push(h); self.save() }
    pub fn remove_ssh_host(&mut self, index: usize) -> Result<()> { if index < self.ssh_hosts.len() { self.ssh_hosts.remove(index); } self.save() }
    pub fn add_script(&mut self, s: ScriptConfig) -> Result<()> { self.scripts.push(s); self.save() }
    pub fn remove_script(&mut self, index: usize) -> Result<()> { if index < self.scripts.len() { self.scripts.remove(index); } self.save() }
}

fn config_path() -> Result<PathBuf> {
    let base = config_dir().context("Could not determine config directory")?;
    Ok(base.join("launchr").join("config.toml"))
}


