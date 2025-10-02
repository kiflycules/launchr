use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
};

use crate::app::{App, AppState, MenuSection};
use crate::modules::configs::ConfigsModule;

pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(f.area());

    draw_title(f, chunks[0], app);

    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(20), Constraint::Percentage(80)])
        .split(chunks[1]);

    draw_menu(f, app, main_chunks[0]);

    match app.current_section {
        MenuSection::Dashboard => draw_dashboard(f, app, main_chunks[1]),
        MenuSection::Apps => draw_apps(f, app, main_chunks[1]),
        MenuSection::Bookmarks => draw_bookmarks(f, app, main_chunks[1]),
        MenuSection::Clipboard => draw_clipboard(f, app, main_chunks[1]),
        MenuSection::Configs => draw_configs(f, app, main_chunks[1]),
        MenuSection::Docker => draw_docker(f, app, main_chunks[1]),
        MenuSection::Network => draw_network(f, app, main_chunks[1]),
        MenuSection::Ssh => draw_ssh(f, app, main_chunks[1]),
        MenuSection::Scripts => draw_scripts(f, app, main_chunks[1]),
        MenuSection::Notifications => draw_notifications(f, app, main_chunks[1]),
        MenuSection::History => draw_history(f, app, main_chunks[1]),
        MenuSection::Git => draw_git(f, app, main_chunks[1]),
        MenuSection::Scratchpad => draw_scratchpad(f, app, main_chunks[1]),
        MenuSection::Shell => draw_shell(f, app, main_chunks[1]),
        MenuSection::Services => draw_services(f, app, main_chunks[1]),
    }

    draw_status(f, app, chunks[2]);

    if app.state == AppState::Input {
        draw_input_popup(f, app);
    } else if app.state == AppState::Confirm {
        draw_confirm_popup(f, app);
    } else if app.show_help {
        draw_help_popup(f);
    } else if app.state == AppState::Search {
        draw_search_popup(f, app);
    } else if app.state == AppState::ShellInput {
        draw_shell_input_popup(f, app);
    }
}

fn draw_title(f: &mut Frame, area: Rect, app: &App) {
    // Collect header info: user, time, arch, CPU, GPU (best-effort), network
    let username = std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| "unknown".to_string());
    let time_str = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let arch = std::env::consts::ARCH;

    // CPU snapshot from sysinfo via apps module
    let (cores, cpu_avg) = app.apps_module.snapshot_cpu_metrics();

    // Detected shell label
    let shell_label = match app.shell_module.detected_shell {
        crate::modules::history::ShellKind::PowerShell => "pwsh",
        crate::modules::history::ShellKind::Bash => "bash",
        crate::modules::history::ShellKind::Zsh => "zsh",
        crate::modules::history::ShellKind::Fish => "fish",
        crate::modules::history::ShellKind::Unknown => "shell",
    };

    let mut header = format!(
        "{} | {} | {} | shell: {}",
        username, time_str, arch, shell_label
    );
    if cores > 0 {
        header.push_str(&format!(" | CPU: {}c {:.0}%", cores, cpu_avg));
    }

    let title = Paragraph::new(header)
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, area);
}

fn draw_menu(f: &mut Frame, app: &App, area: Rect) {
    let menu_items = vec![
        ("1", "Dashboard", MenuSection::Dashboard),
        ("2", "Apps", MenuSection::Apps),
        ("3", "Bookmarks", MenuSection::Bookmarks),
        ("4", "Clipboard", MenuSection::Clipboard),
        ("5", "Docker", MenuSection::Docker),
        ("6", "Network", MenuSection::Network),
        ("7", "SSH", MenuSection::Ssh),
        ("8", "Scripts", MenuSection::Scripts),
        ("9", "Git", MenuSection::Git),
        ("0", "History", MenuSection::History),
        ("\\", "Configs", MenuSection::Configs),
        ("-", "Scratchpad", MenuSection::Scratchpad),
        ("=", "Shell", MenuSection::Shell),
        ("]", "Services", MenuSection::Services),
        ("[", "Notifications", MenuSection::Notifications),
    ];

    let items: Vec<ListItem> = menu_items
        .iter()
        .map(|(key, name, section)| {
            let style = if *section == app.current_section {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(format!("{:>2} {}", key, name)).style(style)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .title("Menu (Tab/Shift+Tab)")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::White)),
    );
    f.render_widget(list, area);
}

