use anyhow::Result;
use std::process::Command;

#[derive(Debug, Clone, PartialEq)]
pub enum ServiceState {
    Running,
    Stopped,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    Failed,
    Unknown,
}

impl ServiceState {
    pub fn as_str(&self) -> &str {
        match self {
            ServiceState::Running => "running",
            ServiceState::Stopped => "stopped",
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            ServiceState::Failed => "failed",
            ServiceState::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ServiceInfo {
    pub name: String,
    pub display_name: String,
    pub state: ServiceState,
    pub enabled: bool,
    pub description: String,
    pub pid: Option<u32>,
    pub memory_usage: Option<String>,
    pub uptime: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ServiceManager {
    #[cfg(target_os = "linux")]
    Systemd, // Linux systemd
    #[cfg(target_os = "macos")]
    Launchd, // macOS
    #[cfg(target_os = "windows")]
    WindowsService, // Windows Services
    #[cfg(any(
        target_os = "linux",
        not(any(target_os = "linux", target_os = "macos", target_os = "windows"))
    ))]
    Unknown,
}

pub struct ServicesModule {
    pub services: Vec<ServiceInfo>,
    pub service_manager: ServiceManager,
    pub filter_state: Option<ServiceState>,
    pub show_user_services: bool,
}

impl ServicesModule {
    pub fn new() -> Self {
        let service_manager = Self::detect_service_manager();

        Self {
            services: Vec::new(),
            service_manager,
            filter_state: None,
            show_user_services: false,
        }
    }

    fn detect_service_manager() -> ServiceManager {
        #[cfg(target_os = "linux")]
        {
            // Check if systemctl is available
            if Command::new("systemctl").arg("--version").output().is_ok() {
                ServiceManager::Systemd
            } else {
                ServiceManager::Unknown
            }
        }

        #[cfg(target_os = "macos")]
        {
            ServiceManager::Launchd
        }

        #[cfg(target_os = "windows")]
        {
            ServiceManager::WindowsService
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            ServiceManager::Unknown
        }
    }

