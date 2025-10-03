use crate::config::{ConfigFile, UserConfigEntry};
use anyhow::{Context, Result};
use ratatui::style::{Color, Style};
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
    config_file: PathBuf, // points to ~/.config/launchr/config.toml
    pub preview_content: Option<String>,
    pub preview_scroll: usize,
    pub preview_mode: bool,
    pub highlighted_content: Option<Vec<(Style, String)>>,
}

impl ConfigsModule {
    pub fn new(config_file: PathBuf) -> Result<Self> {
        let mut module = Self {
            configs: Vec::new(),
            config_file,
            preview_content: None,
            preview_scroll: 0,
            preview_mode: false,
            highlighted_content: None,
        };

        module.load()?;
        module.refresh()?;

        Ok(module)
    }

    fn load(&mut self) -> Result<()> {
        if !self.config_file.exists() {
            // No config file yet; start with defaults in-memory
            self.configs = Self::get_default_configs();
            return Ok(());
        }

        let content = fs::read_to_string(&self.config_file)
            .with_context(|| format!("Failed to read {:?}", self.config_file))?;

        let mut file: ConfigFile =
            toml::from_str(&content).context("Failed to parse config.toml")?;

        // If configs missing, seed defaults but don't write yet
        if file.configs.is_empty() {
            self.configs = Self::get_default_configs();
        } else {
            self.configs = file
                .configs
                .drain(..)
                .map(|u| ConfigEntry {
                    name: u.name,
                    path: PathBuf::from(u.path),
                    category: u.category,
                    description: u.description,
                    editor: u.editor,
                    file_size: None,
                    last_modified: None,
                    exists: false,
                })
                .collect();
        }

        Ok(())
    }

    fn save(&self) -> Result<()> {
        if !self.config_file.exists() {
            // If the main config doesn't exist, create a minimal one
            if let Some(parent) = self.config_file.parent() {
                fs::create_dir_all(parent)?;
            }
            let empty = ConfigFile::default();
            let s = toml::to_string_pretty(&empty)?;
            fs::write(&self.config_file, s)?;
        }

        // Read full file, update configs field, then write back
        let content = fs::read_to_string(&self.config_file)
            .with_context(|| format!("Failed to read {:?}", self.config_file))?;
        let mut file: ConfigFile =
            toml::from_str(&content).context("Failed to parse config.toml")?;

        file.configs = self
            .configs
            .iter()
            .map(|c| UserConfigEntry {
                name: c.name.clone(),
                path: c.path.display().to_string(),
                category: c.category.clone(),
                description: c.description.clone(),
                editor: c.editor.clone(),
            })
            .collect();

        let out = toml::to_string_pretty(&file)?;
        fs::write(&self.config_file, out)?;
        Ok(())
    }

