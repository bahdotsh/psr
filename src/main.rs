mod app;
mod processes;
mod ui;

use app::{App, SortKey};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use processes::{ProcessMonitor, ProcessUpdate};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::time::Duration;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Terminal initialization
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create communication channels
    let (tx, mut rx) = mpsc::channel(100);

    // Create process monitor and start it in the background
    let (process_monitor, refresh_sender) = ProcessMonitor::new(tx.clone());
    tokio::spawn(async move {
        process_monitor.start_monitoring().await;
    });

    // Create app with empty initial state
    let mut app = App::new();
    app.refresh_sender = Some(refresh_sender);
    // Display "Loading..." message
    terminal.draw(|f| ui::draw_loading_screen(f))?;

    // Main loop
    loop {
        // Process any updates from the background task
        while let Ok(update) = rx.try_recv() {
            match update {
                ProcessUpdate::ProcessList(processes) => {
                    app.processes = processes;
                    app.update_selection();
                    app.sort_processes();
                }
                ProcessUpdate::SystemInfo(cpu, used, total) => {
                    app.system_resources.update(cpu, used, total);
                }
                ProcessUpdate::LoadingStatus(status) => {
                    app.loading_status = status;
                }
            }
        }

        // Draw UI if needed
        if app.should_refresh_ui() {
            terminal.draw(|f| ui::draw_ui(f, &mut app))?;
            app.refresh_ui();
        }

        // Poll for events with a short timeout to keep things responsive
        if event::poll(Duration::from_millis(16))? {
            if let Event::Key(key) = event::read()? {
                // Check if Ctrl is being pressed
                let ctrl_pressed = key.modifiers.contains(KeyModifiers::CONTROL);

                match (key.code, ctrl_pressed) {
                    // Ctrl+key combinations for commands
                    (KeyCode::Char('q'), true) | (KeyCode::Esc, _) | (KeyCode::Char('c'), true) => {
                        if !app.filter.is_empty() {
                            app.clear_filter();
                        } else {
                            break; // Only exit if filter is empty
                        }
                    }
                    (KeyCode::Char('r'), true) => {
                        // Request an immediate refresh
                        if let Some(tx) = &app.refresh_sender {
                            let _ = tx.try_send(());
                        }
                    }
                    (KeyCode::Char('k'), true) => app.kill_selected_process(),
                    (KeyCode::Char('h'), true) => app.toggle_help(),

                    // Navigation and UI controls
                    (KeyCode::Up, _) => app.previous(),
                    (KeyCode::Down, _) => app.next(),
                    (KeyCode::Left, _) => app.previous_tab(),
                    (KeyCode::Right, _) => app.next_tab(),
                    (KeyCode::Tab, _) => app.next_tab(),
                    (KeyCode::BackTab, _) => app.previous_tab(), // Shift+Tab

                    // Sorting controls
                    (KeyCode::Char(' '), _) => app.toggle_sort(),
                    (KeyCode::Char('1'), true) => app.set_sort_key(SortKey::Pid),
                    (KeyCode::Char('2'), true) => app.set_sort_key(SortKey::Name),
                    (KeyCode::Char('3'), true) => app.set_sort_key(SortKey::Cpu),
                    (KeyCode::Char('4'), true) => app.set_sort_key(SortKey::Memory),
                    (KeyCode::Char('5'), true) => app.set_sort_key(SortKey::Status),
                    (KeyCode::Char('6'), true) => app.set_sort_key(SortKey::User),
                    (KeyCode::Char('7'), true) => app.set_sort_key(SortKey::StartTime),

                    // Filter controls
                    (KeyCode::Backspace, _) => app.backspace_filter(),

                    // Regular character typing for filter (when Ctrl is not pressed)
                    (KeyCode::Char(c), false) => app.add_to_filter(c),

                    _ => {}
                }
            }
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}