fn draw_dashboard(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(40),
            Constraint::Percentage(30),
            Constraint::Percentage(30),
        ])
        .split(area);

    // Loading/empty states
    if app.pending_initial_scan {
        let loading = Paragraph::new("Loading processes and apps...")
            .block(Block::default().title("Loading").borders(Borders::ALL));
        f.render_widget(loading, chunks[0]);
    }

    let list_height = chunks[0].height.saturating_sub(2) as usize;
    let start = app.selected_index.saturating_sub(list_height / 2);
    let end = usize::min(start + list_height, app.apps_module.running_processes.len());

    let running_top: Vec<ListItem> = app
        .apps_module
        .running_processes
        .iter()
        .enumerate()
        .skip(start)
        .take(end.saturating_sub(start))
        .map(|(i, p)| {
            let style = if i == app.selected_index {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(format!(
                "{:<6} {:<20} CPU: {:.1}% MEM: {:.1} MB",
                p.pid, p.name, p.cpu_usage, p.memory_usage
            ))
            .style(style)
        })
        .collect();

    let running_block = List::new(running_top).block(
        Block::default()
            .title("Running Processes (↑/↓ PgUp/PgDn Home/End; '/' to search)")
            .borders(Borders::ALL),
    );
    f.render_widget(running_block, chunks[0]);

    let ssh_sessions: Vec<ListItem> = app
        .ssh_module
        .active_sessions
        .iter()
        .map(|s| {
            let status_icon = if s.status == "Connected" {
                "🟢"
            } else {
                "🔴"
            };
            ListItem::new(format!("{} {} - {}", status_icon, s.name, s.status))
        })
        .collect();

    let ssh_block = List::new(ssh_sessions).block(
        Block::default()
            .title("Active SSH Sessions")
            .borders(Borders::ALL),
    );
    if app.ssh_module.active_sessions.is_empty() {
        let empty = Paragraph::new("No active SSH sessions").block(
            Block::default()
                .title("Active SSH Sessions")
                .borders(Borders::ALL),
        );
        f.render_widget(empty, chunks[1]);
    } else {
        f.render_widget(ssh_block, chunks[1]);
    }

    let notifs: Vec<ListItem> = app
        .notifications_module
        .notifications
        .iter()
        .take(5)
        .map(|n| {
            ListItem::new(format!(
                "[{}] {} - {}",
                n.timestamp.format("%H:%M:%S"),
                n.title,
                n.message
            ))
        })
        .collect();

    let notif_block = List::new(notifs).block(
        Block::default()
            .title("Recent Notifications")
            .borders(Borders::ALL),
    );
    if app.notifications_module.notifications.is_empty() {
        let empty = Paragraph::new("No notifications yet").block(
            Block::default()
                .title("Recent Notifications")
                .borders(Borders::ALL),
        );
        f.render_widget(empty, chunks[2]);
    } else {
        f.render_widget(notif_block, chunks[2]);
    }
}

fn draw_apps(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // Available Apps section
    let available_apps = &app.apps_module.available_apps;
    let window_height = (chunks[0].height.saturating_sub(2)) as usize;
    let available_len = available_apps.len();
    let selected_in_apps = if app.selected_index < available_len {
        app.selected_index
    } else {
        available_len.saturating_sub(1)
    };
    let start = if available_len == 0 {
        0
    } else {
        let half = window_height / 2;
        let base = selected_in_apps.saturating_sub(half);
        let max_start = available_len.saturating_sub(window_height);
        usize::min(base, max_start)
    };
    let end = usize::min(start + window_height, available_len);

    let items: Vec<ListItem> = available_apps
        .get(start..end)
        .unwrap_or(&[])
        .iter()
        .enumerate()
        .map(|(offset, name)| {
            let i = start + offset;
            let style = if i == app.selected_index {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(format!("▶ {}", name)).style(style)
        })
        .collect();

    if available_apps.is_empty() {
        let empty = if app.pending_initial_scan {
            "Scanning PATH..."
        } else {
            "No executables found in PATH"
        };
        let p = Paragraph::new(empty).block(
            Block::default()
                .title("Available Apps")
                .borders(Borders::ALL),
        );
        f.render_widget(p, chunks[0]);
    } else {
        let apps_list = List::new(items).block(
            Block::default()
                .title("Available Apps (Enter to launch)")
                .borders(Borders::ALL),
        );
        f.render_widget(apps_list, chunks[0]);
    }

    // Running Processes section
    let running_processes = &app.apps_module.running_processes;
    let process_window_height = (chunks[1].height.saturating_sub(2)) as usize;
    let proc_len = running_processes.len();
    let process_selection = app.selected_index.saturating_sub(available_len);
    let process_start = if proc_len == 0 {
        0
    } else {
        let half = process_window_height / 2;
        let base = process_selection.saturating_sub(half);
        let max_start = proc_len.saturating_sub(process_window_height);
        usize::min(base, max_start)
    };
    let process_end = usize::min(process_start + process_window_height, proc_len);

    let running: Vec<ListItem> = running_processes
        .get(process_start..process_end)
        .unwrap_or(&[])
        .iter()
        .enumerate()
        .map(|(offset, p)| {
            let i = available_len + process_start + offset;
            let style = if i == app.selected_index {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(format!(
                "{:<6} {:<20} CPU: {:.1}% MEM: {:.1} MB",
                p.pid, p.name, p.cpu_usage, p.memory_usage
            ))
            .style(style)
        })
        .collect();

    let running_block = List::new(running).block(
        Block::default()
            .title("Running Processes (s to stop)")
            .borders(Borders::ALL),
    );
    f.render_widget(running_block, chunks[1]);
}

fn draw_bookmarks(f: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .bookmarks_module
        .bookmarks
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let style = if i == app.selected_index {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            let icon = match b.bookmark_type.as_str() {
                "directory" => "📁",
                "url" => "🌐",
                _ => "📄",
            };
            ListItem::new(format!("{} {} → {}", icon, b.name, b.path)).style(style)
        })
        .collect();

    if items.is_empty() {
        let empty = Paragraph::new("No bookmarks. Press 'n' to add")
            .block(Block::default().title("Bookmarks").borders(Borders::ALL));
        f.render_widget(empty, area);
    } else {
        let list = List::new(items).block(
            Block::default()
                .title("Bookmarks (n: new, d: delete, Enter: open)")
                .borders(Borders::ALL),
        );
        f.render_widget(list, area);
    }
}

fn draw_clipboard(f: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .clipboard_module
        .entries
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let style = if i == app.selected_index {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            let icon = match e.content_type.as_str() {
                "text" => "📝",
                "command" => "⚡",
                "url" => "🔗",
                _ => "📋",
            };
            let pinned_icon = if e.pinned { "📌 " } else { "" };
            let truncated_content = if e.content.len() > 50 {
                format!("{}...", &e.content[..47])
            } else {
                e.content.clone()
            };
            ListItem::new(format!(
                "{} {}{} ({})",
                icon,
                pinned_icon,
                truncated_content,
                e.timestamp.format("%H:%M:%S")
            ))
            .style(style)
        })
        .collect();

    if items.is_empty() {
        let empty = Paragraph::new("No clipboard entries. Press 'r' to refresh")
            .block(Block::default().title("Clipboard").borders(Borders::ALL));
        f.render_widget(empty, area);
    } else {
        let list = List::new(items).block(
            Block::default()
                .title("Clipboard (Enter: copy, r: refresh, p: pin)")
                .borders(Borders::ALL),
        );
        f.render_widget(list, area);
    }
}

