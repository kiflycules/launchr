use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct GitRepo {
    pub name: String,
    pub path: PathBuf,
    pub branch: String,
    pub status: String, // clean, modified, ahead, behind
    pub uncommitted_changes: usize,
    pub ahead: usize,
    pub behind: usize,
    pub last_commit: String,
}

pub struct GitModule {
    pub repos: Vec<GitRepo>,
    pub search_paths: Vec<PathBuf>,
}

impl GitModule {
    pub fn new(config_paths: &[String]) -> Self {
        let mut search_paths = vec![];
        
        // Use configured paths if provided
        if !config_paths.is_empty() {
            for path_str in config_paths {
                let path = if path_str == "~" || path_str == "~/" || path_str == "~\\" {
                    dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"))
                } else if path_str.starts_with("~/") {
                    if let Some(home) = dirs::home_dir() { home.join(&path_str[2..]) } else { PathBuf::from(path_str) }
                } else {
                    PathBuf::from(path_str)
                };
                search_paths.push(path);
            }
        } else {
            // Use default paths if none configured
            if let Some(home) = dirs::home_dir() {
                search_paths.push(home.join("Projects"));
                search_paths.push(home.join("projects"));
                search_paths.push(home.join("Documents").join("GitHub"));
                search_paths.push(home.join("repos"));
                search_paths.push(home.join("src"));
                search_paths.push(home.join("code"));
            }
        }
        
        Self {
            repos: Vec::new(),
            search_paths,
        }
    }

    pub fn scan_repositories(&mut self) -> Result<()> {
        self.repos.clear();
        
        // Clone search_paths to avoid borrow checker issues
        let search_paths = self.search_paths.clone();
        for search_path in &search_paths {
            if search_path.exists() {
                self.scan_directory(search_path, 3)?; // Max depth of 3
            }
        }
        
        // Sort by name
        self.repos.sort_by(|a, b| a.name.cmp(&b.name));
        
        Ok(())
    }

