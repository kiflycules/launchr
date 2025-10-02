use anyhow::Result;
use std::time::{Duration, Instant};

use crate::config::Config;
use crate::modules::{
    apps::AppsModule, bookmarks::BookmarksModule, clipboard::ClipboardModule,
    configs::ConfigsModule, docker::DockerModule, git::GitModule, history::ShellHistoryModule,
    network::NetworkModule, notifications::NotificationsModule, scratchpad::ScratchpadModule,
    scripts::ScriptsModule, services::ServicesModule, shell::ShellModule, ssh::SSHModule,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MenuSection {
    Dashboard,
    Apps,
    Bookmarks,
    Clipboard,
    Docker,
    Network,
    Ssh,
    Scripts,
    Notifications,
    History,
    Configs,
    Git,
    Scratchpad,
    Shell,
    Services,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AppState {
    Normal,
    Input,
    Confirm,
    Search,
    ShellInput,
}

pub struct App {
    pub current_section: MenuSection,
    pub state: AppState,
    pub selected_index: usize,
    pub input_buffer: String,
    pub input_cursor: usize,
    pub input_prompt: String,
    pub confirm_message: String,
    pub status_message: String,
    pub show_detail: bool,
    pub last_refresh: Instant,

    pub apps_module: AppsModule,
    pub bookmarks_module: BookmarksModule,
    pub clipboard_module: ClipboardModule,
    pub configs_module: ConfigsModule,
    pub docker_module: DockerModule,
    pub git_module: GitModule,
    pub network_module: NetworkModule,
    pub ssh_module: SSHModule,
    pub scripts_module: ScriptsModule,
    pub notifications_module: NotificationsModule,
    pub show_help: bool,
    pub pending_initial_scan: bool,

    // Fuzzy search state
    pub search_query: String,
    pub search_cursor: usize,
    pub search_results: Vec<SearchResult>,
    pub search_selected: usize,

    // Shell
    pub shell_module: ShellHistoryModule,
    pub scratchpad_module: ScratchpadModule,
    pub shell_terminal_module: ShellModule,
    pub services_module: ServicesModule,

    // Shell input state
    pub shell_input_buffer: String,
    pub shell_input_cursor: usize,

    // Scratchpad search
    pub scratchpad_search_query: String,
    pub scratchpad_search_results: Vec<usize>,
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub section: MenuSection,
    pub index: usize,
    pub label: String,
    score: i32,
}

impl App {
    pub async fn new() -> Result<Self> {
        let config = Config::load()?;

        let apps_module = AppsModule::new();

        let bookmarks_module = BookmarksModule::new(&config);
        let clipboard_module = ClipboardModule::new();
        let configs_module = ConfigsModule::new(Some(config.config_directory.clone()))?;
        let docker_module = DockerModule::new();
        let git_module = GitModule::new(&config.git_search_paths);
        let network_module = NetworkModule::new();
        let ssh_module = SSHModule::new(&config);
        let scripts_module = ScriptsModule::new(&config);
        let notifications_module = NotificationsModule::new();
        let shell_module = ShellHistoryModule::new();
        let scratchpad_module = ScratchpadModule::new(
            config.scratchpad_directory.clone(),
            config.scratchpad_editor.clone(),
        )?;
        let shell_terminal_module = ShellModule::new()?;
        let services_module = ServicesModule::new();

        Ok(Self {
            current_section: MenuSection::Dashboard,
            state: AppState::Normal,
            selected_index: 0,
            input_buffer: String::new(),
            input_cursor: 0,
            input_prompt: String::new(),
            confirm_message: String::new(),
            status_message: String::from("Welcome to launchr! Press '?' for help"),
            show_detail: false,
            last_refresh: Instant::now(),
            apps_module,
            bookmarks_module,
            clipboard_module,
            configs_module,
            docker_module,
            git_module,
            network_module,
            ssh_module,
            scripts_module,
            notifications_module,
            show_help: false,
            pending_initial_scan: true,
            search_query: String::new(),
            search_cursor: 0,
            search_results: Vec::new(),
            search_selected: 0,
            shell_module,
            scratchpad_module,
            shell_terminal_module,
            services_module,
            shell_input_buffer: String::new(),
            shell_input_cursor: 0,
            scratchpad_search_query: String::new(),
            scratchpad_search_results: Vec::new(),
        })
    }

    pub fn next_item(&mut self) {
        let max = self.get_current_list_len();
        if max > 0 {
            self.selected_index = (self.selected_index + 1) % max;
            
            // Clear preview when navigating to different config
            if self.current_section == MenuSection::Configs {
                self.configs_module.exit_preview_mode();
            }
        }
    }

    pub fn previous_item(&mut self) {
        let max = self.get_current_list_len();
        if max > 0 {
            self.selected_index = if self.selected_index == 0 {
                max - 1
            } else {
                self.selected_index - 1
            };
            
            // Clear preview when navigating to different config
            if self.current_section == MenuSection::Configs {
                self.configs_module.exit_preview_mode();
            }
        }
    }

    pub async fn activate_item(&mut self) -> Result<()> {
        match self.current_section {
            MenuSection::Apps => {
                let available_apps_len = self.apps_module.available_apps.len();
                if self.selected_index < available_apps_len {
                    // Launch an available app
                    let app_name = self.apps_module.available_apps[self.selected_index].clone();
                    self.apps_module.launch_app(&app_name).await?;
                    self.status_message = format!("Launched: {}", app_name);
                    self.notifications_module
                        .push("App Launched", &app_name, "info");
                }
                // If selected_index >= available_apps_len, it's a process - no action on Enter
            }
            MenuSection::Bookmarks => {
                if self.selected_index < self.bookmarks_module.bookmarks.len() {
                    self.bookmarks_module.open_bookmark(self.selected_index)?;
                    self.status_message = format!(
                        "Opened: {}",
                        self.bookmarks_module.bookmarks[self.selected_index].name
                    );
                    let b = &self.bookmarks_module.bookmarks[self.selected_index];
                    self.notifications_module
                        .push("Bookmark Opened", &b.name, "info");
                }
            }
            MenuSection::Ssh => {
                if self.selected_index < self.ssh_module.hosts.len() {
                    let host_name = self.ssh_module.hosts[self.selected_index].name.clone();
                    self.status_message = format!("Connecting to {}...", host_name);
                    self.ssh_module.connect(self.selected_index).await?;
                    self.notifications_module
                        .push("SSH Connected", &host_name, "info");
                }
            }
            MenuSection::Clipboard => {
                if self.selected_index < self.clipboard_module.entries.len() {
                    if let Err(e) = self.clipboard_module.copy_to_clipboard(self.selected_index) {
                        self.status_message = format!("Failed to copy to clipboard: {}", e);
                    } else {
                        self.status_message = "Copied to clipboard".to_string();
                        self.notifications_module.push(
                            "Clipboard",
                            "Content copied to clipboard",
                            "info",
                        );
                    }
                }
            }
            MenuSection::Docker => {
                if self.selected_index < self.docker_module.containers.len() {
                    if let Err(e) = self.docker_module.exec_into_container(self.selected_index) {
                        self.status_message = format!("Failed to exec into container: {}", e);
                    } else {
                        let container_name =
                            &self.docker_module.containers[self.selected_index].name;
                        self.status_message =
                            format!("Executed into container: {}", container_name);
                        self.notifications_module.push(
                            "Docker",
                            format!("Executed into {}", container_name),
                            "info",
                        );
                    }
                }
            }
            MenuSection::Scripts => {
                if self.selected_index < self.scripts_module.scripts.len() {
                    let script_name = self.scripts_module.scripts[self.selected_index]
                        .name
                        .clone();
                    self.scripts_module.run_script(self.selected_index).await?;
                    self.status_message = format!("Executed: {}", script_name);
                    self.notifications_module
                        .push("Script Executed", &script_name, "info");
                }
            }
            MenuSection::History => {
                if self.selected_index < self.shell_module.entries.len() {
                    let cmd = self.shell_module.entries[self.selected_index]
                        .command
                        .clone();
                    self.shell_module.run_entry(self.selected_index);
                    self.status_message = format!("Ran: {}", cmd);
                    self.notifications_module.push("History Ran", &cmd, "info");
                }
            }
            MenuSection::Git => {
                if self.selected_index < self.git_module.repos.len() {
                    if let Err(e) = self.git_module.open_in_editor(self.selected_index) {
                        self.status_message = format!("Failed to open repository: {}", e);
                    } else {
                        let repo_name = &self.git_module.repos[self.selected_index].name;
                        self.status_message = format!("Opened repository: {}", repo_name);
                        self.notifications_module.push(
                            "Git",
                            format!("Opened {}", repo_name),
                            "info",
                        );
                    }
                }
            }
            MenuSection::Scratchpad => {
                if self.selected_index < self.scratchpad_module.notes.len() {
                    if let Err(e) = self.scratchpad_module.open_existing(self.selected_index) {
                        self.status_message = format!("Failed to open note: {}", e);
                    } else {
                        let note_name = &self.scratchpad_module.notes[self.selected_index].name;
                        self.status_message = format!("Opened note: {}", note_name);
                        self.notifications_module.push(
                            "Scratchpad",
                            format!("Opened {}", note_name),
                            "info",
                        );
                    }
                }
            }
            MenuSection::Shell => {
                // Shell terminal - Enter key handled separately in shell input mode
                self.status_message = "Type commands in the shell below".to_string();
            }
            MenuSection::Services => {
                // Services - use other keys for actions
                self.status_message =
                    "Use 's' to start, 'S' to stop, 'r' to restart service".to_string();
            }
            MenuSection::Network => {
                // Network view doesn't have a default action on Enter
                self.status_message = "Use 'v' to switch views, 'f' to filter".to_string();
            }
            MenuSection::Configs => {
                if self.selected_index < self.configs_module.configs.len() {
                    if let Err(e) = self.configs_module.open_config(self.selected_index) {
                        self.status_message = format!("Failed to open config: {}", e);
                    } else {
                        let config_name = &self.configs_module.configs[self.selected_index].name;
                        self.status_message = format!("Opened config: {}", config_name);
                        self.notifications_module.push(
                            "Config",
                            format!("Opened {}", config_name),
                            "info",
                        );
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub fn new_item(&mut self) {
        self.state = AppState::Input;
        self.input_buffer.clear();
        self.input_cursor = 0;
        self.input_prompt = match self.current_section {
            MenuSection::Bookmarks => "Enter bookmark (name|path|type): ".to_string(),
            MenuSection::Configs => {
                "Enter config (name|path|category|description|editor): ".to_string()
            }
            MenuSection::Ssh => "Enter SSH host (name|user@host:port): ".to_string(),
            MenuSection::Scripts => "Enter script (name|command): ".to_string(),
            MenuSection::Scratchpad => {
                "Enter note name (or leave empty for auto-name): ".to_string()
            }
            _ => "Enter value: ".to_string(),
        };
    }

    pub fn delete_item(&mut self) {
        self.state = AppState::Confirm;
        self.confirm_message = "Delete selected item? (y/n)".to_string();
    }

    pub async fn refresh(&mut self) -> Result<()> {
        match self.current_section {
            MenuSection::Apps => {
                self.apps_module.refresh_running_processes().await?;
                self.status_message = "Refreshed running processes".to_string();
            }
            MenuSection::Dashboard => {
                self.apps_module.refresh_running_processes().await?;
                self.status_message = "Refreshed dashboard".to_string();
            }
            MenuSection::Clipboard => {
                if let Err(e) = self.clipboard_module.refresh() {
                    self.status_message = format!("Failed to refresh clipboard: {}", e);
                } else {
                    self.status_message = "Refreshed clipboard".to_string();
                }
            }
            MenuSection::Docker => {
                if let Err(e) = self.docker_module.refresh() {
                    self.status_message = format!("Failed to refresh docker: {}", e);
                } else {
                    self.status_message = "Refreshed docker containers/images".to_string();
                }
            }
            MenuSection::History => {
                self.shell_module.refresh();
                self.status_message = "Refreshed history".to_string();
            }
            MenuSection::Git => {
                if let Err(e) = self.git_module.refresh() {
                    self.status_message = format!("Failed to refresh git: {}", e);
                } else {
                    self.status_message = "Refreshed git repositories".to_string();
                }
            }
            MenuSection::Network => {
                if let Err(e) = self.network_module.refresh() {
                    self.status_message = format!("Failed to refresh network: {}", e);
                } else {
                    self.status_message = "Refreshed network information".to_string();
                }
            }
            MenuSection::Scratchpad => {
                if let Err(e) = self.scratchpad_module.refresh() {
                    self.status_message = format!("Failed to refresh scratchpad: {}", e);
                } else {
                    self.status_message = "Refreshed scratchpad notes".to_string();
                }
            }
            MenuSection::Services => {
                if let Err(e) = self.services_module.refresh() {
                    self.status_message = format!("Failed to refresh services: {}", e);
                } else {
                    self.status_message = "Refreshed services".to_string();
                }
            }
            MenuSection::Configs => {
                if let Err(e) = self.configs_module.refresh() {
                    self.status_message = format!("Failed to refresh configs: {}", e);
                } else {
                    self.status_message = "Refreshed configs".to_string();
                }
            }
            _ => {}
        }
        self.last_refresh = Instant::now();
        Ok(())
    }

    pub fn start_service(&mut self) {
        if self.current_section == MenuSection::Services
            && let Ok(msg) = self.services_module.start_service(self.selected_index) {
                self.status_message = format!("Started service: {}", msg);
                self.notifications_module
                    .push("Service", "Service started", "info");
            }
    }

    pub fn stop_service(&mut self) {
        if self.current_section == MenuSection::Services
            && let Ok(msg) = self.services_module.stop_service(self.selected_index) {
                self.status_message = format!("Stopped service: {}", msg);
                self.notifications_module
                    .push("Service", "Service stopped", "warning");
            }
    }

    pub fn restart_service(&mut self) {
        if self.current_section == MenuSection::Services
            && let Ok(msg) = self.services_module.restart_service(self.selected_index) {
                self.status_message = format!("Restarted service: {}", msg);
                self.notifications_module
                    .push("Service", "Service restarted", "info");
            }
    }

    pub fn enable_service(&mut self) {
        if self.current_section == MenuSection::Services
            && let Ok(msg) = self.services_module.enable_service(self.selected_index) {
                self.status_message = format!("Enabled service: {}", msg);
                self.notifications_module
                    .push("Service", "Service enabled", "info");
            }
    }

    pub fn disable_service(&mut self) {
        if self.current_section == MenuSection::Services
            && let Ok(msg) = self.services_module.disable_service(self.selected_index) {
                self.status_message = format!("Disabled service: {}", msg);
                self.notifications_module
                    .push("Service", "Service disabled", "warning");
            }
    }

    pub fn view_service_logs(&mut self) {
        if self.current_section == MenuSection::Services
            && let Ok(logs) = self
                .services_module
                .get_service_logs(self.selected_index, 50)
            {
                self.status_message =
                    format!("Logs: {}...", logs.chars().take(100).collect::<String>());
            }
    }

    pub fn search_services(&mut self) {
        if self.current_section == MenuSection::Services {
            self.state = AppState::Input;
            self.input_buffer.clear();
            self.input_cursor = 0;
            self.input_prompt = "Search services: ".to_string();
        }
    }

    pub fn execute_service_search(&mut self, query: String) {
        let results = self.services_module.search(&query);
        self.status_message = format!("Found {} services matching \"{}\"", results.len(), query);
        if !results.is_empty() {
            self.selected_index = results[0];
        }
    }

    pub fn stop_selected(&mut self) {
        if self.current_section == MenuSection::Apps {
            // Check if we're selecting from running processes (bottom half of Apps view)
            let available_apps_len = self.apps_module.available_apps.len();
            if self.selected_index >= available_apps_len {
                let process_index = self.selected_index - available_apps_len;
                if let Some(process) = self
                    .apps_module
                    .running_processes
                    .get(process_index)
                    .cloned()
                {
                    let pid = process.pid;
                    let name = process.name.clone();
                    match self.apps_module.stop_process(pid) {
                        Ok(()) => {
                            self.status_message = format!("Stopped process: {}", name);
                            self.notifications_module
                                .push("Process Stopped", &name, "warning");
                        }
                        Err(e) => {
                            self.status_message = format!("Failed to stop process {}: {}", name, e);
                            self.notifications_module
                                .push("Process Stop Failed", &name, "error");
                        }
                    }
                }
            }
        } else if self.current_section == MenuSection::Dashboard {
            // In Dashboard, we can stop processes directly
            if let Some(process) = self
                .apps_module
                .running_processes
                .get(self.selected_index)
                .cloned()
            {
                let pid = process.pid;
                let name = process.name.clone();
                match self.apps_module.stop_process(pid) {
                    Ok(()) => {
                        self.status_message = format!("Stopped process: {}", name);
                        self.notifications_module
                            .push("Process Stopped", &name, "warning");
                    }
                    Err(e) => {
                        self.status_message = format!("Failed to stop process {}: {}", name, e);
                        self.notifications_module
                            .push("Process Stop Failed", &name, "error");
                    }
                }
            }
        }
    }

    pub fn toggle_detail(&mut self) {
        self.show_detail = !self.show_detail;
    }

    // List navigation helpers
    pub fn page_up(&mut self) {
        let len = self.get_current_list_len();
        if len == 0 {
            return;
        }
        let step = 10usize;
        self.selected_index = self.selected_index.saturating_sub(step);
    }

    pub fn page_down(&mut self) {
        let len = self.get_current_list_len();
        if len == 0 {
            return;
        }
        let step = 10usize;
        self.selected_index = usize::min(
            self.selected_index.saturating_add(step),
            len.saturating_sub(1),
        );
    }

    pub fn go_home(&mut self) {
        self.selected_index = 0;
    }
    pub fn go_end(&mut self) {
        let len = self.get_current_list_len();
        if len == 0 {
            return;
        }
        self.selected_index = len - 1;
    }

    pub fn next_section(&mut self) {
        self.current_section = match self.current_section {
            MenuSection::Dashboard => MenuSection::Apps,
            MenuSection::Apps => MenuSection::Bookmarks,
            MenuSection::Bookmarks => MenuSection::Clipboard,
            MenuSection::Clipboard => MenuSection::Docker,
            MenuSection::Configs => MenuSection::Scratchpad,
            MenuSection::Docker => MenuSection::Network,
            MenuSection::Network => MenuSection::Ssh,
            MenuSection::Ssh => MenuSection::Scripts,
            MenuSection::Scripts => MenuSection::Git,
            MenuSection::Git => MenuSection::History,
            MenuSection::History => MenuSection::Configs,
            MenuSection::Scratchpad => MenuSection::Shell,
            MenuSection::Shell => MenuSection::Services,
            MenuSection::Services => MenuSection::Notifications,
            MenuSection::Notifications => MenuSection::Dashboard,
        };
        self.selected_index = 0;
    }

    pub fn previous_section(&mut self) {
        self.current_section = match self.current_section {
            MenuSection::Dashboard => MenuSection::Notifications,
            MenuSection::Apps => MenuSection::Dashboard,
            MenuSection::Bookmarks => MenuSection::Apps,
            MenuSection::Clipboard => MenuSection::Bookmarks,
            MenuSection::Configs => MenuSection::History,
            MenuSection::Docker => MenuSection::Clipboard,
            MenuSection::Network => MenuSection::Docker,
            MenuSection::Ssh => MenuSection::Network,
            MenuSection::Scripts => MenuSection::Ssh,
            MenuSection::Git => MenuSection::Scripts,
            MenuSection::History => MenuSection::Git,
            MenuSection::Scratchpad => MenuSection::Configs,
            MenuSection::Shell => MenuSection::Scratchpad,
            MenuSection::Services => MenuSection::Shell,
            MenuSection::Notifications => MenuSection::Services,
        };
        self.selected_index = 0;
    }

    pub fn cancel_input(&mut self) {
        self.state = AppState::Normal;
        self.input_buffer.clear();
        self.input_cursor = 0;
    }

    pub async fn submit_input(&mut self) -> Result<()> {
        let input = self.input_buffer.clone();
        match self.current_section {
            MenuSection::Bookmarks => {
                self.bookmarks_module.add_from_string(&input)?;
                self.status_message = "Bookmark added".to_string();
                self.notifications_module
                    .push("Bookmark Added", &input, "info");
            }
            MenuSection::Ssh => {
                self.ssh_module.add_from_string(&input)?;
                self.status_message = "SSH host added".to_string();
                self.notifications_module
                    .push("SSH Host Added", &input, "info");
            }
            MenuSection::Scripts => {
                self.scripts_module.add_from_string(&input)?;
                self.status_message = "Script added".to_string();
                self.notifications_module
                    .push("Script Added", &input, "info");
            }
            MenuSection::Scratchpad => {
                // Check if this is a rename, search, or export action
                if self.input_prompt.starts_with("Rename")
                    || self.input_prompt.starts_with("Search")
                    || self.input_prompt.starts_with("Export")
                {
                    self.execute_scratchpad_action(input)?;
                } else {
                    // Create new note
                    let name = if input.trim().is_empty() {
                        None
                    } else {
                        Some(input.clone())
                    };
                    if let Err(e) = self.scratchpad_module.new_and_open(name) {
                        self.status_message = format!("Failed to create note: {}", e);
                    } else {
                        self.status_message = "Note created and opened".to_string();
                        self.notifications_module
                            .push("Scratchpad", "New note created", "info");
                        self.scratchpad_module.refresh()?;
                    }
                }
            }
            MenuSection::Shell => {
                // Check if this is a search action
                if self.input_prompt.starts_with("Search shell") {
                    self.execute_shell_action(input)?;
                }
            }
            MenuSection::Services => {
                if self.input_prompt.starts_with("Search services") {
                    self.execute_service_search(input);
                }
            }
            MenuSection::Configs => {
                // Check if this is a search action
                if self.input_prompt.starts_with("Search configs") {
                    self.execute_configs_search(input);
                } else {
                    self.configs_module.add_from_string(&input)?;
                    self.status_message = "Config added".to_string();
                    self.notifications_module
                        .push("Config Added", &input, "info");
                }
            }
            _ => {}
        }
        self.cancel_input();
        Ok(())
    }

    pub async fn confirm_action(&mut self) -> Result<()> {
        match self.current_section {
            MenuSection::Bookmarks => {
                self.bookmarks_module.delete(self.selected_index);
                self.status_message = "Bookmark deleted".to_string();
                self.notifications_module
                    .push("Bookmark Deleted", "", "warning");
            }
            MenuSection::Ssh => {
                self.ssh_module.delete(self.selected_index);
                self.status_message = "SSH host deleted".to_string();
                self.notifications_module
                    .push("SSH Host Deleted", "", "warning");
            }
            MenuSection::Scripts => {
                self.scripts_module.delete(self.selected_index);
                self.status_message = "Script deleted".to_string();
                self.notifications_module
                    .push("Script Deleted", "", "warning");
            }
            MenuSection::Scratchpad => {
                if let Err(e) = self.scratchpad_module.delete(self.selected_index) {
                    self.status_message = format!("Failed to delete note: {}", e);
                } else {
                    self.status_message = "Note deleted".to_string();
                    self.notifications_module
                        .push("Note Deleted", "", "warning");
                }
            }
            MenuSection::Configs => {
                self.configs_module.delete(self.selected_index)?;
                self.status_message = "Config deleted".to_string();
                self.notifications_module
                    .push("Config Deleted", "", "warning");
            }
            _ => {}
        }
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
        self.cancel_confirm();
        Ok(())
    }

    pub fn cancel_confirm(&mut self) {
        self.state = AppState::Normal;
        self.confirm_message.clear();
    }

    pub fn input_char(&mut self, c: char) {
        self.input_buffer.insert(self.input_cursor, c);
        self.input_cursor += 1;
    }

    pub fn input_backspace(&mut self) {
        if self.input_cursor > 0 {
            self.input_buffer.remove(self.input_cursor - 1);
            self.input_cursor -= 1;
        }
    }

    pub fn input_move_left(&mut self) {
        if self.input_cursor > 0 {
            self.input_cursor -= 1;
        }
    }

    pub fn input_move_right(&mut self) {
        if self.input_cursor < self.input_buffer.len() {
            self.input_cursor += 1;
        }
    }

    pub async fn auto_refresh(&mut self) -> Result<()> {
        if self.pending_initial_scan {
            // Do initial heavy work after first frame to speed up startup
            self.apps_module.scan_path_executables().await?;
            self.apps_module.refresh_running_processes().await?;
            self.pending_initial_scan = false;
        }

        if self.last_refresh.elapsed() > Duration::from_secs(5) {
            if self.current_section == MenuSection::Dashboard
                || self.current_section == MenuSection::Apps
            {
                self.apps_module.refresh_running_processes().await?;
            }
            // Refresh SSH session status
            self.ssh_module.refresh_session_status();
            self.last_refresh = Instant::now();
        }
        Ok(())
    }

    fn get_current_list_len(&self) -> usize {
        match self.current_section {
            MenuSection::Dashboard => self.apps_module.running_processes.len(),
            MenuSection::Apps => {
                // Apps section shows both available apps and running processes
                self.apps_module.available_apps.len() + self.apps_module.running_processes.len()
            }
            MenuSection::Bookmarks => self.bookmarks_module.bookmarks.len(),
            MenuSection::Clipboard => self.clipboard_module.entries.len(),
            MenuSection::Configs => self.configs_module.configs.len(),
            MenuSection::Docker => match self.docker_module.current_view {
                crate::modules::docker::DockerView::Containers => {
                    self.docker_module.containers.len()
                }
                crate::modules::docker::DockerView::Images => self.docker_module.images.len(),
                _ => 0,
            },
            MenuSection::Network => match self.network_module.current_view {
                crate::modules::network::NetworkView::Connections => {
                    self.network_module.connections.len()
                }
                crate::modules::network::NetworkView::Interfaces => {
                    self.network_module.interfaces.len()
                }
                crate::modules::network::NetworkView::Ports => {
                    self.network_module.listening_ports.len()
                }
            },
            MenuSection::Ssh => self.ssh_module.hosts.len(),
            MenuSection::Scripts => self.scripts_module.scripts.len(),
            MenuSection::Notifications => self.notifications_module.notifications.len(),
            MenuSection::History => self.shell_module.entries.len(),
            MenuSection::Git => self.git_module.repos.len(),
            MenuSection::Scratchpad => self.scratchpad_module.notes.len(),
            MenuSection::Shell => 0, // Shell has its own UI, no list
            MenuSection::Services => self.services_module.services.len(),
        }
    }

    pub async fn schedule_selected_script(&mut self) -> Result<()> {
        if self.current_section == MenuSection::Scripts
            && self.selected_index < self.scripts_module.scripts.len() {
                self.scripts_module
                    .schedule_script(self.selected_index, 60)
                    .await?;
                let name = self.scripts_module.scripts[self.selected_index]
                    .name
                    .clone();
                self.status_message = format!("Scheduled: {} (every 60s)", name);
                self.notifications_module
                    .push("Script Scheduled", &name, "info");
            }
        Ok(())
    }

    pub fn disconnect_latest_session(&mut self) {
        if self.current_section == MenuSection::Ssh
            && !self.ssh_module.active_sessions.is_empty() {
                let idx = self.ssh_module.active_sessions.len() - 1;
                let name = self.ssh_module.active_sessions[idx].name.clone();
                match self.ssh_module.disconnect(idx) {
                    Ok(()) => {
                        self.status_message = format!("Disconnected: {}", name);
                        self.notifications_module
                            .push("SSH Disconnected", &name, "warning");
                        // Immediately refresh sessions so UI reflects removal
                        self.ssh_module.refresh_session_status();
                    }
                    Err(e) => {
                        self.status_message = format!("Failed to disconnect {}: {}", name, e);
                        self.notifications_module
                            .push("SSH Disconnect Failed", &name, "error");
                    }
                }
            }
    }

    pub fn report_error(&mut self, context: &str, err: anyhow::Error) {
        let msg = format!("{}: {}", context, err);
        self.status_message = msg.clone();
        self.notifications_module.push(context, msg, "error");
    }

    // Search mode
    pub fn open_search(&mut self) {
        self.state = AppState::Search;
        self.search_query.clear();
        self.search_cursor = 0;
        self.search_selected = 0;
        self.rebuild_search_results();
    }

    pub fn close_search(&mut self) {
        self.state = AppState::Normal;
    }

    pub fn submit_search(&mut self) {
        if self.search_results.is_empty() {
            self.close_search();
            return;
        }
        let sel = self.search_results[self.search_selected].clone();
        self.current_section = sel.section;
        self.selected_index = sel.index;
        self.status_message = format!("Jumped to: {}", sel.label);
        self.close_search();
    }

    pub fn search_input_char(&mut self, c: char) {
        self.search_query.insert(self.search_cursor, c);
        self.search_cursor += 1;
        self.rebuild_search_results();
    }
    pub fn search_backspace(&mut self) {
        if self.search_cursor > 0 {
            self.search_query.remove(self.search_cursor - 1);
            self.search_cursor -= 1;
            self.rebuild_search_results();
        }
    }
    pub fn search_move_left(&mut self) {
        if self.search_cursor > 0 {
            self.search_cursor -= 1;
        }
    }
    pub fn search_move_right(&mut self) {
        if self.search_cursor < self.search_query.len() {
            self.search_cursor += 1;
        }
    }
    pub fn search_next(&mut self) {
        if !self.search_results.is_empty() {
            self.search_selected = (self.search_selected + 1) % self.search_results.len();
        }
    }
    pub fn search_prev(&mut self) {
        if !self.search_results.is_empty() {
            if self.search_selected == 0 {
                self.search_selected = self.search_results.len() - 1;
            } else {
                self.search_selected -= 1;
            }
        }
    }
    pub fn search_page_up(&mut self) {
        if !self.search_results.is_empty() {
            self.search_selected = self.search_selected.saturating_sub(10);
        }
    }
    pub fn search_page_down(&mut self) {
        if !self.search_results.is_empty() {
            self.search_selected = usize::min(
                self.search_selected + 10,
                self.search_results.len().saturating_sub(1),
            );
        }
    }
    pub fn search_go_home(&mut self) {
        self.search_selected = 0;
    }
    pub fn search_go_end(&mut self) {
        if !self.search_results.is_empty() {
            self.search_selected = self.search_results.len() - 1;
        }
    }

    fn rebuild_search_results(&mut self) {
        let mut results: Vec<SearchResult> = Vec::new();
        match self.current_section {
            MenuSection::Dashboard => {
                for (i, p) in self.apps_module.running_processes.iter().enumerate() {
                    let label = format!("Process: {} (pid {})", p.name, p.pid);
                    if let Some(score) = score_match(&label, &self.search_query) {
                        results.push(SearchResult {
                            section: MenuSection::Dashboard,
                            index: i,
                            label,
                            score,
                        });
                    }
                }
            }
            MenuSection::Apps => {
                for (i, name) in self.apps_module.available_apps.iter().enumerate() {
                    if let Some(score) = score_match(name, &self.search_query) {
                        results.push(SearchResult {
                            section: MenuSection::Apps,
                            index: i,
                            label: format!("App: {}", name),
                            score,
                        });
                    }
                }
            }
            MenuSection::Bookmarks => {
                for (i, b) in self.bookmarks_module.bookmarks.iter().enumerate() {
                    let label = format!("Bookmark: {} {}", b.name, b.path);
                    if let Some(score) = score_match(&label, &self.search_query) {
                        results.push(SearchResult {
                            section: MenuSection::Bookmarks,
                            index: i,
                            label,
                            score,
                        });
                    }
                }
            }
            MenuSection::Clipboard => {
                for (i, e) in self.clipboard_module.entries.iter().enumerate() {
                    let label = format!("Clip: {} ({})", e.content, e.content_type);
                    if let Some(score) = score_match(&label, &self.search_query) {
                        results.push(SearchResult {
                            section: MenuSection::Clipboard,
                            index: i,
                            label: label.clone(),
                            score,
                        });
                    }
                }
            }
            MenuSection::Docker => {
                for (i, c) in self.docker_module.containers.iter().enumerate() {
                    let label = format!("Docker: {} - {} ({})", c.name, c.image, c.status);
                    if let Some(score) = score_match(&label, &self.search_query) {
                        results.push(SearchResult {
                            section: MenuSection::Docker,
                            index: i,
                            label,
                            score,
                        });
                    }
                }
            }
            MenuSection::Ssh => {
                for (i, h) in self.ssh_module.hosts.iter().enumerate() {
                    let mut target = h.host.clone();
                    if !h.user.is_empty() {
                        target = format!("{}@{}", h.user, h.host);
                    }
                    let label = format!("SSH: {} {}:{}", h.name, target, h.port);
                    if let Some(score) = score_match(&label, &self.search_query) {
                        results.push(SearchResult {
                            section: MenuSection::Ssh,
                            index: i,
                            label,
                            score,
                        });
                    }
                }
            }
            MenuSection::Scripts => {
                for (i, s) in self.scripts_module.scripts.iter().enumerate() {
                    let label = if s.description.is_empty() {
                        format!("Script: {} - {}", s.name, s.command)
                    } else {
                        format!("Script: {} - {}", s.name, s.description)
                    };
                    if let Some(score) = score_match(&label, &self.search_query) {
                        results.push(SearchResult {
                            section: MenuSection::Scripts,
                            index: i,
                            label,
                            score,
                        });
                    }
                }
            }
            MenuSection::Notifications => {
                for (i, n) in self.notifications_module.notifications.iter().enumerate() {
                    let label = format!("Notif: {} - {}", n.title, n.message);
                    if let Some(score) = score_match(&label, &self.search_query) {
                        results.push(SearchResult {
                            section: MenuSection::Notifications,
                            index: i,
                            label,
                            score,
                        });
                    }
                }
            }
            MenuSection::History => {
                for (i, e) in self.shell_module.entries.iter().enumerate() {
                    let label = format!("Hist: {}", e.command);
                    if let Some(score) = score_match(&label, &self.search_query) {
                        results.push(SearchResult {
                            section: MenuSection::History,
                            index: i,
                            label,
                            score,
                        });
                    }
                }
            }
            MenuSection::Git => {
                for (i, r) in self.git_module.repos.iter().enumerate() {
                    let label = format!("Git: {} ({}) - {}", r.name, r.branch, r.status);
                    if let Some(score) = score_match(&label, &self.search_query) {
                        results.push(SearchResult {
                            section: MenuSection::Git,
                            index: i,
                            label,
                            score,
                        });
                    }
                }
            }
            MenuSection::Network => {
                for (i, c) in self.network_module.connections.iter().enumerate() {
                    let label = format!(
                        "Net: {} {} -> {} ({})",
                        c.protocol, c.local_addr, c.remote_addr, c.state
                    );
                    if let Some(score) = score_match(&label, &self.search_query) {
                        results.push(SearchResult {
                            section: MenuSection::Network,
                            index: i,
                            label,
                            score,
                        });
                    }
                }
            }
            MenuSection::Scratchpad => {
                for (i, note) in self.scratchpad_module.notes.iter().enumerate() {
                    let label = format!(
                        "Note: {} ({})",
                        note.name,
                        note.modified_at.format("%Y-%m-%d %H:%M")
                    );
                    if let Some(score) = score_match(&label, &self.search_query) {
                        results.push(SearchResult {
                            section: MenuSection::Scratchpad,
                            index: i,
                            label,
                            score,
                        });
                    }
                }
            }
            MenuSection::Shell => {
                // Shell terminal doesn't have searchable items
            }
            MenuSection::Services => {
                for (i, svc) in self.services_module.services.iter().enumerate() {
                    let label = format!(
                        "Service: {} - {} ({})",
                        svc.name,
                        svc.display_name,
                        svc.state.as_str()
                    );
                    if let Some(score) = score_match(&label, &self.search_query) {
                        results.push(SearchResult {
                            section: MenuSection::Services,
                            index: i,
                            label,
                            score,
                        });
                    }
                }
            }
            MenuSection::Configs => {
                for (i, cfg) in self.configs_module.configs.iter().enumerate() {
                    let label = format!(
                        "Config: {} - {} ({})",
                        cfg.name,
                        cfg.path.display(),
                        cfg.category
                    );
                    if let Some(score) = score_match(&label, &self.search_query) {
                        results.push(SearchResult {
                            section: MenuSection::Configs,
                            index: i,
                            label,
                            score,
                        });
                    }
                }
            }
        }

        if self.search_query.is_empty() {
            for r in results.iter_mut() {
                r.score = 0;
            }
        }
        results.sort_by_key(|r| r.score);
        if results.len() > 500 {
            results.truncate(500);
        }
        self.search_results = results;
        self.search_selected = 0;
    }
}

fn score_match(candidate: &str, query: &str) -> Option<i32> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Some(0);
    }
    let c = candidate.to_lowercase();
    if let Some(idx) = c.find(&q) {
        return Some(idx as i32);
    }
    // Simple subsequence match with positional sum
    let mut qi = 0usize;
    let mut sum_pos = 0i32;
    let qb = q.as_bytes();
    for (i, ch) in c.chars().enumerate() {
        if qi < qb.len() && ch == qb[qi] as char {
            sum_pos += i as i32;
            qi += 1;
        }
    }
    if qi == qb.len() { Some(sum_pos) } else { None }
}

impl App {
    // Shell terminal methods
    pub fn open_shell_input(&mut self) {
        self.state = AppState::ShellInput;
        self.shell_input_buffer.clear();
        self.shell_input_cursor = 0;
    }

    pub fn shell_input_char(&mut self, c: char) {
        self.shell_input_buffer.insert(self.shell_input_cursor, c);
        self.shell_input_cursor += 1;
    }

    pub fn shell_input_backspace(&mut self) {
        if self.shell_input_cursor > 0 {
            self.shell_input_buffer.remove(self.shell_input_cursor - 1);
            self.shell_input_cursor -= 1;
        }
    }

    pub fn shell_input_move_left(&mut self) {
        if self.shell_input_cursor > 0 {
            self.shell_input_cursor -= 1;
        }
    }

    pub fn shell_input_move_right(&mut self) {
        if self.shell_input_cursor < self.shell_input_buffer.len() {
            self.shell_input_cursor += 1;
        }
    }

    pub fn close_shell_input(&mut self) {
        self.state = AppState::Normal;
        self.shell_input_buffer.clear();
        self.shell_input_cursor = 0;
    }

    pub async fn execute_shell_command(&mut self) -> Result<()> {
        let command = self.shell_input_buffer.clone();
        self.close_shell_input();

        if command.trim().is_empty() {
            return Ok(());
        }

        let result = self.shell_terminal_module.execute_command(&command).await?;

        if result.output.contains("[exit requested]") {
            self.status_message = "Shell exit requested".to_string();
        } else if result.output.contains("[cleared]") {
            self.status_message = "Shell cleared".to_string();
        } else {
            self.status_message = format!("Executed: {}", command);
        }

        Ok(())
    }

    // Scratchpad methods
    pub fn scratchpad_copy_to_clipboard(&mut self) -> Result<()> {
        if self.selected_index < self.scratchpad_module.notes.len() {
            let content = self
                .scratchpad_module
                .copy_to_clipboard(self.selected_index)?;
            self.status_message = format!("Copied {} bytes to clipboard", content.len());
            self.notifications_module
                .push("Scratchpad", "Copied to clipboard", "info");
        }
        Ok(())
    }

    pub fn scratchpad_export(&mut self) {
        if self.selected_index < self.scratchpad_module.notes.len() {
            self.state = AppState::Input;
            self.input_buffer.clear();
            self.input_cursor = 0;
            self.input_prompt = "Export note to path: ".to_string();
        }
    }

    pub fn scratchpad_rename(&mut self) {
        if self.selected_index < self.scratchpad_module.notes.len() {
            self.state = AppState::Input;
            let current_name = &self.scratchpad_module.notes[self.selected_index].name;
            self.input_buffer = current_name.clone();
            self.input_cursor = self.input_buffer.len();
            self.input_prompt = "Rename note to: ".to_string();
        }
    }

    pub fn scratchpad_search(&mut self) {
        self.state = AppState::Input;
        self.input_buffer.clear();
        self.input_cursor = 0;
        self.input_prompt = "Search notes: ".to_string();
    }

    pub fn execute_scratchpad_action(&mut self, input: String) -> Result<()> {
        if self.input_prompt.starts_with("Rename") {
            // Rename action
            if let Err(e) = self.scratchpad_module.rename(self.selected_index, &input) {
                self.status_message = format!("Failed to rename: {}", e);
            } else {
                self.status_message = format!("Renamed to: {}", input);
                self.notifications_module.push(
                    "Scratchpad",
                    format!("Renamed to {}", input),
                    "info",
                );
            }
        } else if self.input_prompt.starts_with("Search") {
            // Search action
            self.scratchpad_search_query = input.clone();
            self.scratchpad_search_results = self.scratchpad_module.search(&input);
            self.status_message = format!("Found {} matches", self.scratchpad_search_results.len());
            if !self.scratchpad_search_results.is_empty() {
                self.selected_index = self.scratchpad_search_results[0];
            }
        } else if self.input_prompt.starts_with("Export") {
            // Export action
            let dest_path = std::path::PathBuf::from(input.clone());
            if let Err(e) = self
                .scratchpad_module
                .export_to_path(self.selected_index, &dest_path)
            {
                self.status_message = format!("Failed to export: {}", e);
            } else {
                self.status_message = format!("Exported to: {}", input);
                self.notifications_module.push(
                    "Scratchpad",
                    format!("Exported to {}", input),
                    "info",
                );
            }
        }
        Ok(())
    }

    // Shell history methods
    pub fn shell_search_history(&mut self) {
        self.state = AppState::Input;
        self.input_buffer.clear();
        self.input_cursor = 0;
        self.input_prompt = "Search shell history: ".to_string();
    }

    pub fn shell_clear_history(&mut self) {
        self.shell_terminal_module.clear_history();
        self.status_message = "Shell history cleared".to_string();
        self.notifications_module
            .push("Shell", "History cleared", "info");
    }

    pub fn execute_shell_action(&mut self, input: String) -> Result<()> {
        if self.input_prompt.starts_with("Search shell") {
            let results = self.shell_terminal_module.search_history(&input);
            self.status_message =
                format!("Found {} commands matching \"{}\"", results.len(), input);
            if !results.is_empty() {
                self.notifications_module.push(
                    "Shell",
                    format!("Found {} matches", results.len()),
                    "info",
                );
            }
        }
        Ok(())
    }

    // Configs helper methods
    pub fn backup_selected_config(&mut self) -> Result<()> {
        if self.current_section == MenuSection::Configs
            && let Ok(backup_path) = self.configs_module.backup_config(self.selected_index) {
                self.status_message = format!("Backed up to: {}", backup_path);
                self.notifications_module
                    .push("Config", "Backup created", "info");
            }
        Ok(())
    }

    pub fn view_selected_config(&mut self) -> Result<()> {
        if self.current_section == MenuSection::Configs {
            let config = &self.configs_module.configs[self.selected_index];
            if !config.exists {
                self.status_message = "File not found - cannot preview".to_string();
                return Ok(());
            }
            
            if let Ok(content) = self.configs_module.view_config(self.selected_index) {
                let total = content.lines().count();
                self.status_message = format!("Preview: {} lines", total);
                // Store the preview content for the info panel
                self.configs_module.set_preview_content(content);
            }
        }
        Ok(())
    }

    pub fn config_copy_to_clipboard(&mut self) -> Result<()> {
        if self.current_section == MenuSection::Configs
            && let Ok(content) = self.configs_module.copy_to_clipboard(self.selected_index) {
                self.status_message = format!("Copied {} bytes to clipboard", content.len());
                self.notifications_module
                    .push("Config", "Copied to clipboard", "info");
            }
        Ok(())
    }

    pub fn open_config_in_file_manager(&mut self) -> Result<()> {
        if self.current_section == MenuSection::Configs
            && let Err(e) = self
                .configs_module
                .open_in_file_manager(self.selected_index)
            {
                self.report_error("Open folder failed", e);
            }
        Ok(())
    }

    pub fn search_configs(&mut self) {
        if self.current_section == MenuSection::Configs {
            self.state = AppState::Input;
            self.input_buffer.clear();
            self.input_cursor = 0;
            self.input_prompt = "Search configs: ".to_string();
        }
    }

    pub fn execute_configs_search(&mut self, query: String) {
        let results = self.configs_module.search(&query);
        self.status_message = format!("Found {} configs matching \"{}\"", results.len(), query);
        if !results.is_empty() {
            self.selected_index = results[0];
            self.notifications_module.push(
                "Configs",
                format!("Found {} matches", results.len()),
                "info",
            );
        }
    }
}