fn draw_docker(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(80), Constraint::Percentage(20)])
        .split(area);

    let items = match app.docker_module.current_view {
        crate::modules::docker::DockerView::Containers => {
            let docker_items: Vec<ListItem> = app
                .docker_module
                .containers
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    let style = if i == app.selected_index {
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    };
                    let status_icon = if c.status.contains("Up") {
                        "🟢"
                    } else if c.status.contains("Exited") {
                        "🔴"
                    } else {
                        "🟡"
                    };
                    let truncated_name = if c.name.len() > 20 {
                        format!("{}...", &c.name[..17])
                    } else {
                        c.name.clone()
                    };
                    ListItem::new(format!(
                        "{} {} - {} ({})",
                        status_icon, truncated_name, c.image, c.status
                    ))
                    .style(style)
                })
                .collect();

            if docker_items.is_empty() {
                vec![ListItem::new("No containers found. Press 'r' to refresh")]
            } else {
                docker_items
            }
        }
        crate::modules::docker::DockerView::Images => {
            let docker_items: Vec<ListItem> = app
                .docker_module
                .images
                .iter()
                .enumerate()
                .map(|(i, img)| {
                    let style = if i == app.selected_index {
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    };
                    let truncated_repo = if img.repository.len() > 25 {
                        format!("{}...", &img.repository[..22])
                    } else {
                        img.repository.clone()
                    };
                    ListItem::new(format!("📦 {}:{} - {}", truncated_repo, img.tag, img.size))
                        .style(style)
                })
                .collect();

            if docker_items.is_empty() {
                vec![ListItem::new("No images found. Press 'r' to refresh")]
            } else {
                docker_items
            }
        }
        _ => vec![ListItem::new("View not implemented yet")],
    };

    let list = List::new(items).block(
        Block::default()
            .title("Docker (Enter: exec, r: refresh, v: switch view)")
            .borders(Borders::ALL),
    );
    f.render_widget(list, chunks[0]);

    // Status bar showing current view and options
    let status_text = match app.docker_module.current_view {
        crate::modules::docker::DockerView::Containers => {
            if app.docker_module.show_all {
                "View: Containers (all) | Press 'a' to show running only"
            } else {
                "View: Containers (running) | Press 'a' to show all"
            }
        }
        crate::modules::docker::DockerView::Images => "View: Images",
        _ => "View: Other",
    };

    let status = Paragraph::new(status_text).block(Block::default().borders(Borders::ALL));
    f.render_widget(status, chunks[1]);
}

fn draw_ssh(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    let items: Vec<ListItem> = app
        .ssh_module
        .hosts
        .iter()
        .enumerate()
        .map(|(i, h)| {
            let style = if i == app.selected_index {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(format!("🖥️  {} ({}@{}:{})", h.name, h.user, h.host, h.port)).style(style)
        })
        .collect();

    if items.is_empty() {
        let empty = Paragraph::new("No SSH hosts. Press 'n' to add")
            .block(Block::default().title("SSH Hosts").borders(Borders::ALL));
        f.render_widget(empty, chunks[0]);
    } else {
        let list = List::new(items).block(
            Block::default()
                .title("SSH Hosts (n: new, d: delete, Enter: connect)")
                .borders(Borders::ALL),
        );
        f.render_widget(list, chunks[0]);
    }

    let sessions: Vec<ListItem> = app
        .ssh_module
        .active_sessions
        .iter()
        .map(|s| {
            let status_color = if s.status == "Connected" {
                Color::Green
            } else {
                Color::Red
            };
            let status_icon = if s.status == "Connected" {
                "🟢"
            } else {
                "🔴"
            };
            ListItem::new(Line::from(vec![
                Span::raw(status_icon),
                Span::raw(" "),
                Span::styled(s.name.clone(), Style::default().fg(status_color)),
                Span::raw(format!(
                    " - {} @ {} (since {})",
                    s.status,
                    s.host,
                    s.connected_at.format("%H:%M:%S")
                )),
            ]))
        })
        .collect();

    let sessions_block = List::new(sessions).block(
        Block::default()
            .title("Active Sessions")
            .borders(Borders::ALL),
    );
    f.render_widget(sessions_block, chunks[1]);
}

fn draw_scripts(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(area);

    let items: Vec<ListItem> = app
        .scripts_module
        .scripts
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let style = if i == app.selected_index {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            let status = if app.scripts_module.is_running(i) {
                "🔄"
            } else {
                "▶"
            };
            ListItem::new(format!("{} {} - {}", status, s.name, s.description)).style(style)
        })
        .collect();

    if items.is_empty() {
        let empty = Paragraph::new("No scripts. Press 'n' to add")
            .block(Block::default().title("Scripts").borders(Borders::ALL));
        f.render_widget(empty, chunks[0]);
    } else {
        let list = List::new(items).block(
            Block::default()
                .title("Scripts (n: new, d: delete, Enter: run)")
                .borders(Borders::ALL),
        );
        f.render_widget(list, chunks[0]);
    }

    if app.show_detail && app.selected_index < app.scripts_module.scripts.len() {
        let script = &app.scripts_module.scripts[app.selected_index];
        let detail = Paragraph::new(format!(
            "Name: {}\nCommand: {}\nDescription: {}",
            script.name, script.command, script.description
        ))
        .block(
            Block::default()
                .title("Details (t to toggle)")
                .borders(Borders::ALL),
        );
        f.render_widget(detail, chunks[1]);
    } else {
        let help = Paragraph::new("Press 't' to toggle details")
            .block(Block::default().title("Details").borders(Borders::ALL));
        f.render_widget(help, chunks[1]);
    }
}

