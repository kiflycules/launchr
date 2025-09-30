use chrono::{DateTime, Local, TimeZone};
use dirs::home_dir;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct HistoryEntry {
    pub timestamp: Option<DateTime<Local>>,
    pub command: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ShellKind {
    PowerShell,
    Bash,
    Zsh,
    Fish,
    Unknown,
}

pub struct ShellHistoryModule {
    pub detected_shell: ShellKind,
    pub entries: Vec<HistoryEntry>,
}

impl ShellHistoryModule {
    pub fn new() -> Self {
        let detected_shell = detect_shell();
        let entries = normalize_entries(load_history(detected_shell));
        Self { detected_shell, entries }
    }

    pub fn refresh(&mut self) {
        self.entries = normalize_entries(load_history(self.detected_shell));
    }

    pub fn run_entry(&self, index: usize) {
        if index >= self.entries.len() { return; }
        let cmd = self.entries[index].command.clone();
        #[cfg(windows)]
        {
            let _ = std::process::Command::new("powershell")
                .args(["-NoLogo", "-NoProfile", "-Command", &cmd])
                .spawn();
        }
        #[cfg(unix)]
        {
            // Try to detect shell; fallback to sh -c
            let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
            let _ = std::process::Command::new(shell)
                .arg("-c")
                .arg(&cmd)
                .spawn();
        }
    }
}

fn normalize_entries(mut entries: Vec<HistoryEntry>) -> Vec<HistoryEntry> {
    // If any timestamps exist, sort by timestamp desc (newest first),
    // placing entries without timestamps at the end in original relative order.
    let any_ts = entries.iter().any(|e| e.timestamp.is_some());
    if any_ts {
        entries.sort_by(|a, b| match (b.timestamp, a.timestamp) {
            (Some(tb), Some(ta)) => tb.cmp(&ta),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        });
        return entries;
    }
    // No timestamps: assume append-only files and show newest first (reverse order)
    entries.reverse();
    entries
}

pub fn detect_shell() -> ShellKind {
    // Windows: prefer PowerShell when environment indicates it
    if cfg!(target_os = "windows") {
        let parent = std::env::var("PSModulePath").ok();
        let appdata = std::env::var("APPDATA").ok();
        if parent.is_some() || appdata.is_some() { return ShellKind::PowerShell; }
    }
    // Unix: read $SHELL
    if let Ok(s) = std::env::var("SHELL") {
        if s.contains("zsh") { return ShellKind::Zsh; }
        if s.contains("fish") { return ShellKind::Fish; }
        if s.contains("bash") { return ShellKind::Bash; }
    }
    // Fallback
    ShellKind::Unknown
}

fn load_history(shell: ShellKind) -> Vec<HistoryEntry> {
    match shell {
        ShellKind::PowerShell => load_powershell_history(),
        ShellKind::Bash => load_bash_history(),
        ShellKind::Zsh => load_zsh_history(),
        ShellKind::Fish => load_fish_history(),
        ShellKind::Unknown => Vec::new(),
    }
}

fn load_powershell_history() -> Vec<HistoryEntry> {
    // PSReadLine history file
    // %APPDATA%\Microsoft\Windows\PowerShell\PSReadLine\ConsoleHost_history.txt
    if let Ok(appdata) = std::env::var("APPDATA") {
        let path = PathBuf::from(appdata)
            .join("Microsoft")
            .join("Windows")
            .join("PowerShell")
            .join("PSReadLine")
            .join("ConsoleHost_history.txt");
        return read_lines_plain(path);
    }
    Vec::new()
}

fn load_bash_history() -> Vec<HistoryEntry> {
    // ~/.bash_history (plain lines), timestamps in separate file when HISTTIMEFORMAT set—ignored here for simplicity
    let path = home_dir().unwrap_or_default().join(".bash_history");
    read_lines_plain(path)
}

fn load_zsh_history() -> Vec<HistoryEntry> {
    // ~/.zsh_history format: ": 1699999999:0;command here"
    let path = home_dir().unwrap_or_default().join(".zsh_history");
    if let Ok(content) = fs::read_to_string(path) {
        let mut out = Vec::new();
        for line in content.lines() {
            if let Some(rest) = line.strip_prefix(": ") {
                if let Some((ts_part, cmd)) = rest.split_once(";") {
                    let ts_str = ts_part.split(':').next().unwrap_or("");
                    let ts = ts_str.parse::<i64>().ok().and_then(|t| Local.timestamp_opt(t, 0).single());
                    out.push(HistoryEntry { timestamp: ts, command: cmd.to_string() });
                    continue;
                }
            }
            out.push(HistoryEntry { timestamp: None, command: line.to_string() });
        }
        return out;
    }
    Vec::new()
}

fn load_fish_history() -> Vec<HistoryEntry> {
    // ~/.local/share/fish/fish_history YAML-ish format with '- cmd: ...' and 'when: <epoch>'
    let mut path = home_dir().unwrap_or_default();
    if cfg!(target_os = "windows") {
        if let Ok(data) = std::env::var("APPDATA") {
            path = PathBuf::from(data);
        }
        path = path.join("fish");
    } else {
        path = path.join(".local").join("share").join("fish");
    }
    let path = path.join("fish_history");
    if let Ok(content) = fs::read_to_string(path) {
        let mut out = Vec::new();
        let mut cur_cmd: Option<String> = None;
        let mut cur_when: Option<DateTime<Local>> = None;
        for line in content.lines() {
            let line = line.trim_start();
            if let Some(rest) = line.strip_prefix("- cmd: ") {
                if let Some(cmd) = cur_cmd.take() {
                    out.push(HistoryEntry { timestamp: cur_when.take(), command: cmd });
                }
                cur_cmd = Some(rest.to_string());
            } else if let Some(rest) = line.strip_prefix("when: ") {
                if let Ok(epoch) = rest.parse::<i64>() {
                    cur_when = Local.timestamp_opt(epoch, 0).single();
                }
            }
        }
        if let Some(cmd) = cur_cmd.take() {
            out.push(HistoryEntry { timestamp: cur_when.take(), command: cmd });
        }
        return out;
    }
    Vec::new()
}

fn read_lines_plain(path: PathBuf) -> Vec<HistoryEntry> {
    if let Ok(content) = fs::read_to_string(path) {
        return content
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| HistoryEntry { timestamp: None, command: l.to_string() })
            .collect();
    }
    Vec::new()
}