    fn scan_directory(&mut self, dir: &Path, max_depth: usize) -> Result<()> {
        if max_depth == 0 { return Ok(()); }
        
        if dir.join(".git").exists() {
            if let Some(repo) = self.analyze_repo(dir) {
                self.repos.push(repo);
            }
            return Ok(()); // Don't scan subdirs of git repos
        }
        
        // Scan subdirectories
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                if let Ok(file_type) = entry.file_type() {
                    let mut next_path: Option<PathBuf> = None;
                    if file_type.is_dir() {
                        next_path = Some(entry.path());
                    } else if file_type.is_symlink() {
                        // Follow directory symlinks
                        if let Ok(target) = fs::read_link(entry.path()) {
                            let resolved = if target.is_absolute() { target } else { dir.join(target) };
                            if resolved.is_dir() { next_path = Some(resolved); }
                        }
                    }
                    if let Some(path) = next_path {
                        // Skip hidden directories (except .git which we check above)
                        if let Some(name) = path.file_name() {
                            if !name.to_string_lossy().starts_with('.') {
                                let _ = self.scan_directory(&path, max_depth - 1);
                            }
                        }
                    }
                }
            }
        }
        
        Ok(())
    }

    fn analyze_repo(&self, path: &Path) -> Option<GitRepo> {
        let name = path.file_name()?.to_string_lossy().to_string();
        
        // Get current branch
        let branch = Command::new("git")
            .args(&["-C", path.to_str()?, "branch", "--show-current"])
            .output()
            .ok()
            .and_then(|o| {
                if o.status.success() {
                    Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| "detached".to_string());
        
        // Get status
        let status_output = Command::new("git")
            .args(&["-C", path.to_str()?, "status", "--porcelain"])
            .output()
            .ok()?;
        
        let uncommitted_changes = if status_output.status.success() {
            String::from_utf8_lossy(&status_output.stdout)
                .lines()
                .filter(|l| !l.is_empty())
                .count()
        } else {
            0
        };
        
        // Get ahead/behind info
        let (ahead, behind) = if !branch.is_empty() && branch != "detached" {
            let upstream_output = Command::new("git")
                .args(&["-C", path.to_str()?, "rev-list", "--count", "--left-right", &format!("origin/{}...HEAD", branch)])
                .output()
                .ok();
            
            if let Some(output) = upstream_output {
                if output.status.success() {
                    let counts = String::from_utf8_lossy(&output.stdout);
                    let parts: Vec<&str> = counts.trim().split('\t').collect();
                    if parts.len() == 2 {
                        (
                            parts[1].parse().unwrap_or(0),
                            parts[0].parse().unwrap_or(0),
                        )
                    } else {
                        (0, 0)
                    }
                } else {
                    (0, 0)
                }
            } else {
                (0, 0)
            }
        } else {
            (0, 0)
        };
        
        let status = if uncommitted_changes > 0 {
            "modified".to_string()
        } else if ahead > 0 {
            "ahead".to_string()
        } else if behind > 0 {
            "behind".to_string()
        } else {
            "clean".to_string()
        };
        
        // Get last commit message
        let last_commit = Command::new("git")
            .args(&["-C", path.to_str()?, "log", "-1", "--pretty=%s"])
            .output()
            .ok()
            .and_then(|o| {
                if o.status.success() {
                    Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| "No commits".to_string());
        
        Some(GitRepo {
            name,
            path: path.to_path_buf(),
            branch,
            status,
            uncommitted_changes,
            ahead,
            behind,
            last_commit,
        })
    }

    pub fn refresh(&mut self) -> Result<()> {
        // Re-analyze existing repos without full scan
        // Collect paths first to avoid borrow checker issues
        let paths: Vec<PathBuf> = self.repos.iter().map(|r| r.path.clone()).collect();
        
        // Analyze all repos first
        let updates: Vec<Option<GitRepo>> = paths.iter()
            .map(|path| self.analyze_repo(path))
            .collect();
        
        // Then update the repos
        for (i, updated) in updates.into_iter().enumerate() {
            if let Some(updated) = updated {
                if let Some(repo) = self.repos.get_mut(i) {
                    repo.branch = updated.branch;
                    repo.status = updated.status;
                    repo.uncommitted_changes = updated.uncommitted_changes;
                    repo.ahead = updated.ahead;
                    repo.behind = updated.behind;
                    repo.last_commit = updated.last_commit;
                }
            }
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub fn open_in_editor(&self, index: usize) -> Result<()> {
        if index >= self.repos.len() { return Ok(()); }
        let repo = &self.repos[index];
        
        // Try common editors
        let editors = vec!["code", "subl", "atom", "nvim", "vim", "nano", "emacs"];
        
        for editor in editors {
            if Command::new(editor)
                .arg(repo.path.to_str().unwrap_or(""))
                .spawn()
                .is_ok()
            {
                return Ok(());
            }
        }
        
        // Fallback to file manager
        #[cfg(target_os = "linux")]
        { Command::new("xdg-open").arg(&repo.path).spawn()?; }
        #[cfg(target_os = "macos")]
        { Command::new("open").arg(&repo.path).spawn()?; }
        #[cfg(target_os = "windows")]
        { Command::new("explorer").arg(&repo.path).spawn()?; }
        
        Ok(())
    }

    #[allow(dead_code)]
    pub fn pull(&self, index: usize) -> Result<String> {
        if index >= self.repos.len() { return Ok(String::new()); }
        let repo = &self.repos[index];
        
        let output = Command::new("git")
            .args(&["-C", repo.path.to_str().unwrap_or(""), "pull"])
            .output()?;
        
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    #[allow(dead_code)]
    pub fn push(&self, index: usize) -> Result<String> {
        if index >= self.repos.len() { return Ok(String::new()); }
        let repo = &self.repos[index];
        
        let output = Command::new("git")
            .args(&["-C", repo.path.to_str().unwrap_or(""), "push"])
            .output()?;
        
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    #[allow(dead_code)]
    pub fn fetch(&self, index: usize) -> Result<String> {
        if index >= self.repos.len() { return Ok(String::new()); }
        let repo = &self.repos[index];
        
        let output = Command::new("git")
            .args(&["-C", repo.path.to_str().unwrap_or(""), "fetch"])
            .output()?;
        
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    #[allow(dead_code)]
    pub fn commit(&self, index: usize, message: &str) -> Result<String> {
        if index >= self.repos.len() { return Ok(String::new()); }
        let repo = &self.repos[index];
        
        // Stage all changes
        Command::new("git")
            .args(&["-C", repo.path.to_str().unwrap_or(""), "add", "."])
            .output()?;
        
        // Commit
        let output = Command::new("git")
            .args(&["-C", repo.path.to_str().unwrap_or(""), "commit", "-m", message])
            .output()?;
        
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    #[allow(dead_code)]
    pub fn switch_branch(&self, index: usize, branch: &str) -> Result<String> {
        if index >= self.repos.len() { return Ok(String::new()); }
        let repo = &self.repos[index];
        
        let output = Command::new("git")
            .args(&["-C", repo.path.to_str().unwrap_or(""), "checkout", branch])
            .output()?;
        
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Ok(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }

    #[allow(dead_code)]
    pub fn get_branches(&self, index: usize) -> Vec<String> {
        if index >= self.repos.len() { return vec![]; }
        let repo = &self.repos[index];
        
        let output = Command::new("git")
            .args(&["-C", repo.path.to_str().unwrap_or(""), "branch", "-a"])
            .output()
            .ok();
        
        if let Some(output) = output {
            if output.status.success() {
                return String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .map(|l| l.trim().trim_start_matches("* ").to_string())
                    .filter(|l| !l.is_empty())
                    .collect();
            }
        }
        
        vec![]
    }

    #[allow(dead_code)]
    pub fn stash(&self, index: usize) -> Result<String> {
        if index >= self.repos.len() { return Ok(String::new()); }
        let repo = &self.repos[index];
        
        let output = Command::new("git")
            .args(&["-C", repo.path.to_str().unwrap_or(""), "stash"])
            .output()?;
        
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    #[allow(dead_code)]
    pub fn add_search_path(&mut self, path: PathBuf) {
        if !self.search_paths.contains(&path) {
            self.search_paths.push(path);
        }
    }
}

