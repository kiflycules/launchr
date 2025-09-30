use anyhow::Result;
use chrono::{DateTime, Local};
use std::process::Command;

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
}

pub struct SSHModule {
    pub hosts: Vec<SSHHost>,
    pub active_sessions: Vec<SSHSession>,
    config: Config,
}

impl SSHModule {
    pub fn new(config: &Config) -> Self {
        let hosts = config.ssh_hosts.iter().map(|h| h.clone().into()).collect();
        Self { hosts, active_sessions: Vec::new(), config: config.clone() }
    }

    pub async fn connect(&mut self, index: usize) -> Result<()> {
        if index >= self.hosts.len() { return Ok(()); }
        let host = &self.hosts[index];

        #[cfg(unix)]
        {
            let ssh_command = if host.user.is_empty() { format!("{}:{}", host.host, host.port) } else { format!("{}@{}", host.user, host.host) };
            let terminal_emulators = vec![
                ("gnome-terminal", vec!["--", "ssh", &ssh_command]),
                ("xterm", vec!["-e", "ssh", &ssh_command]),
                ("konsole", vec!["-e", "ssh", &ssh_command]),
                ("alacritty", vec!["-e", "ssh", &ssh_command]),
                ("kitty", vec!["ssh", &ssh_command]),
            ];
            let mut connected = false;
            for (term, args) in terminal_emulators {
                if Command::new(term).args(&args).spawn().is_ok() { connected = true; break; }
            }
            if !connected { let _ = Command::new("ssh").arg(&ssh_command).spawn(); }
        }

        #[cfg(windows)]
        {
            let ssh_command = if host.user.is_empty() { format!("{}:{}", host.host, host.port) } else { format!("{}@{}", host.user, host.host) };
            Command::new("cmd").args(&["/C", "start", "ssh", &ssh_command]).spawn()?;
        }

        let session = SSHSession { name: host.name.clone(), host: host.host.clone(), status: "Connected".to_string(), connected_at: Local::now() };
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

    pub fn disconnect(&mut self, index: usize) {
        if index < self.active_sessions.len() { self.active_sessions.remove(index); }
    }
}