    pub fn refresh(&mut self) -> Result<()> {
        self.services.clear();

        match self.service_manager {
            #[cfg(target_os = "linux")]
            ServiceManager::Systemd => self.refresh_systemd()?,
            #[cfg(target_os = "macos")]
            ServiceManager::Launchd => self.refresh_launchd()?,
            #[cfg(target_os = "windows")]
            ServiceManager::WindowsService => self.refresh_windows()?,
            ServiceManager::Unknown => {}
        }

        // Apply filter
        if let Some(ref filter) = self.filter_state {
            self.services.retain(|s| &s.state == filter);
        }

        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn refresh_systemd(&mut self) -> Result<()> {
        let scope = if self.show_user_services {
            "--user"
        } else {
            "--system"
        };

        let output = Command::new("systemctl")
            .args([
                scope,
                "list-units",
                "--type=service",
                "--all",
                "--no-pager",
                "--no-legend",
            ])
            .output()?;

        if !output.status.success() {
            return Ok(());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);

        for line in stdout.lines() {
            if line.trim().is_empty() {
                continue;
            }

            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 4 {
                continue;
            }

            let name = parts[0].trim_end_matches(".service").to_string();
            let load_state = parts[1];
            let active_state = parts[2];
            let sub_state = parts[3];
            let description = parts.get(4..).map(|p| p.join(" ")).unwrap_or_default();

            // Determine state
            let state = match (active_state, sub_state) {
                ("active", "running") => ServiceState::Running,
                ("failed", _) => ServiceState::Failed,
                ("inactive", _) => ServiceState::Stopped,
                _ => ServiceState::Unknown,
            };

            // Skip if loaded is "not-found" (means service doesn't exist)
            if load_state == "not-found" {
                continue;
            }

            // Get additional info
            let (enabled, pid, memory, uptime) = self.get_systemd_service_details(&name, scope);

            self.services.push(ServiceInfo {
                name: name.clone(),
                display_name: name,
                state,
                enabled,
                description,
                pid,
                memory_usage: memory,
                uptime,
            });
        }

        // Sort by name
        self.services.sort_by(|a, b| a.name.cmp(&b.name));

        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn get_systemd_service_details(
        &self,
        service: &str,
        scope: &str,
    ) -> (bool, Option<u32>, Option<String>, Option<String>) {
        let mut enabled = false;
        let mut pid = None;
        let mut memory = None;
        let mut uptime = None;

        // Check if enabled
        if let Ok(output) = Command::new("systemctl")
            .args([scope, "is-enabled", &format!("{}.service", service)])
            .output()
        {
            enabled = String::from_utf8_lossy(&output.stdout).trim() == "enabled";
        }

        // Get status for more details
        if let Ok(output) = Command::new("systemctl")
            .args([
                scope,
                "show",
                &format!("{}.service", service),
                "--property=MainPID,MemoryCurrent,ActiveEnterTimestamp",
            ])
            .output()
        {
            let status = String::from_utf8_lossy(&output.stdout);

            for line in status.lines() {
                if let Some(value) = line.strip_prefix("MainPID=") {
                    if let Ok(p) = value.parse::<u32>()
                        && p > 0
                    {
                        pid = Some(p);
                    }
                } else if let Some(value) = line.strip_prefix("MemoryCurrent=") {
                    if let Ok(bytes) = value.parse::<u64>()
                        && bytes > 0
                    {
                        memory = Some(Self::format_memory(bytes));
                    }
                } else if let Some(value) = line.strip_prefix("ActiveEnterTimestamp=")
                    && !value.is_empty()
                    && value != "0"
                {
                    uptime = Some(value.to_string());
                }
            }
        }

        (enabled, pid, memory, uptime)
    }

    #[cfg(target_os = "macos")]
    fn refresh_launchd(&mut self) -> Result<()> {
        let output = Command::new("launchctl").args(&["list"]).output()?;

        if !output.status.success() {
            return Ok(());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);

        for line in stdout.lines().skip(1) {
            // Skip header
            if line.trim().is_empty() {
                continue;
            }

            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 3 {
                continue;
            }

            let pid_str = parts[0];
            let status_str = parts[1];
            let label = parts[2];

            // Parse PID
            let pid = if pid_str != "-" {
                pid_str.parse::<u32>().ok()
            } else {
                None
            };

            // Determine state based on PID and status
            let state = if pid.is_some() {
                ServiceState::Running
            } else if status_str == "0" {
                ServiceState::Stopped
            } else {
                ServiceState::Failed
            };

            self.services.push(ServiceInfo {
                name: label.to_string(),
                display_name: label.to_string(),
                state,
                enabled: true, // launchd services are typically enabled if listed
                description: String::new(),
                pid,
                memory_usage: None,
                uptime: None,
            });
        }

        self.services.sort_by(|a, b| a.name.cmp(&b.name));

        Ok(())
    }

    #[cfg(target_os = "windows")]
    fn refresh_windows(&mut self) -> Result<()> {
        let output = Command::new("sc").args(&["query"]).output()?;

        if !output.status.success() {
            return Ok(());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut current_service: Option<ServiceInfo> = None;

        for line in stdout.lines() {
            let trimmed = line.trim();

            if trimmed.starts_with("SERVICE_NAME:") {
                // Save previous service
                if let Some(service) = current_service.take() {
                    self.services.push(service);
                }

                let name = trimmed
                    .strip_prefix("SERVICE_NAME:")
                    .unwrap_or("")
                    .trim()
                    .to_string();

                current_service = Some(ServiceInfo {
                    name: name.clone(),
                    display_name: name,
                    state: ServiceState::Unknown,
                    enabled: false,
                    description: String::new(),
                    pid: None,
                    memory_usage: None,
                    uptime: None,
                });
            } else if trimmed.starts_with("DISPLAY_NAME:") {
                if let Some(ref mut service) = current_service {
                    service.display_name = trimmed
                        .strip_prefix("DISPLAY_NAME:")
                        .unwrap_or("")
                        .trim()
                        .to_string();
                }
            } else if trimmed.starts_with("STATE") {
                if let Some(ref mut service) = current_service {
                    if trimmed.contains("RUNNING") {
                        service.state = ServiceState::Running;
                    } else if trimmed.contains("STOPPED") {
                        service.state = ServiceState::Stopped;
                    }
                }
            }
        }

        // Save last service
        if let Some(service) = current_service {
            self.services.push(service);
        }

        self.services.sort_by(|a, b| a.name.cmp(&b.name));

        Ok(())
    }

    pub fn start_service(&self, index: usize) -> Result<String> {
        if index >= self.services.len() {
            return Ok(String::new());
        }

        let service = &self.services[index];

        match self.service_manager {
            #[cfg(target_os = "linux")]
            ServiceManager::Systemd => self.systemd_start(&service.name),
            #[cfg(target_os = "macos")]
            ServiceManager::Launchd => self.launchd_start(&service.name),
            #[cfg(target_os = "windows")]
            ServiceManager::WindowsService => self.windows_start(&service.name),
            ServiceManager::Unknown => Ok("Unknown service manager".to_string()),
        }
    }

    pub fn stop_service(&self, index: usize) -> Result<String> {
        if index >= self.services.len() {
            return Ok(String::new());
        }

        let service = &self.services[index];

        match self.service_manager {
            #[cfg(target_os = "linux")]
            ServiceManager::Systemd => self.systemd_stop(&service.name),
            #[cfg(target_os = "macos")]
            ServiceManager::Launchd => self.launchd_stop(&service.name),
            #[cfg(target_os = "windows")]
            ServiceManager::WindowsService => self.windows_stop(&service.name),
            ServiceManager::Unknown => Ok("Unknown service manager".to_string()),
        }
    }

    pub fn restart_service(&self, index: usize) -> Result<String> {
        if index >= self.services.len() {
            return Ok(String::new());
        }

        let service = &self.services[index];

        match self.service_manager {
            #[cfg(target_os = "linux")]
            ServiceManager::Systemd => self.systemd_restart(&service.name),
            #[cfg(target_os = "macos")]
            ServiceManager::Launchd => self.launchd_restart(&service.name),
            #[cfg(target_os = "windows")]
            ServiceManager::WindowsService => self.windows_restart(&service.name),
            ServiceManager::Unknown => Ok("Unknown service manager".to_string()),
        }
    }

    pub fn enable_service(&self, index: usize) -> Result<String> {
        if index >= self.services.len() {
            return Ok(String::new());
        }

        let service = &self.services[index];

        match self.service_manager {
            #[cfg(target_os = "linux")]
            ServiceManager::Systemd => self.systemd_enable(&service.name),
            #[cfg(target_os = "macos")]
            ServiceManager::Launchd => {
                Ok("Launchd services are managed via plist files".to_string())
            }
            #[cfg(target_os = "windows")]
            ServiceManager::WindowsService => self.windows_enable(&service.name),
            ServiceManager::Unknown => Ok("Unknown service manager".to_string()),
        }
    }

    pub fn disable_service(&self, index: usize) -> Result<String> {
        if index >= self.services.len() {
            return Ok(String::new());
        }

        let service = &self.services[index];

        match self.service_manager {
            #[cfg(target_os = "linux")]
            ServiceManager::Systemd => self.systemd_disable(&service.name),
            #[cfg(target_os = "macos")]
            ServiceManager::Launchd => {
                Ok("Launchd services are managed via plist files".to_string())
            }
            #[cfg(target_os = "windows")]
            ServiceManager::WindowsService => self.windows_disable(&service.name),
            ServiceManager::Unknown => Ok("Unknown service manager".to_string()),
        }
    }

    pub fn get_service_logs(&self, index: usize, _lines: usize) -> Result<String> {
        if index >= self.services.len() {
            return Ok(String::new());
        }

        let _service = &self.services[index];

        match self.service_manager {
            #[cfg(target_os = "linux")]
            ServiceManager::Systemd => self.systemd_logs(&_service.name, _lines),
            #[cfg(target_os = "macos")]
            ServiceManager::Launchd => Ok("Log viewing not implemented for launchd".to_string()),
            #[cfg(target_os = "windows")]
            ServiceManager::WindowsService => {
                Ok("Log viewing not implemented for Windows services".to_string())
            }
            ServiceManager::Unknown => Ok("Unknown service manager".to_string()),
        }
    }

    // Systemd operations
    #[cfg(target_os = "linux")]
    fn systemd_start(&self, service: &str) -> Result<String> {
        let scope = if self.show_user_services {
            "--user"
        } else {
            "--system"
        };
        let output = Command::new("systemctl")
            .args([scope, "start", &format!("{}.service", service)])
            .output()?;

        Ok(String::from_utf8_lossy(&output.stderr).to_string())
    }

    #[cfg(target_os = "linux")]
    fn systemd_stop(&self, service: &str) -> Result<String> {
        let scope = if self.show_user_services {
            "--user"
        } else {
            "--system"
        };
        let output = Command::new("systemctl")
            .args([scope, "stop", &format!("{}.service", service)])
            .output()?;

        Ok(String::from_utf8_lossy(&output.stderr).to_string())
    }

    #[cfg(target_os = "linux")]
    fn systemd_restart(&self, service: &str) -> Result<String> {
        let scope = if self.show_user_services {
            "--user"
        } else {
            "--system"
        };
        let output = Command::new("systemctl")
            .args([scope, "restart", &format!("{}.service", service)])
            .output()?;

        Ok(String::from_utf8_lossy(&output.stderr).to_string())
    }

    #[cfg(target_os = "linux")]
    fn systemd_enable(&self, service: &str) -> Result<String> {
        let scope = if self.show_user_services {
            "--user"
        } else {
            "--system"
        };
        let output = Command::new("systemctl")
            .args([scope, "enable", &format!("{}.service", service)])
            .output()?;

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    #[cfg(target_os = "linux")]
    fn systemd_disable(&self, service: &str) -> Result<String> {
        let scope = if self.show_user_services {
            "--user"
        } else {
            "--system"
        };
        let output = Command::new("systemctl")
            .args([scope, "disable", &format!("{}.service", service)])
            .output()?;

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    #[cfg(target_os = "linux")]
    fn systemd_logs(&self, service: &str, lines: usize) -> Result<String> {
        let scope = if self.show_user_services {
            "--user"
        } else {
            "--system"
        };
        let output = Command::new("journalctl")
            .args([
                scope,
                "-u",
                &format!("{}.service", service),
                "-n",
                &lines.to_string(),
                "--no-pager",
            ])
            .output()?;

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    // Launchd operations
    #[cfg(target_os = "macos")]
    fn launchd_start(&self, service: &str) -> Result<String> {
        let output = Command::new("launchctl")
            .args(&["start", service])
            .output()?;

        Ok(String::from_utf8_lossy(&output.stderr).to_string())
    }

    #[cfg(target_os = "macos")]
    fn launchd_stop(&self, service: &str) -> Result<String> {
        let output = Command::new("launchctl")
            .args(&["stop", service])
            .output()?;

        Ok(String::from_utf8_lossy(&output.stderr).to_string())
    }

    #[cfg(target_os = "macos")]
    fn launchd_restart(&self, service: &str) -> Result<String> {
        let _ = self.launchd_stop(service)?;
        std::thread::sleep(std::time::Duration::from_millis(500));
        self.launchd_start(service)
    }

    // Windows operations
    #[cfg(target_os = "windows")]
    fn windows_start(&self, service: &str) -> Result<String> {
        let output = Command::new("sc").args(&["start", service]).output()?;

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    #[cfg(target_os = "windows")]
    fn windows_stop(&self, service: &str) -> Result<String> {
        let output = Command::new("sc").args(&["stop", service]).output()?;

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    #[cfg(target_os = "windows")]
    fn windows_restart(&self, service: &str) -> Result<String> {
        let _ = self.windows_stop(service)?;
        std::thread::sleep(std::time::Duration::from_secs(2));
        self.windows_start(service)
    }

    #[cfg(target_os = "windows")]
    fn windows_enable(&self, service: &str) -> Result<String> {
        let output = Command::new("sc")
            .args(&["config", service, "start=", "auto"])
            .output()?;

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    #[cfg(target_os = "windows")]
    fn windows_disable(&self, service: &str) -> Result<String> {
        let output = Command::new("sc")
            .args(&["config", service, "start=", "disabled"])
            .output()?;

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    // Utility methods
    pub fn toggle_user_services(&mut self) {
        self.show_user_services = !self.show_user_services;
    }

    pub fn search(&self, query: &str) -> Vec<usize> {
        let query_lower = query.to_lowercase();
        self.services
            .iter()
            .enumerate()
            .filter(|(_, s)| {
                s.name.to_lowercase().contains(&query_lower)
                    || s.display_name.to_lowercase().contains(&query_lower)
                    || s.description.to_lowercase().contains(&query_lower)
            })
            .map(|(i, _)| i)
            .collect()
    }

    #[cfg(target_os = "linux")]
    fn format_memory(bytes: u64) -> String {
        const KB: u64 = 1024;
        const MB: u64 = KB * 1024;
        const GB: u64 = MB * 1024;

        if bytes >= GB {
            format!("{:.2} GB", bytes as f64 / GB as f64)
        } else if bytes >= MB {
            format!("{:.2} MB", bytes as f64 / MB as f64)
        } else if bytes >= KB {
            format!("{:.2} KB", bytes as f64 / KB as f64)
        } else {
            format!("{} B", bytes)
        }
    }
}
