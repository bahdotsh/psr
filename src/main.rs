mod app;
mod processes;
mod ui;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::time::Duration;

use crate::app::{App, SortKey};
use crate::processes::ProcessMonitor;
use crate::ui::draw_ui;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Terminal initialization
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let process_monitor = ProcessMonitor::new();
    let mut app = App::new(process_monitor);

    // Main loop
    loop {
        // Check if we should refresh data before drawing
        if app.should_refresh_data() {
            app.refresh_all_data();
        }

        // Draw UI only when needed (reduces CPU usage)
        if app.should_refresh_ui() {
            terminal.draw(|f| draw_ui(f, &mut app))?;
            app.refresh_ui(); // Mark UI as refreshed
        }

        // Poll events with short timeout to stay responsive
        if event::poll(Duration::from_millis(16))? {
            if let Event::Key(key) = event::read()? {
                // Check if Ctrl is being pressed
                let ctrl_pressed = key.modifiers.contains(KeyModifiers::CONTROL);

                match (key.code, ctrl_pressed) {
                    // Ctrl+key combinations for commands
                    (KeyCode::Char('q'), true) | (KeyCode::Esc, _) | (KeyCode::Char('c'), true) => {
                        break
                    }
                    (KeyCode::Char('r'), true) => app.refresh_all_data(),
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
