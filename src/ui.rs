use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};

use crate::app::{App, AppState, MenuSection};

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
        MenuSection::Docker => draw_docker(f, app, main_chunks[1]),
        MenuSection::Network => draw_network(f, app, main_chunks[1]),
        MenuSection::SSH => draw_ssh(f, app, main_chunks[1]),
        MenuSection::Scripts => draw_scripts(f, app, main_chunks[1]),
        MenuSection::Notifications => draw_notifications(f, app, main_chunks[1]),
        MenuSection::History => draw_history(f, app, main_chunks[1]),
        MenuSection::Git => draw_git(f, app, main_chunks[1]),
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

    let mut header = format!("{} | {} | {} | shell: {}", username, time_str, arch, shell_label);
    if cores > 0 { header.push_str(&format!(" | CPU: {}c {:.0}%", cores, cpu_avg)); }



    let title = Paragraph::new(header)
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, area);
}

fn draw_menu(f: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = vec![
        ("1", "Dashboard", MenuSection::Dashboard),
        ("2", "Apps", MenuSection::Apps),
        ("3", "Bookmarks", MenuSection::Bookmarks),
        ("4", "Clipboard", MenuSection::Clipboard),
        ("5", "Docker", MenuSection::Docker),
        ("6", "Network", MenuSection::Network),
        ("7", "SSH", MenuSection::SSH),
        ("8", "Scripts", MenuSection::Scripts),
        ("9", "Git", MenuSection::Git),
    ].iter()
        .map(|(key, name, section)| {
            let style = if *section == app.current_section {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(format!("{} {}", key, name)).style(style)
        })
        .chain(std::iter::once({
            // Special handling for History/Notifications toggle
            let (label, is_active) = match app.current_section {
                MenuSection::History => ("0 History", true),
                MenuSection::Notifications => ("0 Notif", true),
                _ => ("0 History/Notif", false),
            };
            let style = if is_active {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(label).style(style)
        }))
        .collect();

    let list = List::new(items).block(
        Block::default()
            .title("Menu")
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
            let style = if i == app.selected_index { Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD) } else { Style::default() };
            ListItem::new(format!(
                "{:<6} {:<20} CPU: {:.1}% MEM: {:.1} MB",
                p.pid, p.name, p.cpu_usage, p.memory_usage
            )).style(style)
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
            let status_icon = if s.status == "Connected" { "🟢" } else { "🔴" };
            ListItem::new(format!("{} {} - {}", status_icon, s.name, s.status))
        })
        .collect();

    let ssh_block = List::new(ssh_sessions)
        .block(Block::default().title("Active SSH Sessions").borders(Borders::ALL));
    if app.ssh_module.active_sessions.is_empty() {
        let empty = Paragraph::new("No active SSH sessions")
            .block(Block::default().title("Active SSH Sessions").borders(Borders::ALL));
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

    let notif_block = List::new(notifs)
        .block(Block::default().title("Recent Notifications").borders(Borders::ALL));
    if app.notifications_module.notifications.is_empty() {
        let empty = Paragraph::new("No notifications yet")
            .block(Block::default().title("Recent Notifications").borders(Borders::ALL));
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
    let selected_in_apps = if app.selected_index < available_len { app.selected_index } else { available_len.saturating_sub(1) };
    let start = if available_len == 0 {
        0
    } else {
        let half = window_height / 2;
        let base = selected_in_apps.saturating_sub(half);
        let max_start = available_len.saturating_sub(window_height);
        usize::min(base, max_start)
    };
    let end = usize::min(start + window_height, available_len);

    let items: Vec<ListItem> = available_apps.get(start..end).unwrap_or(&[])
        .iter()
        .enumerate()
        .map(|(offset, name)| {
            let i = start + offset;
            let style = if i == app.selected_index {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(format!("▶ {}", name)).style(style)
        })
        .collect();

    if available_apps.is_empty() {
        let empty = if app.pending_initial_scan { "Scanning PATH..." } else { "No executables found in PATH" };
        let p = Paragraph::new(empty)
            .block(Block::default().title("Available Apps").borders(Borders::ALL));
        f.render_widget(p, chunks[0]);
    } else {
        let apps_list = List::new(items)
            .block(Block::default().title("Available Apps (Enter to launch)").borders(Borders::ALL));
        f.render_widget(apps_list, chunks[0]);
    }

    // Running Processes section
    let running_processes = &app.apps_module.running_processes;
    let process_window_height = (chunks[1].height.saturating_sub(2)) as usize;
    let proc_len = running_processes.len();
    let process_selection = if app.selected_index >= available_len { app.selected_index - available_len } else { 0 };
    let process_start = if proc_len == 0 {
        0
    } else {
        let half = process_window_height / 2;
        let base = process_selection.saturating_sub(half);
        let max_start = proc_len.saturating_sub(process_window_height);
        usize::min(base, max_start)
    };
    let process_end = usize::min(process_start + process_window_height, proc_len);

    let running: Vec<ListItem> = running_processes.get(process_start..process_end).unwrap_or(&[])
        .iter()
        .enumerate()
        .map(|(offset, p)| {
            let i = available_len + process_start + offset;
            let style = if i == app.selected_index {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(format!(
                "{:<6} {:<20} CPU: {:.1}% MEM: {:.1} MB",
                p.pid, p.name, p.cpu_usage, p.memory_usage
            )).style(style)
        })
        .collect();

    let running_block = List::new(running)
        .block(Block::default().title("Running Processes (s to stop)").borders(Borders::ALL));
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
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
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
        let list = List::new(items)
            .block(Block::default().title("Bookmarks (n: new, d: delete, Enter: open)").borders(Borders::ALL));
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
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
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
            ListItem::new(format!("{} {}{} ({})", icon, pinned_icon, truncated_content, e.timestamp.format("%H:%M:%S"))).style(style)
        })
        .collect();

    if items.is_empty() {
        let empty = Paragraph::new("No clipboard entries. Press 'r' to refresh")
            .block(Block::default().title("Clipboard").borders(Borders::ALL));
        f.render_widget(empty, area);
    } else {
        let list = List::new(items)
            .block(Block::default().title("Clipboard (Enter: copy, r: refresh, p: pin)").borders(Borders::ALL));
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
                        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
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
                    ListItem::new(format!("{} {} - {} ({})", status_icon, truncated_name, c.image, c.status)).style(style)
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
                        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    };
                    let truncated_repo = if img.repository.len() > 25 {
                        format!("{}...", &img.repository[..22])
                    } else {
                        img.repository.clone()
                    };
                    ListItem::new(format!("📦 {}:{} - {}", truncated_repo, img.tag, img.size)).style(style)
                })
                .collect();
            
            if docker_items.is_empty() {
                vec![ListItem::new("No images found. Press 'r' to refresh")]
            } else {
                docker_items
            }
        }
        _ => vec![ListItem::new("View not implemented yet")]
    };

    let list = List::new(items)
        .block(Block::default().title("Docker (Enter: exec, r: refresh, v: switch view)").borders(Borders::ALL));
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
        _ => "View: Other"
    };
    
    let status = Paragraph::new(status_text)
        .block(Block::default().borders(Borders::ALL));
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
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
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
        let list = List::new(items)
            .block(Block::default().title("SSH Hosts (n: new, d: delete, Enter: connect)").borders(Borders::ALL));
        f.render_widget(list, chunks[0]);
    }

    let sessions: Vec<ListItem> = app
        .ssh_module
        .active_sessions
        .iter()
        .map(|s| {
            let status_color = if s.status == "Connected" { Color::Green } else { Color::Red };
            let status_icon = if s.status == "Connected" { "🟢" } else { "🔴" };
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

    let sessions_block = List::new(sessions)
        .block(Block::default().title("Active Sessions").borders(Borders::ALL));
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
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            let status = if app.scripts_module.is_running(i) { "🔄" } else { "▶" };
            ListItem::new(format!("{} {} - {}", status, s.name, s.description)).style(style)
        })
        .collect();

    if items.is_empty() {
        let empty = Paragraph::new("No scripts. Press 'n' to add")
            .block(Block::default().title("Scripts").borders(Borders::ALL));
        f.render_widget(empty, chunks[0]);
    } else {
        let list = List::new(items)
            .block(Block::default().title("Scripts (n: new, d: delete, Enter: run)").borders(Borders::ALL));
        f.render_widget(list, chunks[0]);
    }

    if app.show_detail && app.selected_index < app.scripts_module.scripts.len() {
        let script = &app.scripts_module.scripts[app.selected_index];
        let detail = Paragraph::new(format!(
            "Name: {}\nCommand: {}\nDescription: {}",
            script.name, script.command, script.description
        ))
        .block(Block::default().title("Details (t to toggle)").borders(Borders::ALL));
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
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
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
        let empty = Paragraph::new("No notifications yet")
            .block(Block::default().title("Notifications").borders(Borders::ALL));
        f.render_widget(empty, area);
    } else {
        let list = List::new(items)
            .block(Block::default().title("Notifications").borders(Borders::ALL));
        f.render_widget(list, area);
    }
}