fn draw_notifications(f: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .notifications_module
        .notifications
        .iter()
        .enumerate()
        .map(|(i, n)| {
            let style = if i == app.selected_index {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            let level_icon = match n.level.as_str() {
                "error" => "❌",
                "warning" => "⚠️",
                "info" => "ℹ️",
                _ => "📢",
            };
            ListItem::new(format!(
                "{} [{}] {} - {}",
                level_icon,
                n.timestamp.format("%H:%M:%S"),
                n.title,
                n.message
            ))
            .style(style)
        })
        .collect();

    if items.is_empty() {
        let empty = Paragraph::new("No notifications yet").block(
            Block::default()
                .title("Notifications")
                .borders(Borders::ALL),
        );
        f.render_widget(empty, area);
    } else {
        let list = List::new(items).block(
            Block::default()
                .title("Notifications")
                .borders(Borders::ALL),
        );
        f.render_widget(list, area);
    }
}

fn draw_history(f: &mut Frame, app: &App, area: Rect) {
    let window_height = area.height.saturating_sub(2) as usize;
    let total = app.shell_module.entries.len();
    let start = app.selected_index.saturating_sub(window_height / 2);
    let end = usize::min(start + window_height, total);
    let any_ts = app
        .shell_module
        .entries
        .iter()
        .any(|e| e.timestamp.is_some());

    let items: Vec<ListItem> = app.shell_module.entries[start..end]
        .iter()
        .enumerate()
        .map(|(offset, e)| {
            let i = start + offset;
            let style = if i == app.selected_index {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            let ts = e.timestamp.map(|t| t.format("%H:%M").to_string());
            let label = match (any_ts, ts) {
                (_, Some(ts)) => format!("[{}] {}", ts, e.command),
                (false, None) => format!("{:>4}. {}", i + 1, e.command),
                (true, None) => e.command.clone(),
            };
            ListItem::new(label).style(style)
        })
        .collect();

    if total == 0 {
        let empty = ratatui::widgets::Paragraph::new("No shell history detected").block(
            Block::default()
                .title("History (Enter to run, r to refresh)")
                .borders(Borders::ALL),
        );
        f.render_widget(empty, area);
    } else {
        let list = List::new(items).block(
            Block::default()
                .title("History (Enter to run, r to refresh)")
                .borders(Borders::ALL),
        );
        f.render_widget(list, area);
    }
}

fn draw_git(f: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .git_module
        .repos
        .iter()
        .enumerate()
        .map(|(i, repo)| {
            let style = if i == app.selected_index {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let status_icon = match repo.status.as_str() {
                "clean" => "✓",
                "modified" => "✎",
                "ahead" => "↑",
                "behind" => "↓",
                _ => "•",
            };

            let status_color = match repo.status.as_str() {
                "clean" => Color::Green,
                "modified" => Color::Yellow,
                "ahead" => Color::Cyan,
                "behind" => Color::Magenta,
                _ => Color::White,
            };

            let mut parts = vec![
                Span::styled(status_icon, Style::default().fg(status_color)),
                Span::raw(" "),
                Span::raw(format!("{:<25}", repo.name)),
                Span::styled(
                    format!(" [{}] ", repo.branch),
                    Style::default().fg(Color::Cyan),
                ),
            ];

            if repo.uncommitted_changes > 0 {
                parts.push(Span::styled(
                    format!("±{} ", repo.uncommitted_changes),
                    Style::default().fg(Color::Yellow),
                ));
            }

            if repo.ahead > 0 {
                parts.push(Span::styled(
                    format!("↑{} ", repo.ahead),
                    Style::default().fg(Color::Green),
                ));
            }

            if repo.behind > 0 {
                parts.push(Span::styled(
                    format!("↓{} ", repo.behind),
                    Style::default().fg(Color::Red),
                ));
            }

            ListItem::new(Line::from(parts)).style(style)
        })
        .collect();

    if items.is_empty() {
        let empty = Paragraph::new("No git repositories found. Press 'S' to scan for repositories")
            .block(
                Block::default()
                    .title("Git Repositories")
                    .borders(Borders::ALL),
            );
        f.render_widget(empty, area);
    } else {
        let list = List::new(items).block(
            Block::default()
                .title("Git Repositories (Enter: open, r: refresh, S: scan)")
                .borders(Borders::ALL),
        );
        f.render_widget(list, area);
    }
}

fn draw_network(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(85), Constraint::Percentage(15)])
        .split(area);

    let items = match app.network_module.current_view {
        crate::modules::network::NetworkView::Connections => {
            let net_items: Vec<ListItem> = app
                .network_module
                .connections
                .iter()
                .enumerate()
                .map(|(i, conn)| {
                    let style = if i == app.selected_index {
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    };

                    let state_color = match conn.state.to_uppercase().as_str() {
                        "ESTABLISHED" | "ESTAB" => Color::Green,
                        "LISTEN" => Color::Cyan,
                        "TIME_WAIT" | "CLOSE_WAIT" => Color::Yellow,
                        _ => Color::White,
                    };

                    let truncated_local = if conn.local_addr.len() > 22 {
                        format!("{}...", &conn.local_addr[..19])
                    } else {
                        format!("{:<22}", conn.local_addr)
                    };

                    let truncated_remote = if conn.remote_addr.len() > 22 {
                        format!("{}...", &conn.remote_addr[..19])
                    } else {
                        format!("{:<22}", conn.remote_addr)
                    };

                    ListItem::new(Line::from(vec![
                        Span::raw(format!("{:<6} ", conn.protocol)),
                        Span::raw(format!("{} → {} ", truncated_local, truncated_remote)),
                        Span::styled(
                            format!("{:<12}", conn.state),
                            Style::default().fg(state_color),
                        ),
                        Span::raw(format!(" {}", conn.process_name)),
                    ]))
                    .style(style)
                })
                .collect();

            if net_items.is_empty() {
                vec![ListItem::new("No connections found. Press 'r' to refresh")]
            } else {
                net_items
            }
        }
        crate::modules::network::NetworkView::Interfaces => {
            let net_items: Vec<ListItem> = app
                .network_module
                .interfaces
                .iter()
                .enumerate()
                .map(|(i, iface)| {
                    let style = if i == app.selected_index {
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    };

                    let status_icon = if iface.status == "up" || iface.status.to_uppercase() == "UP"
                    {
                        "🟢"
                    } else {
                        "🔴"
                    };
                    let ips = if iface.ip_addresses.is_empty() {
                        String::from("no IP")
                    } else {
                        iface.ip_addresses.join(", ")
                    };

                    ListItem::new(format!(
                        "{} {:<15} {} ({})",
                        status_icon, iface.name, ips, iface.mac_address
                    ))
                    .style(style)
                })
                .collect();

            if net_items.is_empty() {
                vec![ListItem::new("No interfaces found. Press 'r' to refresh")]
            } else {
                net_items
            }
        }
        crate::modules::network::NetworkView::Ports => {
            let net_items: Vec<ListItem> = app
                .network_module
                .listening_ports
                .iter()
                .enumerate()
                .map(|(i, port)| {
                    let style = if i == app.selected_index {
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    };

                    ListItem::new(format!(
                        "Port {:<6} ({:<4}) - {} (PID: {})",
                        port.port, port.protocol, port.process_name, port.pid
                    ))
                    .style(style)
                })
                .collect();

            if net_items.is_empty() {
                vec![ListItem::new(
                    "No listening ports found. Press 'r' to refresh",
                )]
            } else {
                net_items
            }
        }
    };

    let title = match app.network_module.current_view {
        crate::modules::network::NetworkView::Connections => {
            if let Some(ref filter) = app.network_module.filter_state {
                format!(
                    "Network Connections [Filter: {}] (v: switch view, r: refresh, f: filter)",
                    filter
                )
            } else {
                "Network Connections (v: switch view, r: refresh, f: filter)".to_string()
            }
        }
        crate::modules::network::NetworkView::Interfaces => {
            "Network Interfaces (v: switch view, r: refresh)".to_string()
        }
        crate::modules::network::NetworkView::Ports => {
            "Listening Ports (v: switch view, r: refresh)".to_string()
        }
    };

    let list = List::new(items).block(Block::default().title(title).borders(Borders::ALL));
    f.render_widget(list, chunks[0]);

    // Status bar showing current view
    let view_name = match app.network_module.current_view {
        crate::modules::network::NetworkView::Connections => "Connections",
        crate::modules::network::NetworkView::Interfaces => "Interfaces",
        crate::modules::network::NetworkView::Ports => "Listening Ports",
    };

    let status_text = format!("View: {} | Press 'v' to switch view", view_name);
    let status = Paragraph::new(status_text).block(Block::default().borders(Borders::ALL));
    f.render_widget(status, chunks[1]);
}

fn draw_status(f: &mut Frame, app: &App, area: Rect) {
    let help_text = match app.state {
        AppState::Normal => {
            if app.current_section == MenuSection::Shell {
                "q: Quit | i: Input | h: Search History | C: Clear History | Built-ins: cd,pwd,history + External commands"
            } else if app.current_section == MenuSection::Scratchpad {
                "q: Quit | n: New | c: Copy | e: Export | R: Rename | f: Search | Enter: Open | d: Delete"
            } else if app.current_section == MenuSection::Services {
                "q: Quit | s: Start | S: Stop | R: Restart | E: Enable | D: Disable | l: Logs | u: User/System | f: Search | r: Refresh"
            } else if app.current_section == MenuSection::Configs {
                "q: Quit | n: New | d: Delete | Enter: Open | b: Backup | v: View | c: Copy | o: Open Folder | f: Search | r: Refresh | t: Toggle"
            } else {
                "q: Quit | Tab/Shift+Tab: Next/Prev Section | ↑↓/jk: Navigate | Enter: Select | n: New | d: Delete | r: Refresh | s: Stop Process | x: Disconnect SSH | t: Toggle"
            }
        }
        AppState::Input => "Enter: Submit | Esc: Cancel | Type your input",
        AppState::Confirm => "y: Yes | n: No | Esc: Cancel",
        AppState::Search => {
            "/: Search | Enter: Jump | Esc: Close | ↑↓/PgUp/PgDn/Home/End: Navigate"
        }
        AppState::ShellInput => "Enter: Execute | Esc: Cancel | Type shell command",
    };

    let status = Paragraph::new(vec![
        Line::from(app.status_message.as_str()),
        Line::from(help_text),
    ])
    .block(Block::default().borders(Borders::ALL));

    f.render_widget(status, area);
}

fn draw_input_popup(f: &mut Frame, app: &App) {
    let area = centered_rect(60, 20, f.area());
    let input_text = format!("{}{}", app.input_prompt, app.input_buffer);
    let input = Paragraph::new(input_text)
        .block(
            Block::default()
                .title("Input")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(ratatui::widgets::Clear, area);
    f.render_widget(input, area);
}

fn draw_confirm_popup(f: &mut Frame, app: &App) {
    let area = centered_rect(50, 15, f.area());
    let confirm = Paragraph::new(app.confirm_message.as_str())
        .block(
            Block::default()
                .title("Confirm")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Red)),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(ratatui::widgets::Clear, area);
    f.render_widget(confirm, area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn draw_help_popup(f: &mut Frame) {
    let area = centered_rect(70, 75, f.area());
    let help = "launchr Help\n\nNavigation:\n  q: Quit  |  ?: Toggle this help\n  Tab/Shift+Tab: Next/Previous Section\n  1-9,0,-,=,\\,[,]: Jump to section\n    1=Dashboard  2=Apps  3=Bookmarks  4=Clipboard  \\=Configs  5=Docker  6=Network\n    7=SSH  8=Scripts  9=Git  0=History  -=Scratchpad  ==Shell  ]=Services  [=Notifications\n  j/k or ↑/↓: Navigate items  |  PgUp/PgDn, Home/End: Jump\n  /: Fuzzy search (type to filter, Enter to jump, Esc to close)\n\nGeneral Actions:\n  Enter: Activate/Open  |  n: New  |  d: Delete  |  r: Refresh  |  t: Toggle details\n\nSection-Specific:\n  Dashboard/Apps: s=stop process\n  Configs: b=backup, v=view, c=copy, o=open folder, f=search\n  Docker/Network: v=switch view  |  Network: f=filter\n  SSH: x=disconnect latest\n  Scripts: S=schedule  |  Git: S=scan repos\n  Scratchpad: c=copy, e=export, R=rename, f=search\n  Shell: i=input command, h=search history, C=clear history\n\nSections:\n  Dashboard: Running processes  |  Apps: Launch apps\n  Bookmarks: Quick links  |  Clipboard: Clipboard history\n  Configs: Configuration files  |  Docker: Containers/Images\n  Network: Connections/Interfaces/Ports  |  SSH: Remote connections\n  Scripts: Run commands  |  Git: Repository status\n  History: Shell command history  |  Scratchpad: Quick notes\n  Shell: Embedded terminal  |  Services: System services\n  Notifications: System messages";

    let paragraph = Paragraph::new(help)
        .block(
            Block::default()
                .title("Help")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .wrap(Wrap { trim: false });

    f.render_widget(ratatui::widgets::Clear, area);
    f.render_widget(paragraph, area);
}

fn draw_search_popup(f: &mut Frame, app: &App) {
    // Keep width, increase height so input is clearly visible
    let area = centered_rect(70, 80, f.area());

    // Outer chrome
    f.render_widget(ratatui::widgets::Clear, area);
    let outer = Block::default().title("Search").borders(Borders::ALL);
    f.render_widget(outer.clone(), area);

    // Inner content area (inside borders)
    let inner_area = Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    };

    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // header line
            Constraint::Length(3), // input box height (taller so it's obvious)
            Constraint::Min(0),    // results list
        ])
        .split(inner_area);

    let header =
        Paragraph::new("Type to filter, Enter to jump, Esc to close").block(Block::default());
    f.render_widget(header, inner[0]);

    // Input box: show "/ " prefix and the full query; no wrapping truncation problems
    let input = Paragraph::new(format!("/ {}", app.search_query))
        .block(Block::default().borders(Borders::ALL))
        .wrap(Wrap { trim: false });
    f.render_widget(input, inner[1]);

    let list_area = inner[2];
    let window_height = list_area.height.saturating_sub(2) as usize;
    let start = app.search_selected.saturating_sub(window_height / 2);
    let end = usize::min(start + window_height, app.search_results.len());

    let items: Vec<ListItem> = app.search_results[start..end]
        .iter()
        .enumerate()
        .map(|(offset, r)| {
            let i = start + offset;
            let style = if i == app.search_selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(r.label.clone()).style(style)
        })
        .collect();

    let list = List::new(items).block(Block::default().title("Results").borders(Borders::ALL));
    f.render_widget(list, list_area);
}

fn draw_scratchpad(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(area);

    // Filter notes if search is active
    let display_indices: Vec<usize> = if !app.scratchpad_search_query.is_empty() {
        app.scratchpad_search_results.clone()
    } else {
        (0..app.scratchpad_module.notes.len()).collect()
    };

    let items: Vec<ListItem> = display_indices
        .iter()
        .map(|&idx| {
            let note = &app.scratchpad_module.notes[idx];
            let style = if idx == app.selected_index {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let size_kb = note.size_bytes as f64 / 1024.0;
            let size_str = if size_kb < 1.0 {
                format!("{}B", note.size_bytes)
            } else {
                format!("{:.1}KB", size_kb)
            };

            let created = note.created_at.format("%Y-%m-%d").to_string();

            ListItem::new(format!(
                "📝 {} - {} ({}) [created: {}]",
                note.name,
                note.modified_at.format("%Y-%m-%d %H:%M"),
                size_str,
                created
            ))
            .style(style)
        })
        .collect();

    let title = if !app.scratchpad_search_query.is_empty() {
        format!(
            "Scratchpad Notes - Search: \"{}\" ({} results) (n: new, d: delete, c: copy, e: export, R: rename, f: search, Enter: open)",
            app.scratchpad_search_query,
            display_indices.len()
        )
    } else {
        "Scratchpad Notes (n: new, d: delete, c: copy, e: export, R: rename, f: search, Enter: open, r: refresh, t: preview)".to_string()
    };

    if items.is_empty() {
        let empty_msg = if !app.scratchpad_search_query.is_empty() {
            format!(
                "No notes match \"{}\"\nPress 'f' to search again",
                app.scratchpad_search_query
            )
        } else {
            "No notes yet. Press 'n' to create a new note".to_string()
        };
        let empty =
            Paragraph::new(empty_msg).block(Block::default().title(title).borders(Borders::ALL));
        f.render_widget(empty, chunks[0]);
    } else {
        let list = List::new(items).block(Block::default().title(title).borders(Borders::ALL));
        f.render_widget(list, chunks[0]);
    }

    // Preview pane
    if app.show_detail && app.selected_index < app.scratchpad_module.notes.len() {
        if let Ok(preview) = app
            .scratchpad_module
            .get_content_preview(app.selected_index, 500)
        {
            let detail = Paragraph::new(preview)
                .block(
                    Block::default()
                        .title("Preview (t to toggle)")
                        .borders(Borders::ALL),
                )
                .wrap(Wrap { trim: false });
            f.render_widget(detail, chunks[1]);
        } else {
            let help = Paragraph::new("Could not load preview")
                .block(Block::default().title("Preview").borders(Borders::ALL));
            f.render_widget(help, chunks[1]);
        }
    } else {
        let help = Paragraph::new("Shortcuts:\n  t: toggle preview\n  n: new note\n  c: copy to clipboard\n  e: export to path\n  R: rename\n  f: search\n  Enter: open in editor")
            .block(Block::default().title("Help").borders(Borders::ALL));
        f.render_widget(help, chunks[1]);
    }
}

fn draw_shell(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(8)])
        .split(area);

    // Command history output
    let history_items: Vec<ListItem> = app
        .shell_terminal_module
        .history
        .iter()
        .take(20)
        .map(|cmd| {
            let output_preview = if cmd.output.len() > 100 {
                format!("{}...", &cmd.output[..97])
            } else {
                cmd.output.clone()
            };

            let exit_status = match cmd.exit_code {
                Some(0) => "✓",
                Some(_) => "✗",
                None => "?",
            };

            ListItem::new(vec![
                Line::from(vec![
                    Span::styled(
                        exit_status,
                        Style::default().fg(if cmd.exit_code == Some(0) {
                            Color::Green
                        } else {
                            Color::Red
                        }),
                    ),
                    Span::raw(" "),
                    Span::styled(&cmd.command, Style::default().fg(Color::Cyan)),
                    Span::raw(format!(
                        " @ {} ({})",
                        cmd.working_dir.display(),
                        cmd.timestamp.format("%H:%M:%S")
                    )),
                ]),
                Line::from(format!("  {}", output_preview)),
            ])
        })
        .collect();

    if app.shell_terminal_module.history.is_empty() {
        let empty = Paragraph::new("No commands executed yet\n\nPress 'i' to enter a command.\n\nSupported built-ins: cd, pwd, clear, exit, export, history\nExternal commands run through your system shell.")
            .block(Block::default().title("Shell Terminal").borders(Borders::ALL));
        f.render_widget(empty, chunks[0]);
    } else {
        let list = List::new(history_items).block(
            Block::default()
                .title("Command History (most recent first)")
                .borders(Borders::ALL),
        );
        f.render_widget(list, chunks[0]);
    }

    // Current working directory and prompt info
    let cwd = app.shell_terminal_module.get_current_dir();
    let prompt = app.shell_terminal_module.get_prompt();
    let history_count = app.shell_terminal_module.history.len();
    let max_history = app.shell_terminal_module.max_history;

    let info_text = format!(
        "Working Directory: {}\nPrompt: {}\nHistory: {}/{}\n\nKeys: i=input  h=search history  C=clear history  r=refresh",
        cwd.display(),
        prompt,
        history_count,
        max_history
    );

    let info = Paragraph::new(info_text)
        .block(Block::default().title("Shell Info").borders(Borders::ALL))
        .wrap(Wrap { trim: false });
    f.render_widget(info, chunks[1]);
}

