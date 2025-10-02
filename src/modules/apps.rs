use anyhow::Result;
use std::collections::HashSet;
use std::env;
use std::path::PathBuf;
use std::process::Stdio;
use sysinfo::{ProcessesToUpdate, RefreshKind, System};
use tokio::process::Command;

#[derive(Debug, Clone)]
pub struct RunningProcess {
    pub pid: u32,
    pub name: String,
    pub cpu_usage: f32,
    pub memory_usage: f64,
}

pub struct AppsModule {
    pub available_apps: Vec<String>,
    pub running_processes: Vec<RunningProcess>,
    system: System,
}

impl AppsModule {
    pub fn new() -> Self {
        Self {
            available_apps: Vec::new(),
            running_processes: Vec::new(),
            system: System::new_with_specifics(RefreshKind::everything()),
        }
    }

    pub async fn scan_path_executables(&mut self) -> Result<()> {
        let mut executables = HashSet::new();
        if let Ok(path_var) = env::var("PATH") {
            let paths: Vec<PathBuf> = env::split_paths(&path_var).collect();
            for path_dir in paths {
                if let Ok(entries) = std::fs::read_dir(&path_dir) {
                    for entry in entries.flatten() {
                        if let Ok(file_type) = entry.file_type()
                            && (file_type.is_file() || file_type.is_symlink())
                            && let Some(name) = entry.file_name().to_str()
                        {
                            // On Windows, filter to common executable extensions to avoid DLLs and assets
                            #[cfg(windows)]
                            {
                                let lower = name.to_ascii_lowercase();
                                let is_exe = lower.ends_with(".exe")
                                    || lower.ends_with(".bat")
                                    || lower.ends_with(".cmd")
                                    || lower.ends_with(".ps1");
                                if !is_exe {
                                    continue;
                                }
                            }

                            if which::which(name).is_ok() {
                                executables.insert(name.to_string());
                            }
                        }
                    }
                }
            }
        }
        self.available_apps = executables.into_iter().collect();
        self.available_apps.sort();
        Ok(())
    }

    pub async fn refresh_running_processes(&mut self) -> Result<()> {
        self.system.refresh_processes(ProcessesToUpdate::All, false);
        self.running_processes = self
            .system
            .processes()
            .iter()
            .map(|(pid, process)| {
                let memory_mb = process.memory() as f64 / 1024.0 / 1024.0;
                RunningProcess {
                    pid: pid.as_u32(),
                    name: process.name().to_string_lossy().to_string(),
                    cpu_usage: process.cpu_usage(),
                    memory_usage: memory_mb,
                }
            })
            .collect();
        self.running_processes
            .sort_by(|a, b| b.cpu_usage.partial_cmp(&a.cpu_usage).unwrap());
        Ok(())
    }

    pub async fn launch_app(&mut self, app_name: &str) -> Result<()> {
        Command::new(app_name)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        self.refresh_running_processes().await?;
        Ok(())
    }

    pub fn stop_process(&mut self, pid: u32) -> Result<()> {
        #[cfg(unix)]
        {
            use std::process::Command as StdCommand;
            let output = StdCommand::new("kill")
                .arg("-15")
                .arg(pid.to_string())
                .output()?;

            if !output.status.success() {
                // Try SIGKILL if SIGTERM fails
                let _ = StdCommand::new("kill")
                    .arg("-9")
                    .arg(pid.to_string())
                    .output();
            }
        }

        #[cfg(windows)]
        {
            use std::process::Command as StdCommand;
            let output = StdCommand::new("taskkill")
                .args(&["/PID", &pid.to_string(), "/F"])
                .output()?;

            if !output.status.success() {
                anyhow::bail!(
                    "Failed to terminate process {}: {}",
                    pid,
                    String::from_utf8_lossy(&output.stderr)
                );
            }
        }

        Ok(())
    }

    /// Snapshot basic CPU metrics for header: (cores, avg_usage)
    pub fn snapshot_cpu_metrics(&self) -> (usize, f32) {
        let cores = self.system.cpus().len();
        if cores == 0 {
            return (0, 0.0);
        }
        let avg = self
            .system
            .cpus()
            .iter()
            .map(|c| c.cpu_usage())
            .sum::<f32>()
            / cores as f32;
        (cores, avg)
    }
}