fn draw_history(f: &mut Frame, app: &App, area: Rect) {
    let window_height = area.height.saturating_sub(2) as usize;
    let total = app.shell_module.entries.len();
    let start = app.selected_index.saturating_sub(window_height / 2);
    let end = usize::min(start + window_height, total);
    let any_ts = app.shell_module.entries.iter().any(|e| e.timestamp.is_some());

    let items: Vec<ListItem> = app
        .shell_module
        .entries[start..end]
        .iter()
        .enumerate()
        .map(|(offset, e)| {
            let i = start + offset;
            let style = if i == app.selected_index { Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD) } else { Style::default() };
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
        let empty = ratatui::widgets::Paragraph::new("No shell history detected")
            .block(Block::default().title("History (Enter to run, r to refresh)").borders(Borders::ALL));
        f.render_widget(empty, area);
    } else {
        let list = List::new(items)
            .block(Block::default().title("History (Enter to run, r to refresh)").borders(Borders::ALL));
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
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
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
                Span::styled(format!(" [{}] ", repo.branch), Style::default().fg(Color::Cyan)),
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
            .block(Block::default().title("Git Repositories").borders(Borders::ALL));
        f.render_widget(empty, area);
    } else {
        let list = List::new(items)
            .block(Block::default().title("Git Repositories (Enter: open, r: refresh, S: scan)").borders(Borders::ALL));
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
                        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
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
                        Span::styled(format!("{:<12}", conn.state), Style::default().fg(state_color)),
                        Span::raw(format!(" {}", conn.process_name)),
                    ])).style(style)
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
                        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    };
                    
                    let status_icon = if iface.status == "up" || iface.status.to_uppercase() == "UP" { "🟢" } else { "🔴" };
                    let ips = if iface.ip_addresses.is_empty() {
                        String::from("no IP")
                    } else {
                        iface.ip_addresses.join(", ")
                    };
                    
                    ListItem::new(format!("{} {:<15} {} ({})", status_icon, iface.name, ips, iface.mac_address)).style(style)
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
                        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    };
                    
                    ListItem::new(format!("Port {:<6} ({:<4}) - {} (PID: {})", 
                        port.port, port.protocol, port.process_name, port.pid
                    )).style(style)
                })
                .collect();
            
            if net_items.is_empty() {
                vec![ListItem::new("No listening ports found. Press 'r' to refresh")]
            } else {
                net_items
            }
        }
    };

    let title = match app.network_module.current_view {
        crate::modules::network::NetworkView::Connections => {
            if let Some(ref filter) = app.network_module.filter_state {
                format!("Network Connections [Filter: {}] (v: switch view, r: refresh, f: filter)", filter)
            } else {
                "Network Connections (v: switch view, r: refresh, f: filter)".to_string()
            }
        }
        crate::modules::network::NetworkView::Interfaces => "Network Interfaces (v: switch view, r: refresh)".to_string(),
        crate::modules::network::NetworkView::Ports => "Listening Ports (v: switch view, r: refresh)".to_string(),
    };

    let list = List::new(items)
        .block(Block::default().title(title).borders(Borders::ALL));
    f.render_widget(list, chunks[0]);

    // Status bar showing current view
    let view_name = match app.network_module.current_view {
        crate::modules::network::NetworkView::Connections => "Connections",
        crate::modules::network::NetworkView::Interfaces => "Interfaces",
        crate::modules::network::NetworkView::Ports => "Listening Ports",
    };
    
    let status_text = format!("View: {} | Press 'v' to switch view", view_name);
    let status = Paragraph::new(status_text)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(status, chunks[1]);
}

