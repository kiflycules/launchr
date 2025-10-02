use anyhow::Result;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct DockerContainer {
    pub id: String,
    pub name: String,
    pub image: String,
    pub status: String,
    #[allow(dead_code)]
    pub ports: String,
    #[allow(dead_code)]
    pub created: String,
}

#[derive(Debug, Clone)]
pub struct DockerImage {
    #[allow(dead_code)]
    pub id: String,
    pub repository: String,
    pub tag: String,
    pub size: String,
    #[allow(dead_code)]
    pub created: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DockerView {
    Containers,
    #[allow(dead_code)]
    Images,
    #[allow(dead_code)]
    Volumes,
    #[allow(dead_code)]
    Networks,
}

pub struct DockerModule {
    pub containers: Vec<DockerContainer>,
    pub images: Vec<DockerImage>,
    pub current_view: DockerView,
    pub show_all: bool, // Show stopped containers
}

impl DockerModule {
    pub fn new() -> Self {
        Self {
            containers: Vec::new(),
            images: Vec::new(),
            current_view: DockerView::Containers,
            show_all: false,
        }
    }

    pub fn refresh(&mut self) -> Result<()> {
        match self.current_view {
            DockerView::Containers => self.refresh_containers()?,
            DockerView::Images => self.refresh_images()?,
            _ => {}
        }
        Ok(())
    }

    pub fn refresh_containers(&mut self) -> Result<()> {
        self.containers.clear();

        let args = if self.show_all {
            vec![
                "ps",
                "-a",
                "--format",
                "{{.ID}}|{{.Names}}|{{.Image}}|{{.Status}}|{{.Ports}}|{{.CreatedAt}}",
            ]
        } else {
            vec![
                "ps",
                "--format",
                "{{.ID}}|{{.Names}}|{{.Image}}|{{.Status}}|{{.Ports}}|{{.CreatedAt}}",
            ]
        };

        let output = Command::new("docker").args(&args).output()?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if line.is_empty() {
                    continue;
                }
                let parts: Vec<&str> = line.split('|').collect();
                if parts.len() >= 6 {
                    self.containers.push(DockerContainer {
                        id: parts[0].to_string(),
                        name: parts[1].to_string(),
                        image: parts[2].to_string(),
                        status: parts[3].to_string(),
                        ports: parts[4].to_string(),
                        created: parts[5].to_string(),
                    });
                }
            }
        }

        Ok(())
    }

    pub fn refresh_images(&mut self) -> Result<()> {
        self.images.clear();

        let output = Command::new("docker")
            .args([
                "images",
                "--format",
                "{{.ID}}|{{.Repository}}|{{.Tag}}|{{.Size}}|{{.CreatedAt}}",
            ])
            .output()?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if line.is_empty() {
                    continue;
                }
                let parts: Vec<&str> = line.split('|').collect();
                if parts.len() >= 5 {
                    self.images.push(DockerImage {
                        id: parts[0].to_string(),
                        repository: parts[1].to_string(),
                        tag: parts[2].to_string(),
                        size: parts[3].to_string(),
                        created: parts[4].to_string(),
                    });
                }
            }
        }

        Ok(())
    }

    #[allow(dead_code)]
    pub fn start_container(&self, index: usize) -> Result<()> {
        if index >= self.containers.len() {
            return Ok(());
        }
        let container = &self.containers[index];

        Command::new("docker")
            .args(["start", &container.id])
            .output()?;

        Ok(())
    }

    #[allow(dead_code)]
    pub fn stop_container(&self, index: usize) -> Result<()> {
        if index >= self.containers.len() {
            return Ok(());
        }
        let container = &self.containers[index];

        Command::new("docker")
            .args(["stop", &container.id])
            .output()?;

        Ok(())
    }

    #[allow(dead_code)]
    pub fn restart_container(&self, index: usize) -> Result<()> {
        if index >= self.containers.len() {
            return Ok(());
        }
        let container = &self.containers[index];

        Command::new("docker")
            .args(["restart", &container.id])
            .output()?;

        Ok(())
    }

    #[allow(dead_code)]
    pub fn remove_container(&self, index: usize) -> Result<()> {
        if index >= self.containers.len() {
            return Ok(());
        }
        let container = &self.containers[index];

        Command::new("docker")
            .args(["rm", "-f", &container.id])
            .output()?;

        Ok(())
    }

    pub fn exec_into_container(&self, index: usize) -> Result<()> {
        if index >= self.containers.len() {
            return Ok(());
        }
        let container = &self.containers[index];

        #[cfg(unix)]
        {
            // Try to open in a new terminal
            let docker_cmd = format!(
                "docker exec -it {} /bin/bash || docker exec -it {} /bin/sh",
                container.id, container.id
            );

            let terminals = vec![
                ("gnome-terminal", vec!["--", "bash", "-c", &docker_cmd]),
                ("xterm", vec!["-e", "bash", "-c", &docker_cmd]),
                ("konsole", vec!["-e", "bash", "-c", &docker_cmd]),
                ("alacritty", vec!["-e", "bash", "-c", &docker_cmd]),
            ];

            for (term, args) in terminals {
                if Command::new(term).args(&args).spawn().is_ok() {
                    return Ok(());
                }
            }
        }

        #[cfg(windows)]
        {
            let docker_cmd = format!("docker exec -it {} cmd", container.id);
            Command::new("cmd")
                .args(&["/C", "start", "cmd", "/K", &docker_cmd])
                .spawn()?;
        }

        Ok(())
    }

    #[allow(dead_code)]
    pub fn view_logs(&self, index: usize) -> Result<String> {
        if index >= self.containers.len() {
            return Ok(String::new());
        }
        let container = &self.containers[index];

        let output = Command::new("docker")
            .args(["logs", "--tail", "50", &container.id])
            .output()?;

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    #[allow(dead_code)]
    pub fn remove_image(&self, index: usize) -> Result<()> {
        if index >= self.images.len() {
            return Ok(());
        }
        let image = &self.images[index];

        Command::new("docker").args(["rmi", &image.id]).output()?;

        Ok(())
    }

    #[allow(dead_code)]
    pub fn toggle_show_all(&mut self) {
        self.show_all = !self.show_all;
    }

    #[allow(dead_code)]
    pub fn switch_view(&mut self, view: DockerView) {
        self.current_view = view;
    }

    #[allow(dead_code)]
    pub fn prune_system(&self) -> Result<String> {
        let output = Command::new("docker")
            .args(["system", "prune", "-f"])
            .output()?;

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}
