use anyhow::Result;
use chrono::{DateTime, Local};
use std::process::Command;
use sysinfo::{ProcessesToUpdate, System};

use crate::config::{Config, SSHHostConfig};

#[derive(Debug, Clone)]
pub struct SSHHost {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub user: String,
}

impl From<SSHHostConfig> for SSHHost {
    fn from(config: SSHHostConfig) -> Self {
        Self { name: config.name, host: config.host, port: config.port, user: config.user }
    }
}

#[derive(Debug, Clone)]
pub struct SSHSession {
    pub name: String,
    pub host: String,
    pub status: String,
    pub connected_at: DateTime<Local>,
    pub pid: Option<u32>, // Track the PID of the SSH process
}

pub struct SSHModule {
    pub hosts: Vec<SSHHost>,
    pub active_sessions: Vec<SSHSession>,
    config: Config,
    system: System,
}

impl SSHModule {
    pub fn new(config: &Config) -> Self {
        let hosts = config.ssh_hosts.iter().map(|h| h.clone().into()).collect();
        let system = System::new_all();
        Self { hosts, active_sessions: Vec::new(), config: config.clone(), system }
    }

    pub async fn connect(&mut self, index: usize) -> Result<()> {
        if index >= self.hosts.len() { return Ok(()); }
        let host = &self.hosts[index];

        let mut session_pid: Option<u32> = None;

        #[cfg(unix)]
        {
            let target = if host.user.is_empty() { host.host.clone() } else { format!("{}@{}", host.user, host.host) };
            let ssh_args = vec!["ssh", "-p", &host.port.to_string(), &target];
            let terminal_emulators = vec![
                ("gnome-terminal", { let mut v = vec!["--"]; v.extend(ssh_args.clone()); v }),
                ("xterm", { let mut v = vec!["-e"]; v.extend(ssh_args.clone()); v }),
                ("konsole", { let mut v = vec!["-e"]; v.extend(ssh_args.clone()); v }),
                ("alacritty", { let mut v = vec!["-e"]; v.extend(ssh_args.clone()); v }),
                ("kitty", ssh_args.clone()),
            ];
            let mut connected = false;
            for (term, args) in terminal_emulators {
                if let Ok(child) = Command::new(term).args(&args).spawn() {
                    session_pid = child.id().into();
                    connected = true;
                    break;
                }
            }
            if !connected {
                if let Ok(child) = Command::new("ssh").args(["-p", &host.port.to_string(), &target]).spawn() {
                    session_pid = child.id().into();
                }
            }
        }

        #[cfg(windows)]
        {
            let target = if host.user.is_empty() { host.host.clone() } else { format!("{}@{}", host.user, host.host) };
            if let Ok(child) = Command::new("cmd").args(&["/C", "start", "ssh", "-p", &host.port.to_string(), &target]).spawn() {
                session_pid = child.id().into();
            }
        }

        let session = SSHSession { 
            name: host.name.clone(), 
            host: host.host.clone(), 
            status: "Connected".to_string(), 
            connected_at: Local::now(),
            pid: session_pid,
        };
        self.active_sessions.push(session);
        Ok(())
    }

    pub fn add_from_string(&mut self, input: &str) -> Result<()> {
        let parts: Vec<&str> = input.split('|').collect();
        if parts.len() < 2 { anyhow::bail!("Invalid format. Use: name|user@host:port"); }
        let name = parts[0].trim().to_string();
        let connection_str = parts[1].trim();
        let (user, host_port) = if connection_str.contains('@') {
            let split: Vec<&str> = connection_str.split('@').collect();
            (split[0].to_string(), split[1])
        } else { (String::new(), connection_str) };
        let (host, port) = if host_port.contains(':') {
            let split: Vec<&str> = host_port.split(':').collect();
            (split[0].to_string(), split[1].parse().unwrap_or(22))
        } else { (host_port.to_string(), 22) };
        let ssh_host = SSHHost { name: name.clone(), host: host.clone(), port, user: user.clone() };
        let config_host = SSHHostConfig { name, host, port, user };
        self.hosts.push(ssh_host);
        self.config.add_ssh_host(config_host)?;
        Ok(())
    }

    pub fn delete(&mut self, index: usize) {
        if index < self.hosts.len() {
            self.hosts.remove(index);
            let _ = self.config.remove_ssh_host(index);
        }
    }

    pub fn disconnect(&mut self, index: usize) -> Result<()> {
        if index < self.active_sessions.len() {
            let session = self.active_sessions.remove(index);
            
            // Try to terminate the SSH process if we have its PID
            if let Some(pid) = session.pid {
                #[cfg(unix)]
                {
                    use std::process::Command as StdCommand;
                    let _ = StdCommand::new("kill")
                        .arg("-TERM")
                        .arg(pid.to_string())
                        .output();
                }

                #[cfg(windows)]
                {
                    use std::process::Command as StdCommand;
                    let _ = StdCommand::new("taskkill")
                        .args(&["/PID", &pid.to_string(), "/F"])
                        .output();
                }
            }
            
            Ok(())
        } else {
            anyhow::bail!("Invalid session index");
        }
    }

    pub fn refresh_session_status(&mut self) {
        // Refresh processes
        self.system.refresh_processes(ProcessesToUpdate::All, false);

        // Build a set of detected sessions by scanning running ssh processes
        #[cfg(windows)]
        let ssh_name = "ssh.exe";
        #[cfg(unix)]
        let ssh_name = "ssh";

        let mut detected: Vec<(String, String)> = Vec::new(); // (name, host)

        for (_pid, proc) in self.system.processes() {
            let name = proc.name().to_string_lossy();
            if name != ssh_name { continue; }
            let cmdline = proc
                .cmd()
                .iter()
                .map(|s| s.to_string_lossy().into_owned())
                .collect::<Vec<String>>()
                .join(" ");

            for h in &self.hosts {
                let target = if h.user.is_empty() { h.host.clone() } else { format!("{}@{}", h.user, h.host) };
                let port_flag = format!("-p {}", h.port);
                if cmdline.contains(&target) || cmdline.contains(&h.host) {
                    // Optional but helpful: ensure port matches if present
                    if cmdline.contains(&port_flag) || !cmdline.contains("-p ") {
                        detected.push((h.name.clone(), h.host.clone()));
                    }
                }
            }
        }

        // Update active_sessions based on detection
        // Preserve connected_at if already present
        let mut new_sessions: Vec<SSHSession> = Vec::new();
        for (name, host) in detected {
            if let Some(existing) = self.active_sessions.iter().find(|s| s.name == name && s.host == host) {
                let mut s = existing.clone();
                s.status = "Connected".to_string();
                new_sessions.push(s);
            } else {
                new_sessions.push(SSHSession { name: name.clone(), host: host.clone(), status: "Connected".to_string(), connected_at: Local::now(), pid: None });
            }
        }

        self.active_sessions = new_sessions;
    }
}


