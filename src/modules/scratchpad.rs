use anyhow::Result;
use chrono::{DateTime, Local};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct ScratchpadNote {
    pub name: String,
    pub path: PathBuf,
    pub created_at: DateTime<Local>,
    pub modified_at: DateTime<Local>,
    pub size_bytes: u64,
}

pub struct ScratchpadModule {
    pub notes: Vec<ScratchpadNote>,
    pub scratchpad_dir: PathBuf,
    pub preferred_editor: Option<String>,
}

impl ScratchpadModule {
    pub fn new(scratchpad_dir: Option<PathBuf>, preferred_editor: Option<String>) -> Result<Self> {
        let scratchpad_dir = scratchpad_dir.unwrap_or_else(|| {
            if let Some(home) = dirs::home_dir() {
                home.join(".launchr").join("scratchpad")
            } else {
                PathBuf::from("./scratchpad")
            }
        });

        // Create directory if it doesn't exist
        if !scratchpad_dir.exists() {
            fs::create_dir_all(&scratchpad_dir)?;
        }

        let mut module = Self {
            notes: Vec::new(),
            scratchpad_dir,
            preferred_editor,
        };

        module.refresh()?;
        Ok(module)
    }

    pub fn refresh(&mut self) -> Result<()> {
        self.notes.clear();

        if !self.scratchpad_dir.exists() {
            return Ok(());
        }

        let entries = fs::read_dir(&self.scratchpad_dir)?;
        for entry in entries.flatten() {
            if let Ok(metadata) = entry.metadata()
                && metadata.is_file()
                    && let Some(name) = entry.file_name().to_str() {
                        let path = entry.path();
                        let created_at = metadata
                            .created()
                            .ok()
                            .map(DateTime::<Local>::from)
                            .unwrap_or_else(Local::now);

                        let modified_at = metadata
                            .modified()
                            .ok()
                            .map(DateTime::<Local>::from)
                            .unwrap_or_else(Local::now);

                        self.notes.push(ScratchpadNote {
                            name: name.to_string(),
                            path,
                            created_at,
                            modified_at,
                            size_bytes: metadata.len(),
                        });
                    }
        }

        // Sort by modified time (newest first)
        self.notes.sort_by(|a, b| b.modified_at.cmp(&a.modified_at));
        Ok(())
    }

    pub fn create_new(&self, name: Option<String>) -> Result<PathBuf> {
        let filename = if let Some(n) = name {
            n
        } else {
            format!("note_{}.txt", Local::now().format("%Y%m%d_%H%M%S"))
        };

        let filepath = self.scratchpad_dir.join(&filename);

        // Create empty file
        fs::write(&filepath, "")?;

        Ok(filepath)
    }

