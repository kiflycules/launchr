use anyhow::Result;
use chrono::{DateTime, Local};
use std::collections::VecDeque;

#[derive(Debug, Clone)]
pub struct ClipboardEntry {
    pub content: String,
    pub timestamp: DateTime<Local>,
    pub content_type: String, // text, command, url, etc.
    pub pinned: bool,
}

pub struct ClipboardModule {
    pub entries: VecDeque<ClipboardEntry>,
    pub max_entries: usize,
    last_clipboard_content: String,
}

impl ClipboardModule {
    pub fn new() -> Self {
        Self {
            entries: VecDeque::new(),
            max_entries: 100,
            last_clipboard_content: String::new(),
        }
    }

    pub fn refresh(&mut self) -> Result<()> {
        // Try to get clipboard content
        #[cfg(feature = "clipboard")]
        {
            use clipboard::{ClipboardContext, ClipboardProvider};
            if let Ok(mut ctx) = ClipboardContext::new() {
                if let Ok(content) = ctx.get_contents() {
                    if !content.is_empty() && content != self.last_clipboard_content {
                        self.add_entry(content.clone(), "text");
                        self.last_clipboard_content = content;
                    }
                }
            }
        }

        // Fallback for systems without clipboard support
        #[cfg(not(feature = "clipboard"))]
        {
            // Could integrate with external clipboard managers
            // or use platform-specific commands
        }

        Ok(())
    }

    pub fn add_entry(&mut self, content: String, content_type: &str) {
        // Don't add duplicates of the most recent entry
        if let Some(last) = self.entries.front() {
            if last.content == content && !last.pinned {
                return;
            }
        }

        let entry = ClipboardEntry {
            content,
            timestamp: Local::now(),
            content_type: content_type.to_string(),
            pinned: false,
        };

        self.entries.push_front(entry);

        // Remove old unpinned entries if over limit
        while self.entries.len() > self.max_entries {
            // Find last unpinned entry and remove it
            if let Some(pos) = self.entries.iter().rposition(|e| !e.pinned) {
                self.entries.remove(pos);
            } else {
                break; // All entries are pinned
            }
        }
    }

    pub fn copy_to_clipboard(&self, index: usize) -> Result<()> {
        if index >= self.entries.len() {
            return Ok(());
        }

        let content = &self.entries[index].content;

        #[cfg(target_os = "linux")]
        {
            use std::io::Write;
            use std::process::Command;

            // Try xclip first
            if let Ok(mut child) = Command::new("xclip")
                .arg("-selection")
                .arg("clipboard")
                .stdin(std::process::Stdio::piped())
                .spawn()
            {
                if let Some(stdin) = child.stdin.as_mut() {
                    let _ = stdin.write_all(content.as_bytes());
                }
                let _ = child.wait();
                return Ok(());
            }

            // Fallback to xsel
            if let Ok(mut child) = Command::new("xsel")
                .arg("--clipboard")
                .arg("--input")
                .stdin(std::process::Stdio::piped())
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
            use std::process::Command;

            if let Ok(mut child) = Command::new("pbcopy")
                .stdin(std::process::Stdio::piped())
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
            use std::process::Command;
            // Use PowerShell to set clipboard
            let _ = Command::new("powershell")
                .args(&[
                    "-command",
                    &format!("Set-Clipboard -Value '{}'", content.replace('\'', "''")),
                ])
                .output();
        }

        Ok(())
    }

    #[allow(dead_code)]
    pub fn toggle_pin(&mut self, index: usize) {
        if let Some(entry) = self.entries.get_mut(index) {
            entry.pinned = !entry.pinned;
        }

        // Sort to keep pinned items at top
        let mut pinned: Vec<_> = self.entries.drain(..).collect();
        pinned.sort_by(|a, b| match (a.pinned, b.pinned) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => b.timestamp.cmp(&a.timestamp),
        });
        self.entries = pinned.into();
    }

    #[allow(dead_code)]
    pub fn clear_unpinned(&mut self) {
        self.entries.retain(|e| e.pinned);
    }

    #[allow(dead_code)]
    pub fn search(&self, query: &str) -> Vec<usize> {
        let query_lower = query.to_lowercase();
        self.entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.content.to_lowercase().contains(&query_lower))
            .map(|(i, _)| i)
            .collect()
    }
}
