use anyhow::Result;
use chrono::{DateTime, Local};
use std::collections::VecDeque;
use std::env;
use std::path::PathBuf;
use std::process::{Command, Stdio};

#[derive(Debug, Clone)]
pub struct CommandHistory {
    pub command: String,
    pub output: String,
    pub exit_code: Option<i32>,
    pub timestamp: DateTime<Local>,
    pub working_dir: PathBuf,
}

pub struct ShellModule {
    pub history: VecDeque<CommandHistory>,
    pub current_dir: PathBuf,
    pub max_history: usize,
    pub environment_vars: Vec<(String, String)>,
}

impl ShellModule {
    pub fn new() -> Result<Self> {
        let current_dir = env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));

        Ok(Self {
            history: VecDeque::new(),
            current_dir,
            max_history: 1000,
            environment_vars: Vec::new(),
        })
    }

    pub async fn execute_command(&mut self, command: &str) -> Result<CommandHistory> {
        let command = command.trim();

        // Handle built-in commands
        if let Some(result) = self.handle_builtin(command).await? {
            return Ok(result);
        }

        // Execute external command
        let result = self.execute_external(command).await?;

        // Add to history
        self.add_to_history(result.clone());

        Ok(result)
    }

    async fn handle_builtin(&mut self, command: &str) -> Result<Option<CommandHistory>> {
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.is_empty() {
            return Ok(None);
        }

        let cmd = parts[0];

        match cmd {
            "cd" => {
                let new_dir = if parts.len() > 1 {
                    let path = parts[1];
                    if path == "~" {
                        dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"))
                    } else if let Some(stripped) = path.strip_prefix("~/") {
                        if let Some(home) = dirs::home_dir() {
                            home.join(stripped)
                        } else {
                            PathBuf::from(path)
                        }
                    } else {
                        self.current_dir.join(path)
                    }
                } else {
                    dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"))
                };

                let output = if new_dir.exists() && new_dir.is_dir() {
                    self.current_dir = new_dir.canonicalize()?;
                    format!("Changed directory to: {}", self.current_dir.display())
                } else {
                    format!("Directory not found: {}", new_dir.display())
                };

                let result = CommandHistory {
                    command: command.to_string(),
                    output,
                    exit_code: Some(0),
                    timestamp: Local::now(),
                    working_dir: self.current_dir.clone(),
                };

                self.add_to_history(result.clone());
                return Ok(Some(result));
            }
            "pwd" => {
                let output = self.current_dir.display().to_string();
                let result = CommandHistory {
                    command: command.to_string(),
                    output,
                    exit_code: Some(0),
                    timestamp: Local::now(),
                    working_dir: self.current_dir.clone(),
                };
                self.add_to_history(result.clone());
                return Ok(Some(result));
            }
            "clear" => {
                // Signal to UI to clear display
                let result = CommandHistory {
                    command: command.to_string(),
                    output: String::from("[cleared]"),
                    exit_code: Some(0),
                    timestamp: Local::now(),
                    working_dir: self.current_dir.clone(),
                };
                self.add_to_history(result.clone());
                return Ok(Some(result));
            }
            "exit" | "quit" => {
                let result = CommandHistory {
                    command: command.to_string(),
                    output: String::from("[exit requested]"),
                    exit_code: Some(0),
                    timestamp: Local::now(),
                    working_dir: self.current_dir.clone(),
                };
                self.add_to_history(result.clone());
                return Ok(Some(result));
            }
            "export" => {
                if parts.len() > 1 {
                    let var_def = parts[1..].join(" ");
                    if let Some((key, value)) = var_def.split_once('=') {
                        // SAFETY: Setting environment variables is inherently unsafe in multi-threaded
                        // programs. This is acceptable here as it's a deliberate user action in a
                        // shell-like environment. Users should be aware of potential race conditions.
                        unsafe {
                            env::set_var(key, value);
                        }
                        self.environment_vars
                            .push((key.to_string(), value.to_string()));
                        let result = CommandHistory {
                            command: command.to_string(),
                            output: format!("Set {}={}", key, value),
                            exit_code: Some(0),
                            timestamp: Local::now(),
                            working_dir: self.current_dir.clone(),
                        };
                        self.add_to_history(result.clone());
                        return Ok(Some(result));
                    }
                }
            }
            "history" => {
                let output = self
                    .history
                    .iter()
                    .enumerate()
                    .map(|(i, h)| format!("{:4} {}", i, h.command))
                    .collect::<Vec<_>>()
                    .join("\n");

                let result = CommandHistory {
                    command: command.to_string(),
                    output,
                    exit_code: Some(0),
                    timestamp: Local::now(),
                    working_dir: self.current_dir.clone(),
                };
                self.add_to_history(result.clone());
                return Ok(Some(result));
            }
            _ => {}
        }

        Ok(None)
    }

    async fn execute_external(&self, command: &str) -> Result<CommandHistory> {
        let (shell, shell_arg) = self.get_shell();

        let child = Command::new(&shell)
            .arg(&shell_arg)
            .arg(command)
            .current_dir(&self.current_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let output = child.wait_with_output()?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        let combined_output = if stderr.is_empty() {
            stdout
        } else if stdout.is_empty() {
            stderr
        } else {
            format!("{}\n{}", stdout, stderr)
        };

        Ok(CommandHistory {
            command: command.to_string(),
            output: combined_output,
            exit_code: output.status.code(),
            timestamp: Local::now(),
            working_dir: self.current_dir.clone(),
        })
    }

    fn get_shell(&self) -> (String, String) {
        #[cfg(unix)]
        {
            let shell = env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
            (shell, "-c".to_string())
        }

        #[cfg(windows)]
        {
            ("powershell".to_string(), "-Command".to_string())
        }
    }

    fn add_to_history(&mut self, entry: CommandHistory) {
        self.history.push_front(entry);

        // Limit history size
        while self.history.len() > self.max_history {
            self.history.pop_back();
        }
    }

    pub fn search_history(&self, query: &str) -> Vec<&CommandHistory> {
        let query_lower = query.to_lowercase();
        self.history
            .iter()
            .filter(|h| h.command.to_lowercase().contains(&query_lower))
            .collect()
    }

    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    pub fn get_current_dir(&self) -> &PathBuf {
        &self.current_dir
    }

    pub fn get_prompt(&self) -> String {
        let user = env::var("USER")
            .or_else(|_| env::var("USERNAME"))
            .unwrap_or_else(|_| "user".to_string());

        let hostname = hostname::get()
            .ok()
            .and_then(|h| h.into_string().ok())
            .unwrap_or_else(|| "localhost".to_string());

        let dir = self
            .current_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("/");

        format!("{}@{}:{}$ ", user, hostname, dir)
    }
}