    pub fn open_in_editor(&self, path: &PathBuf) -> Result<()> {
        let editor = self.get_editor();
        let path_str = path.to_str().unwrap_or("");

        #[cfg(windows)]
        {
            // On Windows, always use 'start' to open in a new window
            // This prevents blocking and UI conflicts
            Command::new("cmd")
                .args(&["/C", "start", &editor, path_str])
                .spawn()
                .map_err(|e| anyhow::anyhow!(
                    "Failed to open editor '{}': {}.\n\nTips:\n- Set EDITOR environment variable\n- Configure preferred_editor in config\n- Ensure editor is in PATH",
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
                    .arg(path)
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

                let mut spawned = false;
                for (term, args) in &terminals {
                    if Command::new(term).args(args).spawn().is_ok() {
                        spawned = true;
                        break;
                    }
                }

                if !spawned {
                    return Err(anyhow::anyhow!(
                        "Could not find a terminal emulator to open '{}'. \
                        For terminal editors like vim/nano, launchr needs to spawn a new terminal window. \
                        Install gnome-terminal, konsole, xterm, alacritty, or kitty, \
                        or use a GUI editor (code, subl, gedit, etc.)",
                        editor
                    ));
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
                    .arg(path)
                    .spawn()
                    .map_err(|e| anyhow::anyhow!("Failed to open editor '{}': {}", editor, e))?;
            }
        }

        Ok(())
    }

    pub fn new_and_open(&self, name: Option<String>) -> Result<()> {
        let filepath = self.create_new(name)?;
        self.open_in_editor(&filepath)?;
        Ok(())
    }

    pub fn open_existing(&self, index: usize) -> Result<()> {
        if index >= self.notes.len() {
            return Ok(());
        }

        let note = &self.notes[index];
        self.open_in_editor(&note.path)?;
        Ok(())
    }

    pub fn delete(&mut self, index: usize) -> Result<()> {
        if index >= self.notes.len() {
            return Ok(());
        }

        let note = &self.notes[index];
        fs::remove_file(&note.path)?;
        self.notes.remove(index);
        Ok(())
    }

    pub fn rename(&mut self, index: usize, new_name: &str) -> Result<()> {
        if index >= self.notes.len() {
            return Ok(());
        }

        let note = &self.notes[index];
        let new_path = self.scratchpad_dir.join(new_name);
        fs::rename(&note.path, &new_path)?;
        self.refresh()?;
        Ok(())
    }

    pub fn copy_to_clipboard(&self, index: usize) -> Result<String> {
        if index >= self.notes.len() {
            return Ok(String::new());
        }

        let note = &self.notes[index];
        let content = fs::read_to_string(&note.path)?;

        // Try to set clipboard
        #[cfg(target_os = "linux")]
        {
            use std::io::Write;
            use std::process::Stdio;

            if let Ok(mut child) = Command::new("xclip")
                .arg("-selection")
                .arg("clipboard")
                .stdin(Stdio::piped())
                .spawn()
            {
                if let Some(stdin) = child.stdin.as_mut() {
                    let _ = stdin.write_all(content.as_bytes());
                }
                let _ = child.wait();
            }
        }

        #[cfg(target_os = "macos")]
        {
            use std::io::Write;
            use std::process::Stdio;

            if let Ok(mut child) = Command::new("pbcopy").stdin(Stdio::piped()).spawn() {
                if let Some(stdin) = child.stdin.as_mut() {
                    let _ = stdin.write_all(content.as_bytes());
                }
                let _ = child.wait();
            }
        }

        #[cfg(target_os = "windows")]
        {
            let _ = Command::new("powershell")
                .args(&[
                    "-command",
                    &format!("Set-Clipboard -Value @'\n{}\n'@", content),
                ])
                .output();
        }

        Ok(content)
    }

    pub fn export_to_path(&self, index: usize, destination: &PathBuf) -> Result<()> {
        if index >= self.notes.len() {
            return Ok(());
        }

        let note = &self.notes[index];
        fs::copy(&note.path, destination)?;
        Ok(())
    }

    pub fn search(&self, query: &str) -> Vec<usize> {
        let query_lower = query.to_lowercase();
        self.notes
            .iter()
            .enumerate()
            .filter(|(_, note)| {
                // Search in filename
                if note.name.to_lowercase().contains(&query_lower) {
                    return true;
                }

                // Search in content
                if let Ok(content) = fs::read_to_string(&note.path)
                    && content.to_lowercase().contains(&query_lower) {
                        return true;
                    }

                false
            })
            .map(|(i, _)| i)
            .collect()
    }

    fn get_editor(&self) -> String {
        // Use preferred editor if set
        if let Some(ref editor) = self.preferred_editor {
            return editor.clone();
        }

        // Try to detect editor from environment
        if let Ok(editor) = env::var("EDITOR") {
            return editor;
        }

        if let Ok(editor) = env::var("VISUAL") {
            return editor;
        }

        // Platform-specific defaults
        #[cfg(target_os = "linux")]
        {
            // Try common editors in order of preference
            let editors = ["code", "subl", "gedit", "kate", "nano", "vim", "vi"];
            for editor in &editors {
                if Command::new("which").arg(editor).output().is_ok() {
                    return editor.to_string();
                }
            }
            "nano".to_string()
        }

        #[cfg(target_os = "macos")]
        {
            let editors = ["code", "subl", "nano", "vim", "vi", "open -e"];
            for editor in &editors {
                if Command::new("which")
                    .arg(editor.split_whitespace().next().unwrap())
                    .output()
                    .is_ok()
                {
                    return editor.to_string();
                }
            }
            "open -e".to_string()
        }

        #[cfg(target_os = "windows")]
        {
            // Check for common Windows editors
            // Prefer GUI editors that open in new windows
            let editors = [
                "notepad++.exe",    // Notepad++ (best default for Windows)
                "code.cmd",         // VS Code
                "code",             // VS Code alternate
                "subl.exe",         // Sublime Text
                "sublime_text.exe", // Sublime Text alternate
                "atom.exe",         // Atom
                "gvim.exe",         // GVim (GUI vim)
                "notepad.exe",      // Windows Notepad (fallback)
                "nvim-qt.exe",      // Neovim GUI
                "vim.exe",          // Vim for Windows (terminal)
            ];

            for editor in &editors {
                // Check if editor exists by running 'where' and checking exit code
                if let Ok(output) = Command::new("where").arg(editor).output() {
                    if output.status.success() && !output.stdout.is_empty() {
                        return editor.to_string();
                    }
                }
            }

            // Ultimate fallback - notepad always exists on Windows
            "notepad.exe".to_string()
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            // Fallback for unsupported platforms
            "nano".to_string()
        }
    }

    pub fn get_content_preview(&self, index: usize, max_chars: usize) -> Result<String> {
        if index >= self.notes.len() {
            return Ok(String::new());
        }

        let note = &self.notes[index];
        let content = fs::read_to_string(&note.path)?;

        if content.len() <= max_chars {
            Ok(content)
        } else {
            Ok(format!("{}...", &content[..max_chars]))
        }
    }
}
