use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::time::Duration;

mod app;
mod config;
mod modules;
mod ui;

use app::{App, AppState, MenuSection};

#[tokio::main]
async fn main() -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new().await?;
    let res = run_app(&mut terminal, &mut app).await;

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("Error: {:?}", err);
    }

    Ok(())
}

async fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> Result<()> {
    loop {
        terminal.draw(|f| ui::draw(f, app))?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(KeyEvent { code, modifiers, kind, .. }) = event::read()? {
                if kind != KeyEventKind::Press { continue; }
                match app.state {
                    AppState::Normal => match code {
                        KeyCode::Char('q') => return Ok(()),
                        KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                            return Ok(())
                        }
                        KeyCode::Char('?') => { app.show_help = !app.show_help; }
                        KeyCode::Char('1') => app.current_section = MenuSection::Dashboard,
                        KeyCode::Char('2') => app.current_section = MenuSection::Apps,
                        KeyCode::Char('3') => app.current_section = MenuSection::Bookmarks,
                        KeyCode::Char('4') => app.current_section = MenuSection::Clipboard,
                        KeyCode::Char('5') => app.current_section = MenuSection::Docker,
                        KeyCode::Char('6') => app.current_section = MenuSection::Network,
                        KeyCode::Char('7') => app.current_section = MenuSection::SSH,
                        KeyCode::Char('8') => app.current_section = MenuSection::Scripts,
                        KeyCode::Char('9') => app.current_section = MenuSection::Git,
                        KeyCode::Char('0') => app.current_section = MenuSection::History,
                        KeyCode::Char('-') => app.current_section = MenuSection::Scratchpad,
                        KeyCode::Char('=') => app.current_section = MenuSection::Shell,
                        KeyCode::Char(']') => app.current_section = MenuSection::Services,
                        KeyCode::Char('[') => app.current_section = MenuSection::Notifications,
                        KeyCode::Up | KeyCode::Char('k') => app.previous_item(),
                        KeyCode::Down | KeyCode::Char('j') => app.next_item(),
                        KeyCode::PageUp => app.page_up(),
                        KeyCode::PageDown => app.page_down(),
                        KeyCode::Home => app.go_home(),
                        KeyCode::End => app.go_end(),
                        KeyCode::Char('/') => app.open_search(),
                        KeyCode::Enter => {
                            if let Err(e) = app.activate_item().await { app.report_error("Action failed", e); }
                        }
                        KeyCode::Char('n') => app.new_item(),
                        KeyCode::Char('d') => app.delete_item(),
                        KeyCode::Char('r') => {
                            if let Err(e) = app.refresh().await { app.report_error("Refresh failed", e); }
                        }
                        KeyCode::Char('R') => {
                            if app.current_section == MenuSection::Scratchpad {
                                app.scratchpad_rename();
                            } else if app.current_section == MenuSection::Services {
                                app.restart_service();
                                if let Err(e) = app.refresh().await { app.report_error("Refresh failed", e); }
                            }
                        }
                        KeyCode::Char('s') => {
                            if app.current_section == MenuSection::Services {
                                app.start_service();
                            } else {
                                app.stop_selected();
                            }
                        }
                        KeyCode::Char('t') => app.toggle_detail(),
                        KeyCode::Char('S') => {
                            if app.current_section == MenuSection::Git {
                                app.status_message = "Scanning for git repositories...".to_string();
                                if let Err(e) = app.git_module.scan_repositories() {
                                    app.report_error("Scan failed", e);
                                } else {
                                    app.status_message = format!("Found {} repositories", app.git_module.repos.len());
                                }
                            } else if app.current_section == MenuSection::Services {
                                app.stop_service();
                            } else {
                                if let Err(e) = app.schedule_selected_script().await { app.report_error("Schedule failed", e); }
                            }
                        }
                        KeyCode::Char('u') => {
                            if app.current_section == MenuSection::Services {
                                app.services_module.toggle_user_services();
                                if let Err(e) = app.refresh().await { app.report_error("Refresh failed", e); }
                            }
                        }
                        KeyCode::Char('E') => {
                            if app.current_section == MenuSection::Services {
                                app.enable_service();
                            }
                        }
                        KeyCode::Char('D') => {
                            if app.current_section == MenuSection::Services {
                                app.disable_service();
                            }
                        }
                        KeyCode::Char('l') => {
                            if app.current_section == MenuSection::Services {
                                app.view_service_logs();
                            }
                        }
                        KeyCode::Char('x') => app.disconnect_latest_session(),
                        KeyCode::Char('v') => {
                            if app.current_section == MenuSection::Docker {
                                // Cycle through Docker views
                                use crate::modules::docker::DockerView;
                                app.docker_module.current_view = match app.docker_module.current_view {
                                    DockerView::Containers => DockerView::Images,
                                    DockerView::Images => DockerView::Containers,
                                    _ => DockerView::Containers,
                                };
                                app.selected_index = 0;
                                if let Err(e) = app.refresh().await { app.report_error("Refresh failed", e); }
                            } else if app.current_section == MenuSection::Network {
                                // Cycle through Network views
                                use crate::modules::network::NetworkView;
                                app.network_module.current_view = match app.network_module.current_view {
                                    NetworkView::Connections => NetworkView::Interfaces,
                                    NetworkView::Interfaces => NetworkView::Ports,
                                    NetworkView::Ports => NetworkView::Connections,
                                };
                                app.selected_index = 0;
                                if let Err(e) = app.refresh().await { app.report_error("Refresh failed", e); }
                            }
                        }
                        KeyCode::Char('f') => {
                            if app.current_section == MenuSection::Network {
                                // Toggle ESTABLISHED filter for network connections
                                app.network_module.toggle_filter("ESTABLISHED");
                                if let Err(e) = app.refresh().await { app.report_error("Refresh failed", e); }
                            } else if app.current_section == MenuSection::Scratchpad {
                                // Search in scratchpad
                                app.scratchpad_search();
                            } else if app.current_section == MenuSection::Services {
                                app.search_services();
                            }
                        }
                        KeyCode::Char('c') => {
                            if app.current_section == MenuSection::Scratchpad {
                                if let Err(e) = app.scratchpad_copy_to_clipboard() {
                                    app.report_error("Copy failed", e);
                                }
                            }
                        }
                        KeyCode::Char('i') => {
                            if app.current_section == MenuSection::Shell {
                                app.open_shell_input();
                            }
                        }
                        KeyCode::Char('e') => {
                            if app.current_section == MenuSection::Scratchpad {
                                app.scratchpad_export();
                            }
                        }
                        KeyCode::Char('h') => {
                            if app.current_section == MenuSection::Shell {
                                app.shell_search_history();
                            }
                        }
                        KeyCode::Char('C') => {
                            if app.current_section == MenuSection::Shell {
                                app.shell_clear_history();
                            }
                        }
                        KeyCode::Tab => app.next_section(),
                        KeyCode::BackTab => app.previous_section(),
                        KeyCode::Esc => app.cancel_input(),
                        _ => {}
                    },
                    AppState::Input => match code {
                        KeyCode::Enter => {
                            if let Err(e) = app.submit_input().await { app.report_error("Submit failed", e); }
                        }
                        KeyCode::Esc => app.cancel_input(),
                        KeyCode::Backspace => app.input_backspace(),
                        KeyCode::Char(c) => app.input_char(c),
                        KeyCode::Left => app.input_move_left(),
                        KeyCode::Right => app.input_move_right(),
                        _ => {}
                    },
                    AppState::Search => match code {
                        KeyCode::Enter => { app.submit_search(); }
                        KeyCode::Esc => { app.close_search(); }
                        KeyCode::Backspace => { app.search_backspace(); }
                        KeyCode::Up | KeyCode::Char('k') => { app.search_prev(); }
                        KeyCode::Down | KeyCode::Char('j') => { app.search_next(); }
                        KeyCode::Left => { app.search_move_left(); }
                        KeyCode::Right => { app.search_move_right(); }
                        KeyCode::PageUp => { app.search_page_up(); }
                        KeyCode::PageDown => { app.search_page_down(); }
                        KeyCode::Home => { app.search_go_home(); }
                        KeyCode::End => { app.search_go_end(); }
                        KeyCode::Char(c) => { app.search_input_char(c); }
                        _ => {}
                    },
                    AppState::Confirm => match code {
                        KeyCode::Char('y') | KeyCode::Char('Y') => {
                            if let Err(e) = app.confirm_action().await { app.report_error("Confirm failed", e); }
                        }
                        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                            app.cancel_confirm()
                        }
                        _ => {}
                    },
                    AppState::ShellInput => match code {
                        KeyCode::Enter => {
                            if let Err(e) = app.execute_shell_command().await {
                                app.report_error("Shell command failed", e);
                            }
                        }
                        KeyCode::Esc => app.close_shell_input(),
                        KeyCode::Backspace => app.shell_input_backspace(),
                        KeyCode::Char(c) => app.shell_input_char(c),
                        KeyCode::Left => app.shell_input_move_left(),
                        KeyCode::Right => app.shell_input_move_right(),
                        _ => {}
                    },
                }
            }
        }

        if let Err(e) = app.auto_refresh().await { app.report_error("Auto refresh failed", e); }
    }
}