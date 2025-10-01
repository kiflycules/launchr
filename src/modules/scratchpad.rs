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
            if let Ok(metadata) = entry.metadata() {
                if metadata.is_file() {
                    if let Some(name) = entry.file_name().to_str() {
                        let path = entry.path();
                        let created_at = metadata
                            .created()
                            .ok()
                            .map(|t| DateTime::<Local>::from(t))
                            .unwrap_or_else(Local::now);
                        
                        let modified_at = metadata
                            .modified()
                            .ok()
                            .map(|t| DateTime::<Local>::from(t))
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
        
        #[cfg(unix)]
        {
            Command::new(&editor)
                .arg(path)
                .spawn()
                .map_err(|e| anyhow::anyhow!("Failed to open editor '{}': {}. Set EDITOR environment variable or configure preferred_editor.", editor, e))?;
        }

        #[cfg(windows)]
        {
            let result = if editor.to_lowercase().contains("notepad") {
                Command::new(&editor)
                    .arg(path)
                    .spawn()
            } else {
                // Try direct spawn first
                let direct = Command::new(&editor)
                    .arg(path)
                    .spawn();
                
                if direct.is_ok() {
                    direct
                } else {
                    // Fallback to cmd start for stubborn editors
                    Command::new("cmd")
                        .args(&["/C", "start", "", &editor, path.to_str().unwrap_or("")])
                        .spawn()
                }
            };
            
            result.map_err(|e| anyhow::anyhow!(
                "Failed to open editor '{}': {}.\n\nTips:\n- Set EDITOR environment variable\n- Configure preferred_editor in config\n- Ensure editor is in PATH",
                editor, e
            ))?;
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
            
            if let Ok(mut child) = Command::new("pbcopy")
                .stdin(Stdio::piped())
                .spawn()
            {
                if let Some(stdin) = child.stdin.as_mut() {
                    let _ = stdin.write_all(content.as_bytes());
                }
                let _ = child.wait();
            }
        }

        #[cfg(target_os = "windows")]
        {
            let _ = Command::new("powershell")
                .args(&["-command", &format!("Set-Clipboard -Value @'\n{}\n'@", content)])
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
                if let Ok(content) = fs::read_to_string(&note.path) {
                    if content.to_lowercase().contains(&query_lower) {
                        return true;
                    }
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
            return "nano".to_string();
        }

        #[cfg(target_os = "macos")]
        {
            let editors = ["code", "subl", "nano", "vim", "vi", "open -e"];
            for editor in &editors {
                if Command::new("which").arg(editor.split_whitespace().next().unwrap()).output().is_ok() {
                    return editor.to_string();
                }
            }
            return "open -e".to_string();
        }

        #[cfg(target_os = "windows")]
        {
            // Check for common Windows editors
            let editors = [
                "code.cmd",           // VS Code
                "code",               // VS Code alternate
                "subl.exe",           // Sublime Text
                "sublime_text.exe",   // Sublime Text alternate
                "notepad++.exe",      // Notepad++
                "atom.exe",           // Atom
                "vim.exe",            // Vim for Windows
                "gvim.exe",           // GVim
                "notepad.exe",        // Windows Notepad (fallback)
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
            return "notepad.exe".to_string();
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

