use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigEntry {
    pub name: String,
    pub path: PathBuf,
    pub category: String,
    pub description: String,
    pub editor: Option<String>,
    #[serde(skip)]
    pub file_size: Option<u64>,
    #[serde(skip)]
    pub last_modified: Option<String>,
    #[serde(skip)]
    pub exists: bool,
}

pub struct ConfigsModule {
    pub configs: Vec<ConfigEntry>,
    config_file: PathBuf,
}

impl ConfigsModule {
    pub fn new(config_dir: Option<PathBuf>) -> Result<Self> {
        let config_file = config_dir
            .unwrap_or_else(|| {
                dirs::config_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join("launchr")
            })
            .join("configs.json");

        let mut module = Self {
            configs: Vec::new(),
            config_file,
        };

        module.load()?;
        module.refresh()?;

        Ok(module)
    }

    fn load(&mut self) -> Result<()> {
        if !self.config_file.exists() {
            // Create default config entries if file doesn't exist
            self.configs = Self::get_default_configs();
            self.save()?;
            return Ok(());
        }

        let content =
            fs::read_to_string(&self.config_file).context("Failed to read configs file")?;

        self.configs = serde_json::from_str(&content).context("Failed to parse configs file")?;

        Ok(())
    }

    fn save(&self) -> Result<()> {
        if let Some(parent) = self.config_file.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(&self.configs)?;
        fs::write(&self.config_file, content)?;

        Ok(())
    }

    pub fn refresh(&mut self) -> Result<()> {
        for config in &mut self.configs {
            config.exists = config.path.exists();

            if config.exists {
                if let Ok(metadata) = fs::metadata(&config.path) {
                    config.file_size = Some(metadata.len());

                    if let Ok(modified) = metadata.modified() {
                        if let Ok(duration) = modified.elapsed() {
                            config.last_modified = Some(Self::format_time_ago(duration));
                        }
                    }
                }
            } else {
                config.file_size = None;
                config.last_modified = None;
            }
        }

        // Sort by category, then name
        self.configs.sort_by(|a, b| {
            a.category
                .cmp(&b.category)
                .then_with(|| a.name.cmp(&b.name))
        });

        Ok(())
    }

    pub fn add(
        &mut self,
        name: String,
        path: PathBuf,
        category: String,
        description: String,
        editor: Option<String>,
    ) -> Result<()> {
        let config = ConfigEntry {
            name,
            path,
            category,
            description,
            editor,
            file_size: None,
            last_modified: None,
            exists: false,
        };

        self.configs.push(config);
        self.save()?;
        self.refresh()?;

        Ok(())
    }

    pub fn add_from_string(&mut self, input: &str) -> Result<()> {
        // Format: name|path|category|description|editor (editor is optional)
        let parts: Vec<&str> = input.split('|').map(|s| s.trim()).collect();

        if parts.len() < 3 {
            anyhow::bail!("Invalid format. Use: name|path|category|description|editor");
        }

        let name = parts[0].to_string();
        let path = PathBuf::from(parts[1]);
        let category = parts[2].to_string();
        let description = parts.get(3).unwrap_or(&"").to_string();
        let editor = parts
            .get(4)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());

