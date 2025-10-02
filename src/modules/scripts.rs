use anyhow::Result;
use std::collections::HashSet;
use std::process::Stdio;
use tokio::process::Command;

use crate::config::{Config, ScriptConfig};

#[derive(Debug, Clone)]
pub struct Script {
    pub name: String,
    pub command: String,
    pub description: String,
}

impl From<ScriptConfig> for Script {
    fn from(config: ScriptConfig) -> Self {
        Self {
            name: config.name,
            command: config.command,
            description: config.description,
        }
    }
}

pub struct ScriptsModule {
    pub scripts: Vec<Script>,
    running_scripts: HashSet<usize>,
    config: Config,
}

impl ScriptsModule {
    pub fn new(config: &Config) -> Self {
        let scripts = config.scripts.iter().map(|s| s.clone().into()).collect();
        Self {
            scripts,
            running_scripts: HashSet::new(),
            config: config.clone(),
        }
    }

    pub async fn run_script(&mut self, index: usize) -> Result<()> {
        if index >= self.scripts.len() {
            return Ok(());
        }
        let script = &self.scripts[index];
        self.running_scripts.insert(index);

        let parts: Vec<String> = script
            .command
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();
        if parts.is_empty() {
            return Ok(());
        }
        let program = parts[0].clone();
        let args: Vec<String> = parts[1..].to_vec();

        tokio::spawn(async move {
            let _ = Command::new(program)
                .args(args)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn();
        });

        // In a real implementation we would track the JoinHandle and mark completion.

        Ok(())
    }

    pub fn is_running(&self, index: usize) -> bool {
        self.running_scripts.contains(&index)
    }

    pub fn add_from_string(&mut self, input: &str) -> Result<()> {
        let parts: Vec<&str> = input.split('|').collect();
        if parts.len() < 2 {
            anyhow::bail!("Invalid format. Use: name|command");
        }
        let name = parts[0].trim().to_string();
        let command = parts[1].trim().to_string();
        let description = if parts.len() > 2 {
            parts[2].trim().to_string()
        } else {
            String::new()
        };
        let script = Script {
            name: name.clone(),
            command: command.clone(),
            description: description.clone(),
        };
        let config_script = ScriptConfig {
            name,
            command,
            description,
        };
        self.scripts.push(script);
        self.config.add_script(config_script)?;
        Ok(())
    }

    pub fn delete(&mut self, index: usize) {
        if index < self.scripts.len() {
            self.scripts.remove(index);
            let _ = self.config.remove_script(index);
        }
    }

    pub async fn schedule_script(&mut self, index: usize, interval_secs: u64) -> Result<()> {
        if index >= self.scripts.len() {
            return Ok(());
        }
        let script = self.scripts[index].clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(interval_secs)).await;
                let parts: Vec<&str> = script.command.split_whitespace().collect();
                if parts.is_empty() {
                    continue;
                }
                let program = parts[0];
                let args = &parts[1..];
                let _ = Command::new(program)
                    .args(args)
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn();
            }
        });
        Ok(())
    }
}