fn draw_shell_input_popup(f: &mut Frame, app: &App) {
    let area = centered_rect(70, 20, f.area());
    let prompt = app.shell_terminal_module.get_prompt();
    let input_text = format!("{}{}", prompt, app.shell_input_buffer);
    let input = Paragraph::new(input_text)
        .block(
            Block::default()
                .title("Shell Command (Enter to execute, Esc to cancel)")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Green)),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(ratatui::widgets::Clear, area);
    f.render_widget(input, area);
}

fn draw_services(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(area);

    // Calculate visible window
    let window_height = chunks[0].height.saturating_sub(2) as usize;
    let total = app.services_module.services.len();
    let start = app.selected_index.saturating_sub(window_height / 2);
    let end = usize::min(start + window_height, total);

    let items: Vec<ListItem> = app
        .services_module
        .services
        .iter()
        .enumerate()
        .skip(start)
        .take(end.saturating_sub(start))
        .map(|(i, svc)| {
            let style = if i == app.selected_index {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let state_icon = match svc.state {
                crate::modules::services::ServiceState::Running => "🟢",
                crate::modules::services::ServiceState::Stopped => "🔴",
                #[cfg(any(target_os = "linux", target_os = "macos"))]
                crate::modules::services::ServiceState::Failed => "❌",
                crate::modules::services::ServiceState::Unknown => "⚪",
            };

            let state_color = match svc.state {
                crate::modules::services::ServiceState::Running => Color::Green,
                crate::modules::services::ServiceState::Stopped => Color::Red,
                #[cfg(any(target_os = "linux", target_os = "macos"))]
                crate::modules::services::ServiceState::Failed => Color::Red,
                crate::modules::services::ServiceState::Unknown => Color::Gray,
            };

            let enabled_icon = if svc.enabled { "✓" } else { "✗" };

            let mut line_parts = vec![
                Span::raw(state_icon),
                Span::raw(" "),
                Span::styled(format!("{:<30}", svc.display_name), style),
                Span::raw(" "),
                Span::styled(
                    format!("[{}]", svc.state.as_str()),
                    Style::default().fg(state_color),
                ),
                Span::raw(format!(" {} ", enabled_icon)),
            ];

            if let Some(ref mem) = svc.memory_usage {
                line_parts.push(Span::raw(format!("Mem: {} ", mem)));
            }

            if let Some(pid) = svc.pid {
                line_parts.push(Span::raw(format!("PID: {} ", pid)));
            }

            ListItem::new(Line::from(line_parts)).style(style)
        })
        .collect();

    let title = if let Some(ref filter) = app.services_module.filter_state {
        format!(
            "Services [Filter: {}] (s: start, S: stop, R: restart, E: enable, D: disable, l: logs, u: user/sys, f: search)",
            filter.as_str()
        )
    } else {
        let scope = if app.services_module.show_user_services {
            "user"
        } else {
            "system"
        };
        format!(
            "Services [{}] (s: start, S: stop, R: restart, E: enable, D: disable, l: logs, u: user/sys, f: search, r: refresh)",
            scope
        )
    };

    if items.is_empty() {
        let empty = Paragraph::new(
            "No services found. Press 'r' to refresh or 'u' to toggle user/system services",
        )
        .block(Block::default().title(title).borders(Borders::ALL));
        f.render_widget(empty, chunks[0]);
    } else {
        let list = List::new(items).block(Block::default().title(title).borders(Borders::ALL));
        f.render_widget(list, chunks[0]);
    }

    // Details pane
    if app.show_detail && app.selected_index < app.services_module.services.len() {
        let svc = &app.services_module.services[app.selected_index];
        let mut detail_text = format!(
            "Name: {}\nDisplay Name: {}\nState: {}\nEnabled: {}",
            svc.name,
            svc.display_name,
            svc.state.as_str(),
            if svc.enabled { "Yes" } else { "No" }
        );

        if !svc.description.is_empty() {
            detail_text.push_str(&format!("\nDescription: {}", svc.description));
        }

        if let Some(pid) = svc.pid {
            detail_text.push_str(&format!("\nPID: {}", pid));
        }

        if let Some(ref mem) = svc.memory_usage {
            detail_text.push_str(&format!("\nMemory: {}", mem));
        }

        if let Some(ref uptime) = svc.uptime {
            detail_text.push_str(&format!("\nUptime: {}", uptime));
        }

        let detail = Paragraph::new(detail_text)
            .block(
                Block::default()
                    .title("Details (t to toggle)")
                    .borders(Borders::ALL),
            )
            .wrap(Wrap { trim: false });
        f.render_widget(detail, chunks[1]);
    } else {
        let help = Paragraph::new("Press 't' to toggle service details\nPress 'l' to view logs")
            .block(Block::default().title("Info").borders(Borders::ALL));
        f.render_widget(help, chunks[1]);
    }
}

fn draw_configs(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(area);

    let items: Vec<ListItem> = app
        .configs_module
        .configs
        .iter()
        .enumerate()
        .map(|(i, config)| {
            let style = if i == app.selected_index {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let exists_icon = if config.exists { "✓" } else { "✗" };
            let category_color = match config.category.as_str() {
                "Shell" => Color::Green,
                "Git" => Color::Cyan,
                "SSH" => Color::Magenta,
                "Editor" => Color::Blue,
                _ => Color::White,
            };

            let truncated_path = if config.path.to_string_lossy().len() > 40 {
                format!("{}...", &config.path.to_string_lossy()[..37])
            } else {
                config.path.to_string_lossy().to_string()
            };

            let size_info = if let Some(size) = config.file_size {
                format!(" ({})", ConfigsModule::format_file_size(size))
            } else {
                String::new()
            };

            let modified_info = if let Some(ref modified) = config.last_modified {
                format!(" [{}]", modified)
            } else {
                String::new()
            };

            ListItem::new(Line::from(vec![
                Span::styled(
                    exists_icon,
                    Style::default().fg(if config.exists {
                        Color::Green
                    } else {
                        Color::Red
                    }),
                ),
                Span::raw(" "),
                Span::styled(format!("{:<20}", config.name), style),
                Span::styled(
                    format!("[{}] ", config.category),
                    Style::default().fg(category_color),
                ),
                Span::raw(truncated_path.to_string()),
                Span::styled(size_info, Style::default().fg(Color::Gray)),
                Span::styled(modified_info, Style::default().fg(Color::Gray)),
            ]))
        })
        .collect();

    if items.is_empty() {
        let empty = Paragraph::new("No configs found. Press 'n' to add")
            .block(Block::default().title("Configs").borders(Borders::ALL));
        f.render_widget(empty, chunks[0]);
    } else {
        let list = List::new(items)
            .block(Block::default().title("Configs (n: new, d: delete, Enter: open, b: backup, v: view, c: copy, o: open folder, f: search)").borders(Borders::ALL));
        f.render_widget(list, chunks[0]);
    }

    // Details pane
    if app.show_detail && app.selected_index < app.configs_module.configs.len() {
        let config = &app.configs_module.configs[app.selected_index];
        let mut detail_text = format!(
            "Name: {}\nPath: {}\nCategory: {}\nDescription: {}\nExists: {}",
            config.name,
            config.path.display(),
            config.category,
            config.description,
            if config.exists { "Yes" } else { "No" }
        );

        if let Some(ref editor) = config.editor {
            detail_text.push_str(&format!("\nEditor: {}", editor));
        }

        if let Some(size) = config.file_size {
            detail_text.push_str(&format!(
                "\nSize: {}",
                ConfigsModule::format_file_size(size)
            ));
        }

        if let Some(ref modified) = config.last_modified {
            detail_text.push_str(&format!("\nLast Modified: {}", modified));
        }

        let detail = Paragraph::new(detail_text)
            .block(
                Block::default()
                    .title("Details (t to toggle)")
                    .borders(Borders::ALL),
            )
            .wrap(Wrap { trim: false });
        f.render_widget(detail, chunks[1]);
    } else {
        let help = Paragraph::new(
            "Press 't' to toggle config details\nPress 'v' to preview config content",
        )
        .block(Block::default().title("Info").borders(Borders::ALL));
        f.render_widget(help, chunks[1]);
    }
}