        self.add(name, path, category, description, editor)
    }

    pub fn delete(&mut self, index: usize) -> Result<()> {
        if index < self.configs.len() {
            self.configs.remove(index);
            self.save()?;
        }
        Ok(())
    }

    pub fn open_config(&self, index: usize) -> Result<()> {
        if index >= self.configs.len() {
            anyhow::bail!("Invalid config index");
        }

        let config = &self.configs[index];

        if !config.exists {
            anyhow::bail!("Config file does not exist: {:?}", config.path);
        }

        let editor = config
            .editor
            .clone()
            .or_else(|| std::env::var("EDITOR").ok())
            .unwrap_or_else(|| "nano".to_string());

        #[cfg(unix)]
        {
            Command::new(&editor).arg(&config.path).spawn()?;
        }

        #[cfg(windows)]
        {
            Command::new("cmd")
                .args(&["/C", "start", "", &editor, &config.path.to_string_lossy()])
                .spawn()?;
        }

        Ok(())
    }

    pub fn backup_config(&self, index: usize) -> Result<String> {
        if index >= self.configs.len() {
            anyhow::bail!("Invalid config index");
        }

        let config = &self.configs[index];

        if !config.exists {
            anyhow::bail!("Config file does not exist: {:?}", config.path);
        }

        let extension = config
            .path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let backup_extension = format!("{}.backup.{}", extension, timestamp);

        let backup_path = config.path.with_extension(backup_extension);

        fs::copy(&config.path, &backup_path)?;

        Ok(backup_path.to_string_lossy().to_string())
    }

    pub fn view_config(&self, index: usize) -> Result<String> {
        if index >= self.configs.len() {
            anyhow::bail!("Invalid config index");
        }

        let config = &self.configs[index];

        if !config.exists {
            return Ok(format!("Config file does not exist: {:?}", config.path));
        }

        let content = fs::read_to_string(&config.path)?;

        // Limit to first 50 lines for preview
        let lines: Vec<&str> = content.lines().take(50).collect();
        let preview = lines.join("\n");

        if content.lines().count() > 50 {
            Ok(format!(
                "{}\n\n... ({} more lines)",
                preview,
                content.lines().count() - 50
            ))
        } else {
            Ok(preview)
        }
    }

    pub fn search(&self, query: &str) -> Vec<usize> {
        let query_lower = query.to_lowercase();
        self.configs
            .iter()
            .enumerate()
            .filter(|(_, c)| {
                c.name.to_lowercase().contains(&query_lower)
                    || c.path
                        .to_string_lossy()
                        .to_lowercase()
                        .contains(&query_lower)
                    || c.category.to_lowercase().contains(&query_lower)
                    || c.description.to_lowercase().contains(&query_lower)
            })
            .map(|(i, _)| i)
            .collect()
    }

    pub fn copy_to_clipboard(&self, index: usize) -> Result<String> {
        if index >= self.configs.len() {
            anyhow::bail!("Invalid config index");
        }

        let config = &self.configs[index];

        if !config.exists {
            anyhow::bail!("Config file does not exist: {:?}", config.path);
        }

        let content = fs::read_to_string(&config.path)?;

        #[cfg(target_os = "macos")]
        {
            use std::process::Stdio;
            let mut child = Command::new("pbcopy").stdin(Stdio::piped()).spawn()?;

            if let Some(mut stdin) = child.stdin.take() {
                use std::io::Write;
                stdin.write_all(content.as_bytes())?;
            }
            child.wait()?;
        }

        #[cfg(target_os = "linux")]
        {
            use std::process::Stdio;
            let mut child = Command::new("xclip")
                .args(&["-selection", "clipboard"])
                .stdin(Stdio::piped())
                .spawn()?;

            if let Some(mut stdin) = child.stdin.take() {
                use std::io::Write;
                stdin.write_all(content.as_bytes())?;
            }
            child.wait()?;
        }

        #[cfg(target_os = "windows")]
        {
            use std::process::Stdio;
            let mut child = Command::new("clip").stdin(Stdio::piped()).spawn()?;

            if let Some(mut stdin) = child.stdin.take() {
                use std::io::Write;
                stdin.write_all(content.as_bytes())?;
            }
            child.wait()?;
        }

        Ok(content)
    }

    pub fn open_in_file_manager(&self, index: usize) -> Result<()> {
        if index >= self.configs.len() {
            anyhow::bail!("Invalid config index");
        }

        let config = &self.configs[index];
        let _dir = config.path.parent().unwrap_or(Path::new("."));

        #[cfg(target_os = "macos")]
        {
            Command::new("open").arg(_dir).spawn()?;
        }

        #[cfg(target_os = "linux")]
        {
            Command::new("xdg-open").arg(_dir).spawn()?;
        }

        #[cfg(target_os = "windows")]
        {
            Command::new("explorer").arg(_dir).spawn()?;
        }

        Ok(())
    }

    fn get_default_configs() -> Vec<ConfigEntry> {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));

        vec![
            ConfigEntry {
                name: "Bash Config".to_string(),
                path: home.join(".bashrc"),
                category: "Shell".to_string(),
                description: "Bash shell configuration".to_string(),
                editor: None,
                file_size: None,
                last_modified: None,
                exists: false,
            },
            ConfigEntry {
                name: "Zsh Config".to_string(),
                path: home.join(".zshrc"),
                category: "Shell".to_string(),
                description: "Zsh shell configuration".to_string(),
                editor: None,
                file_size: None,
                last_modified: None,
                exists: false,
            },
            ConfigEntry {
                name: "Git Config".to_string(),
                path: home.join(".gitconfig"),
                category: "Git".to_string(),
                description: "Git global configuration".to_string(),
                editor: None,
                file_size: None,
                last_modified: None,
                exists: false,
            },
            ConfigEntry {
                name: "SSH Config".to_string(),
                path: home.join(".ssh/config"),
                category: "SSH".to_string(),
                description: "SSH client configuration".to_string(),
                editor: None,
                file_size: None,
                last_modified: None,
                exists: false,
            },
            ConfigEntry {
                name: "Vim Config".to_string(),
                path: home.join(".vimrc"),
                category: "Editor".to_string(),
                description: "Vim editor configuration".to_string(),
                editor: None,
                file_size: None,
                last_modified: None,
                exists: false,
            },
        ]
    }

    fn format_time_ago(duration: std::time::Duration) -> String {
        let secs = duration.as_secs();

        if secs < 60 {
            format!("{}s ago", secs)
        } else if secs < 3600 {
            format!("{}m ago", secs / 60)
        } else if secs < 86400 {
            format!("{}h ago", secs / 3600)
        } else {
            format!("{}d ago", secs / 86400)
        }
    }

    pub fn format_file_size(size: u64) -> String {
        const KB: u64 = 1024;
        const MB: u64 = KB * 1024;
        const GB: u64 = MB * 1024;

        if size >= GB {
            format!("{:.2} GB", size as f64 / GB as f64)
        } else if size >= MB {
            format!("{:.2} MB", size as f64 / MB as f64)
        } else if size >= KB {
            format!("{:.2} KB", size as f64 / KB as f64)
        } else {
            format!("{} B", size)
        }
    }
}