    pub fn refresh(&mut self) -> Result<()> {
        for config in &mut self.configs {
            config.exists = config.path.exists();

            if config.exists {
                if let Ok(metadata) = fs::metadata(&config.path) {
                    config.file_size = Some(metadata.len());

                    if let Ok(modified) = metadata.modified()
                        && let Ok(duration) = modified.elapsed()
                    {
                        config.last_modified = Some(Self::format_time_ago(duration));
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

        let backup_path = if extension.is_empty() {
            // No extension, just add backup suffix
            config.path.with_extension(format!("backup.{}", timestamp))
        } else {
            // Has extension, replace it with backup version
            config
                .path
                .with_extension(format!("{}.backup.{}", extension, timestamp))
        };

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

        // Check if the path is a directory
        if config.path.is_dir() {
            return self.view_directory_config(&config.path);
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

    fn view_directory_config(&self, dir_path: &Path) -> Result<String> {
        let mut result = String::new();
        result.push_str(&format!("Directory: {}\n", dir_path.display()));
        result.push_str(&format!("{}\n", "=".repeat(50)));

        // Get directory contents
        let mut entries = Vec::new();
        if let Ok(read_dir) = fs::read_dir(dir_path) {
            for entry in read_dir {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    let name = path.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown");
                    
                    let is_dir = path.is_dir();
                    let size = if is_dir {
                        "DIR".to_string()
                    } else {
                        fs::metadata(&path)
                            .map(|m| Self::format_file_size(m.len()))
                            .unwrap_or_else(|_| "?".to_string())
                    };

                    entries.push((name.to_string(), is_dir, size, path));
                }
            }
        }

        // Sort entries: directories first, then files, both alphabetically
        entries.sort_by(|a, b| {
            match (a.1, b.1) {
                (true, false) => std::cmp::Ordering::Less,  // dirs first
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.0.cmp(&b.0),  // then alphabetically
            }
        });

        // Prioritize common config files
        let common_config_files = [
            "config", "config.json", "config.toml", "config.yaml", "config.yml",
            "init.lua", "init.vim", "init.lua", "settings.json", "keybindings.json",
            "config.rasi", "style.css", "config.ini", "hyprland.conf", "waybar.css",
            "config.lua", "init.lua", "main.lua", "options.lua", "keymaps.lua",
            "plugins.lua", "colorscheme.lua", "lsp.lua", "treesitter.lua"
        ];

        // Reorder to put common config files at the top
        let mut prioritized_entries = Vec::new();
        let mut remaining_entries = entries;

        // First, add common config files
        for common_file in &common_config_files {
            if let Some(pos) = remaining_entries.iter().position(|(name, _, _, _)| name == common_file) {
                prioritized_entries.push(remaining_entries.remove(pos));
            }
        }

        // Then add remaining entries
        prioritized_entries.extend(remaining_entries);

        let total_items = prioritized_entries.len();

        // Display the directory tree
        for (name, is_dir, size, _) in prioritized_entries {
            let icon = if is_dir { "📁" } else { "📄" };
            let type_indicator = if is_dir { "DIR" } else { "FILE" };
            
            // Highlight common config files
            let is_common = common_config_files.contains(&name.as_str());
            let prefix = if is_common { "★ " } else { "  " };
            
            result.push_str(&format!("{}{} {} {} ({})\n", prefix, icon, name, size, type_indicator));
        }

        if total_items == 0 {
            result.push_str("(empty directory)\n");
        } else {
            result.push_str(&format!("\nTotal: {} items\n", total_items));
            result.push_str("★ = Common config file\n");
            result.push_str("Use 'e' to open in editor (will open directory in file manager)\n");
        }

        Ok(result)
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

        // Handle directories differently
        if config.path.is_dir() {
            return self.copy_directory_info_to_clipboard(&config.path);
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
                .args(["-selection", "clipboard"])
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

    fn copy_directory_info_to_clipboard(&self, dir_path: &Path) -> Result<String> {
        let mut content = String::new();
        content.push_str(&format!("Directory: {}\n", dir_path.display()));
        content.push_str(&format!("{}\n", "=".repeat(50)));

        // Get directory contents
        let mut entries = Vec::new();
        if let Ok(read_dir) = fs::read_dir(dir_path) {
            for entry in read_dir {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    let name = path.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown");
                    
                    let is_dir = path.is_dir();
                    let size = if is_dir {
                        "DIR".to_string()
                    } else {
                        fs::metadata(&path)
                            .map(|m| Self::format_file_size(m.len()))
                            .unwrap_or_else(|_| "?".to_string())
                    };

                    entries.push((name.to_string(), is_dir, size));
                }
            }
        }

        // Sort entries: directories first, then files, both alphabetically
        entries.sort_by(|a, b| {
            match (a.1, b.1) {
                (true, false) => std::cmp::Ordering::Less,  // dirs first
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.0.cmp(&b.0),  // then alphabetically
            }
        });

        let total_items = entries.len();

        // Display the directory listing
        for (name, is_dir, size) in entries {
            let type_indicator = if is_dir { "DIR" } else { "FILE" };
            content.push_str(&format!("{} {} ({})\n", name, size, type_indicator));
        }

        if total_items == 0 {
            content.push_str("(empty directory)\n");
        } else {
            content.push_str(&format!("\nTotal: {} items\n", total_items));
        }

        // Copy to clipboard
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
                .args(["-selection", "clipboard"])
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

    pub fn open_in_editor(&self, index: usize) -> Result<()> {
        if index >= self.configs.len() {
            anyhow::bail!("Invalid config index");
        }

        let config = &self.configs[index];

        if !config.exists {
            anyhow::bail!("Config file does not exist: {:?}", config.path);
        }

        // Handle directories differently
        if config.path.is_dir() {
            return self.open_directory_in_file_manager(&config.path);
        }

        let editor = self.get_editor(config);
        let path_str = config.path.to_str().unwrap_or("");

        #[cfg(windows)]
        {
            // On Windows, always use 'start' to open in a new window
            // This prevents blocking and UI conflicts
            Command::new("cmd")
                .args(&["/C", "start", &editor, path_str])
                .spawn()
                .map_err(|e| anyhow::anyhow!(
                    "Failed to open editor '{}': {}.\n\nTips:\n- Set EDITOR environment variable\n- Configure editor in config\n- Ensure editor is in PATH",
                    editor, e
                ))?;
        }

        #[cfg(unix)]
        {
            // On Unix, try to detect and use the appropriate terminal emulator
            let term_editors = ["code", "subl", "atom", "gedit", "kate"];

            if term_editors.iter().any(|&e| editor.contains(e)) {
                // GUI editors can be spawned directly
                Command::new(&editor)
                    .arg(&config.path)
                    .spawn()
                    .map_err(|e| anyhow::anyhow!("Failed to open editor '{}': {}", editor, e))?;
            } else {
                // Terminal editors need to be opened in a new terminal window
                let terminals = [
                    ("x-terminal-emulator", vec!["-e", &editor, path_str]),
                    ("gnome-terminal", vec!["--", &editor, path_str]),
                    ("konsole", vec!["-e", &editor, path_str]),
                    ("xterm", vec!["-e", &editor, path_str]),
                    ("alacritty", vec!["-e", &editor, path_str]),
                    ("kitty", vec!["-e", &editor, path_str]),
                ];

                let mut opened = false;
                for (term, args) in &terminals {
                    if Command::new(term).args(args).spawn().is_ok() {
                        opened = true;
                        break;
                    }
                }

                if !opened {
                    anyhow::bail!(
                        "No suitable terminal emulator found. Please install one of: gnome-terminal, konsole, xterm, alacritty, or kitty"
                    );
                }
            }
        }

        #[cfg(target_os = "macos")]
        {
            // macOS: use 'open -a' for GUI apps or spawn terminal for terminal editors
            if editor.contains("vim") || editor.contains("nano") || editor.contains("emacs") {
                // Open terminal editors in a new Terminal.app window
                let script = format!(
                    "tell application \"Terminal\" to do script \"{} {}\"",
                    editor, path_str
                );
                Command::new("osascript")
                    .arg("-e")
                    .arg(&script)
                    .spawn()
                    .map_err(|e| anyhow::anyhow!("Failed to open editor in terminal: {}", e))?;
            } else {
                // GUI editors can use 'open'
                Command::new("open")
                    .arg("-a")
                    .arg(&editor)
                    .arg(&config.path)
                    .spawn()
                    .map_err(|e| anyhow::anyhow!("Failed to open editor '{}': {}", editor, e))?;
            }
        }

        Ok(())
    }

    fn open_directory_in_file_manager(&self, dir_path: &Path) -> Result<()> {
        let path_str = dir_path.to_str().unwrap_or("");

        #[cfg(target_os = "linux")]
        {
            // Try different file managers on Linux
            let file_managers = [
                ("nautilus", vec![path_str]),
                ("dolphin", vec![path_str]),
                ("thunar", vec![path_str]),
                ("pcmanfm", vec![path_str]),
                ("nemo", vec![path_str]),
                ("caja", vec![path_str]),
                ("konqueror", vec![path_str]),
                ("krusader", vec![path_str]),
                ("ranger", vec![path_str]),
                ("mc", vec![path_str]),
            ];

            let mut opened = false;
            for (manager, args) in &file_managers {
                if Command::new(manager).args(args).spawn().is_ok() {
                    opened = true;
                    break;
                }
            }

            if !opened {
                // Fallback: try to open in terminal with ls
                let command = format!("ls -la '{}'; read -p 'Press Enter to close...'", path_str);
                let terminals = [
                    ("x-terminal-emulator", vec!["-e", "bash", "-c", &command]),
                    ("gnome-terminal", vec!["--", "bash", "-c", &command]),
                    ("konsole", vec!["-e", "bash", "-c", &command]),
                    ("xterm", vec!["-e", "bash", "-c", &command]),
                ];

                for (term, args) in &terminals {
                    if Command::new(term).args(args).spawn().is_ok() {
                        opened = true;
                        break;
                    }
                }
            }

            if !opened {
                anyhow::bail!("No suitable file manager or terminal found to open directory");
            }
        }

        #[cfg(target_os = "macos")]
        {
            // macOS: use 'open' to open directory in Finder
            Command::new("open")
                .arg(path_str)
                .spawn()
                .map_err(|e| anyhow::anyhow!("Failed to open directory in Finder: {}", e))?;
        }

        #[cfg(target_os = "windows")]
        {
            // Windows: use 'explorer' to open directory
            Command::new("explorer")
                .arg(path_str)
                .spawn()
                .map_err(|e| anyhow::anyhow!("Failed to open directory in Explorer: {}", e))?;
        }

        Ok(())
    }

    fn get_editor(&self, config: &ConfigEntry) -> String {
        // Use the config's specific editor if set, otherwise fall back to environment or defaults
        if let Some(ref custom_editor) = config.editor {
            custom_editor.clone()
        } else if let Ok(env_editor) = std::env::var("EDITOR") {
            env_editor
        } else {
            // Default editors by platform
            #[cfg(target_os = "windows")]
            {
                "notepad.exe".to_string()
            }
            #[cfg(not(target_os = "windows"))]
            {
                "nano".to_string()
            }
        }
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

    pub fn set_preview_content(&mut self, content: String) {
        self.preview_content = Some(content);
        self.preview_scroll = 0; // Reset scroll when new content is set
        self.preview_mode = true; // Enter preview mode when content is set
    }

    pub fn exit_preview_mode(&mut self) {
        self.preview_mode = false;
        self.preview_content = None;
        self.preview_scroll = 0;
    }

    pub fn scroll_preview_up(&mut self) {
        if self.preview_scroll > 0 {
            self.preview_scroll = self.preview_scroll.saturating_sub(1);
        }
    }

    pub fn scroll_preview_down(&mut self) {
        if let Some(ref content) = self.preview_content {
            let lines: Vec<&str> = content.lines().collect();
            let max_scroll = lines.len().saturating_sub(1);
            if self.preview_scroll < max_scroll {
                self.preview_scroll = (self.preview_scroll + 1).min(max_scroll);
            }
        }
    }

    pub fn highlight_content(&mut self, content: &str, file_path: &Path) {
        let file_ext = file_path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");

        let file_name = file_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");

        let highlighted = match self.detect_file_type(file_ext, file_name) {
            FileType::Json => self.highlight_json(content),
            FileType::Yaml => self.highlight_yaml(content),
            FileType::Toml => self.highlight_toml(content),
            FileType::Ini => self.highlight_ini(content),
            FileType::Conf => self.highlight_conf(content),
            FileType::Env => self.highlight_env(content),
            FileType::Ssh => self.highlight_ssh(content),
            FileType::Git => self.highlight_git(content),
            FileType::Docker => self.highlight_docker(content),
            FileType::Nginx => self.highlight_nginx(content),
            FileType::Apache => self.highlight_apache(content),
            FileType::Systemd => self.highlight_systemd(content),
            FileType::Bash => self.highlight_bash(content),
            FileType::Unknown => self.highlight_generic(content),
        };

        self.highlighted_content = Some(highlighted);
    }

    fn detect_file_type(&self, ext: &str, name: &str) -> FileType {
        match (ext.to_lowercase().as_str(), name.to_lowercase().as_str()) {
            ("json", _) => FileType::Json,
            ("yaml", _) | ("yml", _) => FileType::Yaml,
            ("toml", _) => FileType::Toml,
            ("ini", _) => FileType::Ini,
            ("conf", _) => FileType::Conf,
            ("env", _) => FileType::Env,
            ("sh", _) | ("bash", _) | ("zsh", _) => FileType::Bash,
            (_, name) if name.contains(".env") => FileType::Env,
            (_, name)
                if name.contains("bashrc")
                    || name.contains("bash_profile")
                    || name.contains("zshrc")
                    || name.contains("profile")
                    || name.contains("aliases")
                    || name.contains("functions") =>
            {
                FileType::Bash
            }
            (_, name) if name.contains("ssh") || name.contains("config") => FileType::Ssh,
            (_, name) if name.contains("git") => FileType::Git,
            (_, name) if name.contains("docker") => FileType::Docker,
            (_, name) if name.contains("nginx") => FileType::Nginx,
            (_, name) if name.contains("apache") => FileType::Apache,
            (_, name) if name.contains("systemd") || name.contains("service") => FileType::Systemd,
            _ => FileType::Unknown,
        }
    }

    fn highlight_json(&self, content: &str) -> Vec<(Style, String)> {
        let mut result = Vec::new();
        let chars = content.chars().peekable();
        let mut current_token = String::new();
        let mut in_string = false;
        let mut escape_next = false;

        for ch in chars {
            if escape_next {
                current_token.push(ch);
                escape_next = false;
                continue;
            }

            match ch {
                '"' if !in_string => {
                    if !current_token.is_empty() {
                        result.push((Style::default(), current_token.clone()));
                        current_token.clear();
                    }
                    in_string = true;
                    current_token.push(ch);
                }
                '"' if in_string => {
                    current_token.push(ch);
                    result.push((Style::default().fg(Color::Green), current_token.clone()));
                    current_token.clear();
                    in_string = false;
                }
                '\\' if in_string => {
                    current_token.push(ch);
                    escape_next = true;
                }
                ':' if !in_string => {
                    if !current_token.is_empty() {
                        result.push((Style::default(), current_token.clone()));
                        current_token.clear();
                    }
                    result.push((Style::default().fg(Color::Cyan), ch.to_string()));
                }
                '{' | '}' | '[' | ']' if !in_string => {
                    if !current_token.is_empty() {
                        result.push((Style::default(), current_token.clone()));
                        current_token.clear();
                    }
                    result.push((Style::default().fg(Color::Yellow), ch.to_string()));
                }
                ',' if !in_string => {
                    if !current_token.is_empty() {
                        result.push((Style::default(), current_token.clone()));
                        current_token.clear();
                    }
                    result.push((Style::default().fg(Color::Magenta), ch.to_string()));
                }
                _ => {
                    current_token.push(ch);
                }
            }
        }

        if !current_token.is_empty() {
            result.push((Style::default(), current_token));
        }

        result
    }

    fn highlight_yaml(&self, content: &str) -> Vec<(Style, String)> {
        let mut result = Vec::new();

        for line in content.lines() {
            let mut chars = line.chars().peekable();
            let mut current_token = String::new();
            let mut in_string = false;
            let mut escape_next = false;
            let mut indent_level = 0;

            // Count indentation
            while let Some(&ch) = chars.peek() {
                if ch == ' ' || ch == '\t' {
                    indent_level += 1;
                    chars.next();
                } else {
                    break;
                }
            }

            // Add indentation with dim style
            if indent_level > 0 {
                result.push((
                    Style::default().fg(Color::DarkGray),
                    " ".repeat(indent_level),
                ));
            }

            while let Some(ch) = chars.next() {
                if escape_next {
                    current_token.push(ch);
                    escape_next = false;
                    continue;
                }

                match ch {
                    '"' | '\'' if !in_string => {
                        if !current_token.is_empty() {
                            result.push((Style::default(), current_token.clone()));
                            current_token.clear();
                        }
                        in_string = true;
                        current_token.push(ch);
                    }
                    '"' | '\'' if in_string => {
                        current_token.push(ch);
                        result.push((Style::default().fg(Color::Green), current_token.clone()));
                        current_token.clear();
                        in_string = false;
                    }
                    '\\' if in_string => {
                        current_token.push(ch);
                        escape_next = true;
                    }
                    ':' if !in_string && chars.peek().is_none_or(|&c| c.is_whitespace()) => {
                        if !current_token.is_empty() {
                            result.push((Style::default().fg(Color::Cyan), current_token.clone()));
                            current_token.clear();
                        }
                        result.push((Style::default().fg(Color::Yellow), ch.to_string()));
                    }
                    '#' if !in_string => {
                        if !current_token.is_empty() {
                            result.push((Style::default(), current_token.clone()));
                            current_token.clear();
                        }
                        let comment = ch.to_string() + &chars.collect::<String>();
                        result.push((Style::default().fg(Color::DarkGray), comment));
                        break;
                    }
                    _ => {
                        current_token.push(ch);
                    }
                }
            }

            if !current_token.is_empty() {
                result.push((Style::default(), current_token));
            }
            result.push((Style::default(), "\n".to_string()));
        }

        result
    }

    fn highlight_toml(&self, content: &str) -> Vec<(Style, String)> {
        let mut result = Vec::new();

        for line in content.lines() {
            let mut chars = line.chars().peekable();
            let mut current_token = String::new();
            let mut in_string = false;
            let mut escape_next = false;

            while let Some(ch) = chars.next() {
                if escape_next {
                    current_token.push(ch);
                    escape_next = false;
                    continue;
                }

                match ch {
                    '"' | '\'' if !in_string => {
                        if !current_token.is_empty() {
                            result.push((Style::default(), current_token.clone()));
                            current_token.clear();
                        }
                        in_string = true;
                        current_token.push(ch);
                    }
                    '"' | '\'' if in_string => {
                        current_token.push(ch);
                        result.push((Style::default().fg(Color::Green), current_token.clone()));
                        current_token.clear();
                        in_string = false;
                    }
                    '\\' if in_string => {
                        current_token.push(ch);
                        escape_next = true;
                    }
                    '[' if !in_string => {
                        if !current_token.is_empty() {
                            result.push((Style::default(), current_token.clone()));
                            current_token.clear();
                        }
                        result.push((Style::default().fg(Color::Yellow), ch.to_string()));
                    }
                    ']' if !in_string => {
                        if !current_token.is_empty() {
                            result.push((Style::default(), current_token.clone()));
                            current_token.clear();
                        }
                        result.push((Style::default().fg(Color::Yellow), ch.to_string()));
                    }
                    '=' if !in_string => {
                        if !current_token.is_empty() {
                            result.push((Style::default().fg(Color::Cyan), current_token.clone()));
                            current_token.clear();
                        }
                        result.push((Style::default().fg(Color::Magenta), ch.to_string()));
                    }
                    '#' if !in_string => {
                        if !current_token.is_empty() {
                            result.push((Style::default(), current_token.clone()));
                            current_token.clear();
                        }
                        let comment = ch.to_string() + &chars.collect::<String>();
                        result.push((Style::default().fg(Color::DarkGray), comment));
                        break;
                    }
                    _ => {
                        current_token.push(ch);
                    }
                }
            }

            if !current_token.is_empty() {
                result.push((Style::default(), current_token));
            }
            result.push((Style::default(), "\n".to_string()));
        }

        result
    }

    fn highlight_ini(&self, content: &str) -> Vec<(Style, String)> {
        let mut result = Vec::new();

        for line in content.lines() {
            let trimmed = line.trim();

            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                result.push((Style::default().fg(Color::Yellow), line.to_string()));
            } else if trimmed.starts_with('#') || trimmed.starts_with(';') {
                result.push((Style::default().fg(Color::DarkGray), line.to_string()));
            } else if trimmed.contains('=') {
                let parts: Vec<&str> = trimmed.splitn(2, '=').collect();
                if parts.len() == 2 {
                    result.push((Style::default().fg(Color::Cyan), parts[0].to_string()));
                    result.push((Style::default().fg(Color::Magenta), "=".to_string()));
                    result.push((Style::default().fg(Color::Green), parts[1].to_string()));
                } else {
                    result.push((Style::default(), line.to_string()));
                }
            } else {
                result.push((Style::default(), line.to_string()));
            }
            result.push((Style::default(), "\n".to_string()));
        }

        result
    }

    fn highlight_conf(&self, content: &str) -> Vec<(Style, String)> {
        let mut result = Vec::new();

        for line in content.lines() {
            let trimmed = line.trim();

            if trimmed.starts_with('#') {
                result.push((Style::default().fg(Color::DarkGray), line.to_string()));
            } else if trimmed.contains(' ') && !trimmed.is_empty() {
                let parts: Vec<&str> = trimmed.splitn(2, ' ').collect();
                if parts.len() == 2 {
                    result.push((Style::default().fg(Color::Cyan), parts[0].to_string()));
                    result.push((Style::default(), " ".to_string()));
                    result.push((Style::default().fg(Color::Green), parts[1].to_string()));
                } else {
                    result.push((Style::default(), line.to_string()));
                }
            } else {
                result.push((Style::default(), line.to_string()));
            }
            result.push((Style::default(), "\n".to_string()));
        }

        result
    }

    fn highlight_env(&self, content: &str) -> Vec<(Style, String)> {
        let mut result = Vec::new();

        for line in content.lines() {
            let trimmed = line.trim();

            if trimmed.starts_with('#') {
                result.push((Style::default().fg(Color::DarkGray), line.to_string()));
            } else if trimmed.contains('=') {
                let parts: Vec<&str> = trimmed.splitn(2, '=').collect();
                if parts.len() == 2 {
                    result.push((Style::default().fg(Color::Cyan), parts[0].to_string()));
                    result.push((Style::default().fg(Color::Magenta), "=".to_string()));
                    result.push((Style::default().fg(Color::Green), parts[1].to_string()));
                } else {
                    result.push((Style::default(), line.to_string()));
                }
            } else {
                result.push((Style::default(), line.to_string()));
            }
            result.push((Style::default(), "\n".to_string()));
        }

        result
    }

    fn highlight_ssh(&self, content: &str) -> Vec<(Style, String)> {
        let mut result = Vec::new();

        for line in content.lines() {
            let trimmed = line.trim();

            if trimmed.starts_with('#') {
                result.push((Style::default().fg(Color::DarkGray), line.to_string()));
            } else if trimmed.contains(' ') && !trimmed.is_empty() {
                let parts: Vec<&str> = trimmed.splitn(2, ' ').collect();
                if parts.len() == 2 {
                    result.push((Style::default().fg(Color::Cyan), parts[0].to_string()));
                    result.push((Style::default(), " ".to_string()));
                    result.push((Style::default().fg(Color::Green), parts[1].to_string()));
                } else {
                    result.push((Style::default(), line.to_string()));
                }
            } else {
                result.push((Style::default(), line.to_string()));
            }
            result.push((Style::default(), "\n".to_string()));
        }

        result
    }

    fn highlight_git(&self, content: &str) -> Vec<(Style, String)> {
        let mut result = Vec::new();

        for line in content.lines() {
            let trimmed = line.trim();

            if trimmed.starts_with('#') {
                result.push((Style::default().fg(Color::DarkGray), line.to_string()));
            } else if trimmed.starts_with('[') && trimmed.ends_with(']') {
                result.push((Style::default().fg(Color::Yellow), line.to_string()));
            } else if trimmed.contains('=') {
                let parts: Vec<&str> = trimmed.splitn(2, '=').collect();
                if parts.len() == 2 {
                    result.push((Style::default().fg(Color::Cyan), parts[0].to_string()));
                    result.push((Style::default().fg(Color::Magenta), "=".to_string()));
                    result.push((Style::default().fg(Color::Green), parts[1].to_string()));
                } else {
                    result.push((Style::default(), line.to_string()));
                }
            } else {
                result.push((Style::default(), line.to_string()));
            }
            result.push((Style::default(), "\n".to_string()));
        }

        result
    }

    fn highlight_docker(&self, content: &str) -> Vec<(Style, String)> {
        let mut result = Vec::new();

        for line in content.lines() {
            let trimmed = line.trim();

            if trimmed.starts_with('#') {
                result.push((Style::default().fg(Color::DarkGray), line.to_string()));
            } else if trimmed.contains(' ') && !trimmed.is_empty() {
                let parts: Vec<&str> = trimmed.splitn(2, ' ').collect();
                if parts.len() == 2 {
                    result.push((Style::default().fg(Color::Cyan), parts[0].to_string()));
                    result.push((Style::default(), " ".to_string()));
                    result.push((Style::default().fg(Color::Green), parts[1].to_string()));
                } else {
                    result.push((Style::default(), line.to_string()));
                }
            } else {
                result.push((Style::default(), line.to_string()));
            }
            result.push((Style::default(), "\n".to_string()));
        }

        result
    }

    fn highlight_nginx(&self, content: &str) -> Vec<(Style, String)> {
        let mut result = Vec::new();

        for line in content.lines() {
            let trimmed = line.trim();

            if trimmed.starts_with('#') {
                result.push((Style::default().fg(Color::DarkGray), line.to_string()));
            } else if trimmed.ends_with('{') || trimmed.ends_with('}') {
                result.push((Style::default().fg(Color::Yellow), line.to_string()));
            } else if trimmed.contains(' ') && !trimmed.is_empty() {
                let parts: Vec<&str> = trimmed.splitn(2, ' ').collect();
                if parts.len() == 2 {
                    result.push((Style::default().fg(Color::Cyan), parts[0].to_string()));
                    result.push((Style::default(), " ".to_string()));
                    result.push((Style::default().fg(Color::Green), parts[1].to_string()));
                } else {
                    result.push((Style::default(), line.to_string()));
                }
            } else {
                result.push((Style::default(), line.to_string()));
            }
            result.push((Style::default(), "\n".to_string()));
        }

        result
    }

    fn highlight_apache(&self, content: &str) -> Vec<(Style, String)> {
        let mut result = Vec::new();

        for line in content.lines() {
            let trimmed = line.trim();

            if trimmed.starts_with('#') {
                result.push((Style::default().fg(Color::DarkGray), line.to_string()));
            } else if trimmed.starts_with('<') && trimmed.ends_with('>') {
                result.push((Style::default().fg(Color::Yellow), line.to_string()));
            } else if trimmed.contains(' ') && !trimmed.is_empty() {
                let parts: Vec<&str> = trimmed.splitn(2, ' ').collect();
                if parts.len() == 2 {
                    result.push((Style::default().fg(Color::Cyan), parts[0].to_string()));
                    result.push((Style::default(), " ".to_string()));
                    result.push((Style::default().fg(Color::Green), parts[1].to_string()));
                } else {
                    result.push((Style::default(), line.to_string()));
                }
            } else {
                result.push((Style::default(), line.to_string()));
            }
            result.push((Style::default(), "\n".to_string()));
        }

        result
    }

    fn highlight_systemd(&self, content: &str) -> Vec<(Style, String)> {
        let mut result = Vec::new();

        for line in content.lines() {
            let trimmed = line.trim();

            if trimmed.starts_with('#') {
                result.push((Style::default().fg(Color::DarkGray), line.to_string()));
            } else if trimmed.starts_with('[') && trimmed.ends_with(']') {
                result.push((Style::default().fg(Color::Yellow), line.to_string()));
            } else if trimmed.contains('=') {
                let parts: Vec<&str> = trimmed.splitn(2, '=').collect();
                if parts.len() == 2 {
                    result.push((Style::default().fg(Color::Cyan), parts[0].to_string()));
                    result.push((Style::default().fg(Color::Magenta), "=".to_string()));
                    result.push((Style::default().fg(Color::Green), parts[1].to_string()));
                } else {
                    result.push((Style::default(), line.to_string()));
                }
            } else {
                result.push((Style::default(), line.to_string()));
            }
            result.push((Style::default(), "\n".to_string()));
        }

        result
    }

    fn highlight_bash(&self, content: &str) -> Vec<(Style, String)> {
        let mut result = Vec::new();

        for line in content.lines() {
            let trimmed = line.trim();

            if trimmed.starts_with('#') {
                result.push((Style::default().fg(Color::DarkGray), line.to_string()));
            } else if trimmed.starts_with("export ") || trimmed.starts_with("alias ") {
                // Highlight export and alias statements
                let parts: Vec<&str> = trimmed.splitn(2, ' ').collect();
                if parts.len() == 2 {
                    result.push((Style::default().fg(Color::Cyan), parts[0].to_string()));
                    result.push((Style::default(), " ".to_string()));
                    result.push((Style::default().fg(Color::Green), parts[1].to_string()));
                } else {
                    result.push((Style::default(), line.to_string()));
                }
            } else if trimmed.contains("=") && !trimmed.contains("==") && !trimmed.contains("!=") {
                // Highlight variable assignments
                let parts: Vec<&str> = trimmed.splitn(2, '=').collect();
                if parts.len() == 2 {
                    result.push((Style::default().fg(Color::Cyan), parts[0].to_string()));
                    result.push((Style::default().fg(Color::Magenta), "=".to_string()));
                    result.push((Style::default().fg(Color::Green), parts[1].to_string()));
                } else {
                    result.push((Style::default(), line.to_string()));
                }
            } else if trimmed.starts_with("function ") || trimmed.contains("()") {
                // Highlight function definitions
                result.push((Style::default().fg(Color::Yellow), line.to_string()));
            } else if trimmed.starts_with("if ")
                || trimmed.starts_with("for ")
                || trimmed.starts_with("while ")
                || trimmed.starts_with("case ")
                || trimmed.starts_with("elif ")
                || trimmed.starts_with("else")
                || trimmed.starts_with("fi")
                || trimmed.starts_with("done")
                || trimmed.starts_with("esac")
                || trimmed.starts_with("then")
            {
                // Highlight control structures
                result.push((Style::default().fg(Color::Magenta), line.to_string()));
            } else if trimmed.contains("$") {
                // Highlight lines with variables
                let mut chars = line.chars().peekable();
                let mut current_token = String::new();

                while let Some(ch) = chars.next() {
                    if ch == '$' {
                        if !current_token.is_empty() {
                            result.push((Style::default(), current_token.clone()));
                            current_token.clear();
                        }
                        current_token.push(ch);

                        // Collect the variable name
                        while let Some(&next_ch) = chars.peek() {
                            if next_ch.is_alphanumeric()
                                || next_ch == '_'
                                || next_ch == '{'
                                || next_ch == '}'
                            {
                                current_token.push(chars.next().unwrap());
                            } else {
                                break;
                            }
                        }

                        result.push((Style::default().fg(Color::Yellow), current_token.clone()));
                        current_token.clear();
                    } else {
                        current_token.push(ch);
                    }
                }

                if !current_token.is_empty() {
                    result.push((Style::default(), current_token));
                }
            } else {
                result.push((Style::default(), line.to_string()));
            }
            result.push((Style::default(), "\n".to_string()));
        }

        result
    }

    fn highlight_generic(&self, content: &str) -> Vec<(Style, String)> {
        let mut result = Vec::new();

        for line in content.lines() {
            let trimmed = line.trim();

            if trimmed.starts_with('#') {
                result.push((Style::default().fg(Color::DarkGray), line.to_string()));
            } else {
                result.push((Style::default(), line.to_string()));
            }
            result.push((Style::default(), "\n".to_string()));
        }

        result
    }
}

#[derive(Debug, Clone, Copy)]
enum FileType {
    Json,
    Yaml,
    Toml,
    Ini,
    Conf,
    Env,
    Ssh,
    Git,
    Docker,
    Nginx,
    Apache,
    Systemd,
    Bash,
    Unknown,
}