fn draw_status(f: &mut Frame, app: &App, area: Rect) {
    let help_text = match app.state {
        AppState::Normal => "q: Quit | Tab/Shift+Tab: Next/Prev Section | ↑↓/jk: Navigate | Enter: Select | n: New | d: Delete | r: Refresh | s: Stop Process | x: Disconnect SSH | t: Toggle",
        AppState::Input => "Enter: Submit | Esc: Cancel | Type your input",
        AppState::Confirm => "y: Yes | n: No | Esc: Cancel",
        AppState::Search => "/: Search | Enter: Jump | Esc: Close | ↑↓/PgUp/PgDn/Home/End: Navigate",
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
    let area = centered_rect(70, 70, f.area());
    let help = "launchr Help\n\nKeys:\n  q: Quit\n  Tab/Shift+Tab: Next/Previous Section\n  0-9: Jump to section (0 toggles History/Notifications)\n  j/k or ↑/↓: Navigate\n  PgUp/PgDn, Home/End: Page/Jump\n  Enter: Activate\n  n/d: New/Delete\n  r: Refresh\n  s: Stop process (Dashboard/Apps)\n  t: Toggle details (Scripts)\n  v: Switch view (Docker/Network)\n  f: Filter connections (Network)\n  /: Open fuzzy search\n  In search: type to filter, ↑/↓/PgUp/PgDn/Home/End to move, Enter to jump, Esc to close\n  S: Schedule script (Scripts) or Scan repositories (Git)\n  x: Disconnect latest SSH\n  ?: Toggle this help\n\nSections:\n  Dashboard: View running processes (s to stop)\n  Apps: Launch apps or stop processes (s to stop)\n  Docker: Manage containers/images (v to switch view)\n  Network: View connections/interfaces/ports (v to switch, f to filter)\n  SSH: Connect to hosts or disconnect sessions (x to disconnect)\n  Scripts: Run or schedule scripts\n  Git: View and manage repositories (S to scan)\n  History: Re-run shell commands";

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
            Constraint::Length(1),  // header line
            Constraint::Length(3),  // input box height (taller so it's obvious)
            Constraint::Min(0),     // results list
        ])
        .split(inner_area);

    let header = Paragraph::new("Type to filter, Enter to jump, Esc to close")
        .block(Block::default());
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

    let items: Vec<ListItem> = app
        .search_results[start..end]
        .iter()
        .enumerate()
        .map(|(offset, r)| {
            let i = start + offset;
            let style = if i == app.search_selected { Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD) } else { Style::default() };
            ListItem::new(r.label.clone()).style(style)
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().title("Results").borders(Borders::ALL));
    f.render_widget(list, list_area);
}


