use ratatui::backend::Backend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols::Marker;
use ratatui::text::{Span, Spans};
use ratatui::widgets::{
    Axis, BarChart, Block, Borders, Cell, Chart, Dataset, Paragraph, Row, Sparkline, Table, Tabs,
    Wrap,
};
use ratatui::Frame;
use std::time::Duration;

use crate::app::{App, SortKey};
use crate::processes::ProcessInfo;

// Collection of color constants
struct Colors;
#[allow(dead_code)]
impl Colors {
    const BACKGROUND: Color = Color::Rgb(20, 20, 30);
    const TEXT: Color = Color::Gray;
    const HIGHLIGHT: Color = Color::Yellow;
    const HEADER: Color = Color::Cyan;
    const BORDER: Color = Color::DarkGray;
    const CPU: Color = Color::LightGreen;
    const MEMORY: Color = Color::LightBlue;
    const WARNING: Color = Color::LightYellow;
    const ERROR: Color = Color::LightRed;
    const TAB_ACTIVE: Color = Color::Yellow;
    const TAB_INACTIVE: Color = Color::Gray;
}

pub fn draw_ui<B: Backend>(f: &mut Frame<B>, app: &mut App) {
    let size = f.size();

    // Create the layout
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(
            [
                Constraint::Length(3), // Tabs
                Constraint::Min(0),    // Main content
                Constraint::Length(1), // Filter line
                Constraint::Length(2), // Help
            ]
            .as_ref(),
        )
        .split(size);

    // Draw tabs with improved styling
    let tab_titles: Vec<Spans> = app
        .tabs
        .iter()
        .map(|t| {
            Spans::from(vec![
                Span::styled(" ", Style::default().fg(Colors::TEXT)),
                Span::styled(*t, Style::default().fg(Colors::TEXT)),
                Span::styled(" ", Style::default().fg(Colors::TEXT)),
            ])
        })
        .collect();

    let tabs = Tabs::new(tab_titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Colors::BORDER))
                .title(Span::styled(
                    " Process Monitor ",
                    Style::default().fg(Colors::HEADER),
                )),
        )
        .select(app.current_tab)
        .style(Style::default().fg(Colors::TAB_INACTIVE))
        .highlight_style(
            Style::default()
                .fg(Colors::TAB_ACTIVE)
                .add_modifier(Modifier::BOLD)
                .add_modifier(Modifier::UNDERLINED),
        );

    f.render_widget(tabs, chunks[0]);

    // Draw main content based on current tab
    match app.current_tab {
        0 => draw_dashboard_tab(f, app, chunks[1]),
        1 => draw_processes_tab(f, app, chunks[1]),
        2 => draw_user_processes_tab(f, app, chunks[1]),
        3 => draw_system_processes_tab(f, app, chunks[1]),
        4 => draw_detailed_view(f, app, chunks[1]),
        _ => {}
    }

    // Draw filter bar
    let filter_text = if app.filter.is_empty() {
        Span::styled(
            " Type to filter processes... ",
            Style::default().fg(Color::DarkGray),
        )
    } else {
        Span::styled(
            format!(" Filter: {} ", app.filter),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
    };

    let filter_bar = Paragraph::new(filter_text).style(Style::default().bg(Color::Black));
    f.render_widget(filter_bar, chunks[2]);

    // Draw help
    if app.show_help {
        draw_help_popup(f, app, size);
    } else {
        let help_text = Spans::from(vec![
            Span::raw(" q: Quit | "),
            Span::raw("r: Refresh | "),
            Span::raw("k: Kill | "),
            Span::raw("↑/↓: Navigate | "),
            Span::raw("←/→: Change tab | "),
            Span::raw("Space: Toggle sort | "),
            Span::raw("h: Help | "),
            Span::raw("Esc: Clear filter"),
        ]);
        let help = Paragraph::new(help_text).style(Style::default().fg(Color::DarkGray));

        f.render_widget(help, chunks[3]);
    }
}

fn draw_dashboard_tab<B: Backend>(f: &mut Frame<B>, app: &App, area: Rect) {
    // Create 2x2 grid layout for dashboard
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let top_row = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[0]);

    let bottom_row = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[1]);

    // Draw CPU usage chart
    draw_cpu_chart(f, app, top_row[0]);

    // Draw memory usage chart
    draw_memory_chart(f, app, top_row[1]);

    // Draw top CPU processes
    draw_top_cpu_processes(f, app, bottom_row[0]);

    // Draw top memory processes
    draw_top_memory_processes(f, app, bottom_row[1]);
}

fn draw_cpu_chart<B: Backend>(f: &mut Frame<B>, app: &App, area: Rect) {
    // CPU data: convert history to (x, y) data pairs
    let data: Vec<(f64, f64)> = app
        .system_resources
        .cpu_history
        .iter()
        .enumerate()
        .map(|(i, &cpu)| (i as f64, cpu as f64))
        .collect();

    // Create dataset
    let datasets = vec![Dataset::default()
        .name("CPU %")
        .marker(Marker::Braille)
        .style(Style::default().fg(Colors::CPU))
        .data(&data)];

    // Create chart
    let chart = Chart::new(datasets)
        .block(
            Block::default()
                .title(Span::styled(
                    format!(" CPU Usage: {:.1}% ", app.system_resources.cpu_usage),
                    Style::default()
                        .fg(Colors::HEADER)
                        .add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Colors::BORDER)),
        )
        .x_axis(
            Axis::default()
                .style(Style::default().fg(Colors::TEXT))
                .bounds([0.0, 60.0])
                .labels(vec![]),
        )
        .y_axis(
            Axis::default()
                .style(Style::default().fg(Colors::TEXT))
                .bounds([0.0, 100.0])
                .labels(vec![
                    Span::styled("0%", Style::default().fg(Colors::TEXT)),
                    Span::styled("50%", Style::default().fg(Colors::TEXT)),
                    Span::styled("100%", Style::default().fg(Colors::TEXT)),
                ]),
        );

    f.render_widget(chart, area);
}

fn draw_memory_chart<B: Backend>(f: &mut Frame<B>, app: &App, area: Rect) {
    // Memory data: convert history to (x, y) data pairs
    let data: Vec<(f64, f64)> = app
        .system_resources
        .memory_history
        .iter()
        .enumerate()
        .map(|(i, &mem)| (i as f64, mem as f64))
        .collect();

    // Create dataset
    let datasets = vec![Dataset::default()
        .name("Memory %")
        .marker(Marker::Braille)
        .style(Style::default().fg(Colors::MEMORY))
        .data(&data)];

    // Memory usage information
    let memory_percent = app.system_resources.memory_percentage();
    let used_gb = app.system_resources.used_memory as f64 / 1024.0 / 1024.0 / 1024.0;
    let total_gb = app.system_resources.total_memory as f64 / 1024.0 / 1024.0 / 1024.0;

    // Create chart
    let chart = Chart::new(datasets)
        .block(
            Block::default()
                .title(Span::styled(
                    format!(
                        " Memory: {:.1}% ({:.1}/{:.1} GB) ",
                        memory_percent, used_gb, total_gb
                    ),
                    Style::default()
                        .fg(Colors::HEADER)
                        .add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Colors::BORDER)),
        )
        .x_axis(
            Axis::default()
                .style(Style::default().fg(Colors::TEXT))
                .bounds([0.0, 60.0])
                .labels(vec![]),
        )
        .y_axis(
            Axis::default()
                .style(Style::default().fg(Colors::TEXT))
                .bounds([0.0, 100.0])
                .labels(vec![
                    Span::styled("0%", Style::default().fg(Colors::TEXT)),
                    Span::styled("50%", Style::default().fg(Colors::TEXT)),
                    Span::styled("100%", Style::default().fg(Colors::TEXT)),
                ]),
        );

    f.render_widget(chart, area);
}

fn draw_top_cpu_processes<B: Backend>(f: &mut Frame<B>, app: &App, area: Rect) {
    let (top_cpu, _) = app.top_processes(5);

    // Get the CPU usage percentages and process names
    let data: Vec<(&str, u64)> = top_cpu
        .iter()
        .map(|p| (p.name.as_str(), p.cpu_usage.round() as u64))
        .collect();

    // Create bar chart data
    let barchart = BarChart::default()
        .block(
            Block::default()
                .title(Span::styled(
                    " Top CPU Processes ",
                    Style::default()
                        .fg(Colors::HEADER)
                        .add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Colors::BORDER)),
        )
        .data(&data)
        .bar_width(7)
        .bar_gap(1)
        .bar_style(Style::default().fg(Colors::CPU).bg(Color::Black))
        .value_style(
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .label_style(Style::default().fg(Colors::TEXT));

    f.render_widget(barchart, area);
}

fn draw_top_memory_processes<B: Backend>(f: &mut Frame<B>, app: &App, area: Rect) {
    let (_, top_mem) = app.top_processes(5);

    // Create rows for each top memory process
    let rows = top_mem.iter().map(|p| {
        let memory_mb = p.memory / 1024 / 1024;
        let memory_percent = (p.memory as f64 / app.system_resources.total_memory as f64) * 100.0;

        Row::new(vec![
            Cell::from(format!("{:.1}", memory_percent)).style(Style::default().fg(Colors::TEXT)),
            Cell::from(format!("{}MB", memory_mb)).style(Style::default().fg(Colors::TEXT)),
            Cell::from(p.name.clone()).style(Style::default().fg(Colors::TEXT)),
        ])
    });

    let table = Table::new(rows)
        .header(
            Row::new(vec![
                Cell::from("%").style(Style::default().fg(Colors::HEADER)),
                Cell::from("Size").style(Style::default().fg(Colors::HEADER)),
                Cell::from("Process").style(Style::default().fg(Colors::HEADER)),
            ])
            .style(Style::default().add_modifier(Modifier::BOLD)),
        )
        .block(
            Block::default()
                .title(Span::styled(
                    " Top Memory Processes ",
                    Style::default()
                        .fg(Colors::HEADER)
                        .add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Colors::BORDER)),
        )
        .widths(&[
            Constraint::Length(6),
            Constraint::Length(10),
            Constraint::Percentage(70),
        ])
        .column_spacing(1);

    f.render_widget(table, area);
}

fn draw_processes_tab<B: Backend>(f: &mut Frame<B>, app: &App, area: Rect) {
    // Create table header with sort indicators
    let header_cells = vec![
        create_header_cell("PID", SortKey::Pid, app.sort_key, app.sort_ascending),
        create_header_cell("Name", SortKey::Name, app.sort_key, app.sort_ascending),
        create_header_cell("CPU%", SortKey::Cpu, app.sort_key, app.sort_ascending),
        create_header_cell("Memory", SortKey::Memory, app.sort_key, app.sort_ascending),
        create_header_cell("Status", SortKey::Status, app.sort_key, app.sort_ascending),
        create_header_cell("User", SortKey::User, app.sort_key, app.sort_ascending),
        create_header_cell(
            "Started",
            SortKey::StartTime,
            app.sort_key,
            app.sort_ascending,
        ),
    ];

    let header = Row::new(header_cells).style(Style::default().add_modifier(Modifier::BOLD));

    // Create rows with process information
    let rows = app.processes.iter().map(|p| {
        // Color code CPU usage
        let cpu_style = if p.cpu_usage > 50.0 {
            Style::default().fg(Colors::ERROR)
        } else if p.cpu_usage > 20.0 {
            Style::default().fg(Colors::WARNING)
        } else {
            Style::default().fg(Colors::TEXT)
        };

        // Color code memory usage
        let memory_mb = p.memory / 1024 / 1024;
        let memory_style = if memory_mb > 1024 {
            Style::default().fg(Colors::ERROR)
        } else if memory_mb > 512 {
            Style::default().fg(Colors::WARNING)
        } else {
            Style::default().fg(Colors::TEXT)
        };

        // Format process uptime
        let uptime = format_duration(p.start_time);

        Row::new(vec![
            Cell::from(p.pid.to_string()).style(Style::default().fg(Colors::TEXT)),
            Cell::from(p.name.clone()).style(Style::default().fg(Colors::TEXT)),
            Cell::from(format!("{:.1}%", p.cpu_usage)).style(cpu_style),
            Cell::from(format!("{}MB", memory_mb)).style(memory_style),
            Cell::from(p.status.to_string()).style(Style::default().fg(Colors::TEXT)),
            Cell::from(p.user.clone()).style(Style::default().fg(Colors::TEXT)),
            Cell::from(uptime).style(Style::default().fg(Colors::TEXT)),
        ])
    });

    // Create table with header and rows
    let table = Table::new(rows)
        .header(header)
        .block(
            Block::default()
                .title(Span::styled(
                    format!(" Processes ({}) ", app.processes.len()),
                    Style::default()
                        .fg(Colors::HEADER)
                        .add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Colors::BORDER)),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("➤ ")
        .widths(&[
            Constraint::Length(8),
            Constraint::Percentage(25),
            Constraint::Length(8),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Length(12),
            Constraint::Percentage(15),
        ]);

    // Create table state
    let mut state = ratatui::widgets::TableState::default();

    // Set selected item
    if !app.processes.is_empty() {
        state.select(Some(app.selected_index));
    }

    // Render table
    f.render_stateful_widget(table, area, &mut state);
}

fn create_header_cell(text: &str, key: SortKey, current_sort: SortKey, ascending: bool) -> Cell {
    let is_selected = key == current_sort;
    let display_text = if is_selected {
        format!("{} {}", text, if ascending { "↑" } else { "↓" })
    } else {
        text.to_string()
    };

    Cell::from(display_text).style(
        Style::default()
            .fg(if is_selected {
                Colors::HIGHLIGHT
            } else {
                Colors::HEADER
            })
            .add_modifier(Modifier::BOLD),
    )
}

fn draw_user_processes_tab<B: Backend>(f: &mut Frame<B>, app: &App, area: Rect) {
    // Filter processes owned by the current user
    let current_user = if cfg!(unix) {
        std::env::var("USER").unwrap_or_else(|_| "unknown".to_string())
    } else {
        std::env::var("USERNAME").unwrap_or_else(|_| "unknown".to_string())
    };

    let user_processes: Vec<_> = app
        .processes
        .iter()
        .filter(|p| p.user == current_user)
        .collect();

    // Create table header with sort indicators
    let header_cells = vec![
        create_header_cell("PID", SortKey::Pid, app.sort_key, app.sort_ascending),
        create_header_cell("Name", SortKey::Name, app.sort_key, app.sort_ascending),
        create_header_cell("CPU%", SortKey::Cpu, app.sort_key, app.sort_ascending),
        create_header_cell("Memory", SortKey::Memory, app.sort_key, app.sort_ascending),
        create_header_cell("Status", SortKey::Status, app.sort_key, app.sort_ascending),
    ];

    let header = Row::new(header_cells).style(Style::default().add_modifier(Modifier::BOLD));

    // Create rows with process information
    let rows = user_processes.iter().map(|p| {
        // Color code CPU usage
        let cpu_style = if p.cpu_usage > 50.0 {
            Style::default().fg(Colors::ERROR)
        } else if p.cpu_usage > 20.0 {
            Style::default().fg(Colors::WARNING)
        } else {
            Style::default().fg(Colors::TEXT)
        };

        // Color code memory usage
        let memory_mb = p.memory / 1024 / 1024;
        let memory_style = if memory_mb > 1024 {
            Style::default().fg(Colors::ERROR)
        } else if memory_mb > 512 {
            Style::default().fg(Colors::WARNING)
        } else {
            Style::default().fg(Colors::TEXT)
        };

        Row::new(vec![
            Cell::from(p.pid.to_string()).style(Style::default().fg(Colors::TEXT)),
            Cell::from(p.name.clone()).style(Style::default().fg(Colors::TEXT)),
            Cell::from(format!("{:.1}%", p.cpu_usage)).style(cpu_style),
            Cell::from(format!("{}MB", memory_mb)).style(memory_style),
            Cell::from(p.status.to_string()).style(Style::default().fg(Colors::TEXT)),
        ])
    });

    // Create table with header and rows
    let table = Table::new(rows)
        .header(header)
        .block(
            Block::default()
                .title(Span::styled(
                    format!(" User Processes ({}) ", user_processes.len()),
                    Style::default()
                        .fg(Colors::HEADER)
                        .add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Colors::BORDER)),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("➤ ")
        .widths(&[
            Constraint::Length(8),
            Constraint::Percentage(40),
            Constraint::Length(8),
            Constraint::Length(12),
            Constraint::Length(12),
        ]);

    // Create table state
    let mut state = ratatui::widgets::TableState::default();

    // Set selected item
    if !app.processes.is_empty() {
        state.select(Some(app.selected_index));
    }

    // Render table
    f.render_stateful_widget(table, area, &mut state);
}

fn draw_system_processes_tab<B: Backend>(f: &mut Frame<B>, app: &App, area: Rect) {
    // Filter system processes (those not owned by the current user)
    let current_user = if cfg!(unix) {
        std::env::var("USER").unwrap_or_else(|_| "unknown".to_string())
    } else {
        std::env::var("USERNAME").unwrap_or_else(|_| "unknown".to_string())
    };

    let system_processes: Vec<_> = app
        .processes
        .iter()
        .filter(|p| p.user != current_user && p.user != "unknown")
        .collect();

    // Create table header with sort indicators
    let header_cells = vec![
        create_header_cell("PID", SortKey::Pid, app.sort_key, app.sort_ascending),
        create_header_cell("Name", SortKey::Name, app.sort_key, app.sort_ascending),
        create_header_cell("User", SortKey::User, app.sort_key, app.sort_ascending),
        create_header_cell("CPU%", SortKey::Cpu, app.sort_key, app.sort_ascending),
        create_header_cell("Memory", SortKey::Memory, app.sort_key, app.sort_ascending),
    ];

    let header = Row::new(header_cells).style(Style::default().add_modifier(Modifier::BOLD));

    // Create rows with process information
    let rows = system_processes.iter().map(|p| {
        // Color code CPU usage
        let cpu_style = if p.cpu_usage > 50.0 {
            Style::default().fg(Colors::ERROR)
        } else if p.cpu_usage > 20.0 {
            Style::default().fg(Colors::WARNING)
        } else {
            Style::default().fg(Colors::TEXT)
        };

        // Color code memory usage
        let memory_mb = p.memory / 1024 / 1024;
        let memory_style = if memory_mb > 1024 {
            Style::default().fg(Colors::ERROR)
        } else if memory_mb > 512 {
            Style::default().fg(Colors::WARNING)
        } else {
            Style::default().fg(Colors::TEXT)
        };

        Row::new(vec![
            Cell::from(p.pid.to_string()).style(Style::default().fg(Colors::TEXT)),
            Cell::from(p.name.clone()).style(Style::default().fg(Colors::TEXT)),
            Cell::from(p.user.clone()).style(Style::default().fg(Colors::TEXT)),
            Cell::from(format!("{:.1}%", p.cpu_usage)).style(cpu_style),
            Cell::from(format!("{}MB", memory_mb)).style(memory_style),
        ])
    });

    // Create table with header and rows
    let table = Table::new(rows)
        .header(header)
        .block(
            Block::default()
                .title(Span::styled(
                    format!(" System Processes ({}) ", system_processes.len()),
                    Style::default()
                        .fg(Colors::HEADER)
                        .add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Colors::BORDER)),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("➤ ")
        .widths(&[
            Constraint::Length(8),
            Constraint::Percentage(30),
            Constraint::Percentage(20),
            Constraint::Length(8),
            Constraint::Length(12),
        ]);

    // Create table state
    let mut state = ratatui::widgets::TableState::default();

    // Set selected item
    if !app.processes.is_empty() {
        state.select(Some(app.selected_index));
    }

    // Render table
    f.render_stateful_widget(table, area, &mut state);
}

pub fn draw_loading_screen<B: Backend>(f: &mut Frame<B>) {
    let size = f.size();

    // Create a centered area for the loading message
    let loading_area = ratatui::layout::Rect {
        x: size.width / 4,
        y: size.height / 2 - 2,
        width: size.width / 2,
        height: 4,
    };

    // Loading message with a spinner symbol
    let loading_text = vec![
        Spans::from(vec![Span::styled(
            "Starting PSR (Process Status Reporter)",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]),
        Spans::from(vec![Span::raw("")]),
        Spans::from(vec![Span::styled(
            "⣾ Loading system information...",
            Style::default().fg(Color::White),
        )]),
    ];

    let loading_paragraph = Paragraph::new(loading_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(Span::styled(" PSR ", Style::default().fg(Color::Yellow))),
        )
        .alignment(ratatui::layout::Alignment::Center);

    f.render_widget(loading_paragraph, loading_area);
}

fn draw_detailed_view<B: Backend>(f: &mut Frame<B>, app: &App, area: Rect) {
    if app.processes.is_empty() {
        return;
    }

    let selected_process = &app.processes[app.selected_index];

    // Split into two sections - info and charts
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // Format detailed process information
    let run_time = format_duration(selected_process.start_time);

    // Left panel - detailed information
    let info_text = vec![
        Spans::from(vec![
            Span::styled("PID: ", Style::default().fg(Colors::HEADER)),
            Span::styled(
                selected_process.pid.to_string(),
                Style::default().fg(Colors::TEXT),
            ),
        ]),
        Spans::from(vec![
            Span::styled("Name: ", Style::default().fg(Colors::HEADER)),
            Span::styled(&selected_process.name, Style::default().fg(Colors::TEXT)),
        ]),
        Spans::from(vec![
            Span::styled("Command: ", Style::default().fg(Colors::HEADER)),
            Span::styled(
                selected_process.cmd.join(" "),
                Style::default().fg(Colors::TEXT),
            ),
        ]),
        Spans::from(vec![
            Span::styled("CPU Usage: ", Style::default().fg(Colors::HEADER)),
            Span::styled(
                format!("{:.2}%", selected_process.cpu_usage),
                Style::default().fg(Colors::CPU),
            ),
        ]),
        Spans::from(vec![
            Span::styled("Memory: ", Style::default().fg(Colors::HEADER)),
            Span::styled(
                format!("{} MB", selected_process.memory / 1024 / 1024),
                Style::default().fg(Colors::MEMORY),
            ),
        ]),
        Spans::from(vec![
            Span::styled("Status: ", Style::default().fg(Colors::HEADER)),
            Span::styled(
                selected_process.status.to_string(),
                Style::default().fg(Colors::TEXT),
            ),
        ]),
        Spans::from(vec![
            Span::styled("User: ", Style::default().fg(Colors::HEADER)),
            Span::styled(&selected_process.user, Style::default().fg(Colors::TEXT)),
        ]),
        Spans::from(vec![
            Span::styled("Running Time: ", Style::default().fg(Colors::HEADER)),
            Span::styled(run_time, Style::default().fg(Colors::TEXT)),
        ]),
        Spans::from(vec![
            Span::styled("Threads: ", Style::default().fg(Colors::HEADER)),
            Span::styled(
                selected_process
                    .threads
                    .map_or("N/A".to_string(), |t| t.to_string()),
                Style::default().fg(Colors::TEXT),
            ),
        ]),
        Spans::from(vec![
            Span::styled("Parent PID: ", Style::default().fg(Colors::HEADER)),
            Span::styled(
                selected_process
                    .parent
                    .map_or("None".to_string(), |p| p.to_string()),
                Style::default().fg(Colors::TEXT),
            ),
        ]),
    ];

    let info_panel = Paragraph::new(info_text)
        .block(
            Block::default()
                .title(Span::styled(
                    format!(" Process Details: {} ", selected_process.name),
                    Style::default()
                        .fg(Colors::HEADER)
                        .add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Colors::BORDER)),
        )
        .wrap(Wrap { trim: true });

    f.render_widget(info_panel, chunks[0]);

    // Right panel - charts section
    let chart_area = chunks[1];
    let chart_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chart_area);

    // CPU history chart
    let cpu_data: Vec<(f64, f64)> = selected_process
        .cpu_history
        .iter()
        .enumerate()
        .map(|(i, &cpu)| (i as f64, cpu as f64))
        .collect();

    let cpu_dataset = vec![Dataset::default()
        .name("CPU %")
        .marker(Marker::Braille)
        .style(Style::default().fg(Colors::CPU))
        .data(&cpu_data)];

    let cpu_chart = Chart::new(cpu_dataset)
        .block(
            Block::default()
                .title(Span::styled(
                    " CPU Usage ",
                    Style::default()
                        .fg(Colors::HEADER)
                        .add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Colors::BORDER)),
        )
        .x_axis(
            Axis::default()
                .style(Style::default().fg(Colors::TEXT))
                .bounds([0.0, 60.0])
                .labels(vec![]),
        )
        .y_axis(
            Axis::default()
                .style(Style::default().fg(Colors::TEXT))
                .bounds([0.0, 100.0])
                .labels(vec![
                    Span::styled("0%", Style::default().fg(Colors::TEXT)),
                    Span::styled("50%", Style::default().fg(Colors::TEXT)),
                    Span::styled("100%", Style::default().fg(Colors::TEXT)),
                ]),
        );

    f.render_widget(cpu_chart, chart_chunks[0]);

    // Memory gauge and history
    let memory_mb = selected_process.memory / 1024 / 1024;
    let memory_percent =
        (selected_process.memory as f64 / app.system_resources.total_memory as f64) * 100.0;

    // Create sparkline for memory history
    let memory_data: Vec<u64> = selected_process
        .memory_history
        .iter()
        .map(|&mem| mem / (1024 * 1024)) // Convert to MB for display
        .collect();

    let memory_sparkline = Sparkline::default()
        .block(
            Block::default()
                .title(Span::styled(
                    format!(
                        " Memory: {}MB ({:.1}% of total) ",
                        memory_mb, memory_percent
                    ),
                    Style::default()
                        .fg(Colors::HEADER)
                        .add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Colors::BORDER)),
        )
        .data(&memory_data)
        .style(Style::default().fg(Colors::MEMORY));

    f.render_widget(memory_sparkline, chart_chunks[1]);
}

fn draw_help_popup<B: Backend>(f: &mut Frame<B>, _app: &App, area: Rect) {
    // Calculate a centered position for a reasonably sized panel
    let popup_width = 72;
    let popup_height = 30;
    let popup_x = (area.width.saturating_sub(popup_width)) / 2;
    let popup_y = (area.height.saturating_sub(popup_height)) / 2;

    let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

    // Add a fancy dimming overlay for the entire screen with high opacity
    let dim_overlay = Block::default().style(
        Style::default()
            .bg(Color::Rgb(20, 20, 30))
            .fg(Color::Rgb(20, 20, 30)),
    );
    f.render_widget(dim_overlay, area);

    // Create artistic header with logo - ensure proper centering
    let title_text = "PSR - Process Status Reporter";
    let padding_left = (popup_width as usize - title_text.len() - 4) / 2;
    let padding_right = popup_width as usize - 4 - padding_left - title_text.len();

    let header = vec![
        Spans::from(vec![
            Span::styled("╭", Style::default().fg(Color::Rgb(88, 91, 112))),
            Span::styled(
                "─".repeat(popup_width as usize - 2),
                Style::default().fg(Color::Rgb(108, 111, 132)),
            ),
            Span::styled("╮", Style::default().fg(Color::Rgb(88, 91, 112))),
        ]),
        Spans::from(vec![
            Span::styled("│", Style::default().fg(Color::Rgb(88, 91, 112))),
            Span::styled(
                " ".repeat(padding_left),
                Style::default().fg(Color::Rgb(248, 248, 242)),
            ),
            Span::styled(
                "P",
                Style::default()
                    .fg(Color::Rgb(255, 85, 85))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "S",
                Style::default()
                    .fg(Color::Rgb(255, 121, 198))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "R",
                Style::default()
                    .fg(Color::Rgb(189, 147, 249))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" - ", Style::default().fg(Color::Rgb(248, 248, 242))),
            Span::styled(
                "Process Status Reporter",
                Style::default()
                    .fg(Color::Rgb(139, 233, 253))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                " ".repeat(padding_right),
                Style::default().fg(Color::Rgb(248, 248, 242)),
            ),
            Span::styled("  │", Style::default().fg(Color::Rgb(88, 91, 112))),
        ]),
        Spans::from(vec![
            Span::styled("│", Style::default().fg(Color::Rgb(88, 91, 112))),
            Span::styled(
                "─".repeat(popup_width as usize - 2),
                Style::default().fg(Color::Rgb(68, 71, 90)),
            ),
            Span::styled("│", Style::default().fg(Color::Rgb(88, 91, 112))),
        ]),
    ];

    // For the sections, ensure consistent spacing
    let kb_text = "KEYBOARD SHORTCUTS";
    let kb_padding_left = (popup_width as usize - kb_text.len() - 2) / 2;
    let kb_padding_right = popup_width as usize - 2 - kb_padding_left - kb_text.len();

    // Create the help text with improved styling and consistent alignment
    let help_text = vec![
        Spans::from(vec![
            Span::styled("│", Style::default().fg(Color::Rgb(88, 91, 112))),
            Span::styled(" ".repeat(kb_padding_left), Style::default()),
            Span::styled(
                kb_text,
                Style::default()
                    .fg(Color::Rgb(241, 250, 140))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" ".repeat(kb_padding_right), Style::default()),
            Span::styled("│", Style::default().fg(Color::Rgb(88, 91, 112))),
        ]),
        Spans::from(vec![
            Span::styled("│", Style::default().fg(Color::Rgb(88, 91, 112))),
            Span::raw(" "),
            Span::styled(
                "─".repeat(popup_width as usize - 4),
                Style::default().fg(Color::Rgb(108, 111, 132)),
            ),
            Span::raw(" "),
            Span::styled("│", Style::default().fg(Color::Rgb(88, 91, 112))),
        ]),
        // Navigation section - ensure consistent column alignment
        Spans::from(vec![
            Span::styled("│", Style::default().fg(Color::Rgb(88, 91, 112))),
            Span::raw(" "),
            Span::styled(
                "NAVIGATION:",
                Style::default()
                    .fg(Color::Rgb(255, 121, 198))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" ".repeat(popup_width as usize - 14)),
            Span::styled("│", Style::default().fg(Color::Rgb(88, 91, 112))),
        ]),
        Spans::from(vec![
            Span::styled("│", Style::default().fg(Color::Rgb(88, 91, 112))),
            Span::raw("  "),
            Span::styled(
                "↑/↓        - Navigate through the list of processes",
                Style::default()
                    .fg(Color::Rgb(189, 147, 249))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" ".repeat(popup_width as usize - 55)),
            Span::styled("│", Style::default().fg(Color::Rgb(88, 91, 112))),
        ]),
        Spans::from(vec![
            Span::styled("│", Style::default().fg(Color::Rgb(88, 91, 112))),
            Span::raw("  "),
            Span::styled(
                "←/→, Tab   - Switch to the next tab",
                Style::default()
                    .fg(Color::Rgb(189, 147, 249))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" ".repeat(popup_width as usize - 42)),
            Span::styled("   │", Style::default().fg(Color::Rgb(88, 91, 112))),
        ]),
        Spans::from(vec![
            Span::styled("│", Style::default().fg(Color::Rgb(88, 91, 112))),
            Span::raw("  "),
            Span::styled(
                "Shift+Tab  - Switch to the previous tab",
                Style::default()
                    .fg(Color::Rgb(189, 147, 249))
                    .add_modifier(Modifier::BOLD),
            ),
            // Span::raw("  - Switch to the previous tab"),
            Span::raw(" ".repeat(popup_width as usize - 44)),
            Span::styled(" │", Style::default().fg(Color::Rgb(88, 91, 112))),
        ]),
        // Add a space between sections with a subtle separator
        Spans::from(vec![
            Span::styled("│", Style::default().fg(Color::Rgb(88, 91, 112))),
            Span::raw(" "),
            Span::styled(
                "┄".repeat(popup_width as usize - 4),
                Style::default().fg(Color::Rgb(68, 71, 90)),
            ),
            Span::raw(" "),
            Span::styled("│", Style::default().fg(Color::Rgb(88, 91, 112))),
        ]),
        // Sorting section - maintain consistent column alignment
        Spans::from(vec![
            Span::styled("│", Style::default().fg(Color::Rgb(88, 91, 112))),
            Span::raw(" "),
            Span::styled(
                "SORTING:",
                Style::default()
                    .fg(Color::Rgb(255, 121, 198))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" ".repeat(popup_width as usize - 11)),
            Span::styled("│", Style::default().fg(Color::Rgb(88, 91, 112))),
        ]),
        Spans::from(vec![
            Span::styled("│", Style::default().fg(Color::Rgb(88, 91, 112))),
            Span::raw("  "),
            Span::styled(
                "Space      - Toggle between ascending and descending sort",
                Style::default()
                    .fg(Color::Rgb(189, 147, 249))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" ".repeat(popup_width as usize - 61)),
            Span::styled("│", Style::default().fg(Color::Rgb(88, 91, 112))),
        ]),
        Spans::from(vec![
            Span::styled("│", Style::default().fg(Color::Rgb(88, 91, 112))),
            Span::raw("  "),
            Span::styled(
                "Ctrl+1     - Sort processes by Process ID (PID)",
                Style::default()
                    .fg(Color::Rgb(189, 147, 249))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" ".repeat(popup_width as usize - 51)),
            Span::styled("│", Style::default().fg(Color::Rgb(88, 91, 112))),
        ]),
        Spans::from(vec![
            Span::styled("│", Style::default().fg(Color::Rgb(88, 91, 112))),
            Span::raw("  "),
            Span::styled(
                "Ctrl+2     - Sort processes by Name alphabetically",
                Style::default()
                    .fg(Color::Rgb(189, 147, 249))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" ".repeat(popup_width as usize - 54)),
            Span::styled("│", Style::default().fg(Color::Rgb(88, 91, 112))),
        ]),
        Spans::from(vec![
            Span::styled("│", Style::default().fg(Color::Rgb(88, 91, 112))),
            Span::raw("  "),
            Span::styled(
                "Ctrl+3     - Sort processes by CPU usage percentage",
                Style::default()
                    .fg(Color::Rgb(189, 147, 249))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" ".repeat(popup_width as usize - 55)),
            Span::styled("│", Style::default().fg(Color::Rgb(88, 91, 112))),
        ]),
        Spans::from(vec![
            Span::styled("│", Style::default().fg(Color::Rgb(88, 91, 112))),
            Span::raw("  "),
            Span::styled(
                "Ctrl+4     - Sort processes by Memory consumption",
                Style::default()
                    .fg(Color::Rgb(189, 147, 249))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" ".repeat(popup_width as usize - 53)),
            Span::styled("│", Style::default().fg(Color::Rgb(88, 91, 112))),
        ]),
        // Separator
        Spans::from(vec![
            Span::styled("│", Style::default().fg(Color::Rgb(88, 91, 112))),
            Span::raw(" "),
            Span::styled(
                "┄".repeat(popup_width as usize - 4),
                Style::default().fg(Color::Rgb(68, 71, 90)),
            ),
            Span::raw(" "),
            Span::styled("│", Style::default().fg(Color::Rgb(88, 91, 112))),
        ]),
        // Process actions section - keep aligned with previous sections
        Spans::from(vec![
            Span::styled("│", Style::default().fg(Color::Rgb(88, 91, 112))),
            Span::raw(" "),
            Span::styled(
                "PROCESS ACTIONS:",
                Style::default()
                    .fg(Color::Rgb(255, 121, 198))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" ".repeat(popup_width as usize - 19)),
            Span::styled("│", Style::default().fg(Color::Rgb(88, 91, 112))),
        ]),
        Spans::from(vec![
            Span::styled("│", Style::default().fg(Color::Rgb(88, 91, 112))),
            Span::raw("  "),
            Span::styled(
                "Ctrl+r     - Force refresh all process information",
                Style::default()
                    .fg(Color::Rgb(189, 147, 249))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" ".repeat(popup_width as usize - 54)),
            Span::styled("│", Style::default().fg(Color::Rgb(88, 91, 112))),
        ]),
        Spans::from(vec![
            Span::styled("│", Style::default().fg(Color::Rgb(88, 91, 112))),
            Span::raw("  "),
            Span::styled(
                "Ctrl+k     - Terminate (kill) the currently selected process",
                Style::default()
                    .fg(Color::Rgb(189, 147, 249))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" ".repeat(popup_width as usize - 64)),
            Span::styled("│", Style::default().fg(Color::Rgb(88, 91, 112))),
        ]),
        Spans::from(vec![
            Span::styled("│", Style::default().fg(Color::Rgb(88, 91, 112))),
            Span::raw("  "),
            Span::styled(
                "Esc        - Clear filter or close this help screen",
                Style::default()
                    .fg(Color::Rgb(189, 147, 249))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" ".repeat(popup_width as usize - 55)),
            Span::styled("│", Style::default().fg(Color::Rgb(88, 91, 112))),
        ]),
        Spans::from(vec![
            Span::styled("│", Style::default().fg(Color::Rgb(88, 91, 112))),
            Span::raw("  "),
            Span::styled(
                "Ctrl+q     - Exit the application",
                Style::default()
                    .fg(Color::Rgb(189, 147, 249))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" ".repeat(popup_width as usize - 37)),
            Span::styled("│", Style::default().fg(Color::Rgb(88, 91, 112))),
        ]),
        // Separator
        Spans::from(vec![
            Span::styled("│", Style::default().fg(Color::Rgb(88, 91, 112))),
            Span::raw(" "),
            Span::styled(
                "┄".repeat(popup_width as usize - 4),
                Style::default().fg(Color::Rgb(68, 71, 90)),
            ),
            Span::raw(" "),
            Span::styled("│", Style::default().fg(Color::Rgb(88, 91, 112))),
        ]),
        // Filtering section - maintain column alignment
        Spans::from(vec![
            Span::styled("│", Style::default().fg(Color::Rgb(88, 91, 112))),
            Span::raw(" "),
            Span::styled(
                "FILTERING:",
                Style::default()
                    .fg(Color::Rgb(255, 121, 198))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" ".repeat(popup_width as usize - 13)),
            Span::styled("│", Style::default().fg(Color::Rgb(88, 91, 112))),
        ]),
        Spans::from(vec![
            Span::styled("│", Style::default().fg(Color::Rgb(88, 91, 112))),
            Span::raw("  "),
            Span::styled(
                "Any char   - Type characters to filter processes by name",
                Style::default()
                    .fg(Color::Rgb(189, 147, 249))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" ".repeat(popup_width as usize - 60)),
            Span::styled("│", Style::default().fg(Color::Rgb(88, 91, 112))),
        ]),
        Spans::from(vec![
            Span::styled("│", Style::default().fg(Color::Rgb(88, 91, 112))),
            Span::raw("  "),
            Span::styled(
                "Backspace  - Delete the last character from the filter",
                Style::default()
                    .fg(Color::Rgb(189, 147, 249))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" ".repeat(popup_width as usize - 58)),
            Span::styled("│", Style::default().fg(Color::Rgb(88, 91, 112))),
        ]),
        // Bottom separator
        Spans::from(vec![
            Span::styled("│", Style::default().fg(Color::Rgb(88, 91, 112))),
            Span::raw(" "),
            Span::styled(
                "┄".repeat(popup_width as usize - 4),
                Style::default().fg(Color::Rgb(68, 71, 90)),
            ),
            Span::raw(" "),
            Span::styled("│", Style::default().fg(Color::Rgb(88, 91, 112))),
        ]),
        // Footer with close instruction - centered properly
        Spans::from(vec![
            Span::styled("│", Style::default().fg(Color::Rgb(88, 91, 112))),
            Span::raw(" "),
            Span::styled(
                " ".repeat((popup_width as usize - 40) / 2),
                Style::default(),
            ),
            Span::styled("Press ", Style::default().fg(Color::Rgb(248, 248, 242))),
            Span::styled(
                "Esc",
                Style::default()
                    .fg(Color::Rgb(255, 121, 198))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" or ", Style::default().fg(Color::Rgb(248, 248, 242))),
            Span::styled(
                "Ctrl+h",
                Style::default()
                    .fg(Color::Rgb(255, 121, 198))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                " to close this help",
                Style::default().fg(Color::Rgb(248, 248, 242)),
            ),
            Span::styled(
                " ".repeat((popup_width as usize - 44) / 2),
                Style::default(),
            ),
            Span::raw(" "),
            Span::styled("│", Style::default().fg(Color::Rgb(88, 91, 112))),
        ]),
        // Bottom border
        Spans::from(vec![
            Span::styled("╰", Style::default().fg(Color::Rgb(88, 91, 112))),
            Span::styled(
                "─".repeat(popup_width as usize - 2),
                Style::default().fg(Color::Rgb(108, 111, 132)),
            ),
            Span::styled("╯", Style::default().fg(Color::Rgb(88, 91, 112))),
        ]),
    ];

    // Combine header and content with properly aligned rows
    let all_content = [header, help_text].concat();

    // Create the help panel with visible styling
    let help_paragraph = Paragraph::new(all_content)
        .alignment(ratatui::layout::Alignment::Left)
        .style(Style::default().bg(Color::Rgb(40, 42, 54))); // Dark background for the help panel

    // Render the help panel
    f.render_widget(help_paragraph, popup_area);
}

fn format_duration(duration: Duration) -> String {
    let total_secs = duration.as_secs();

    if total_secs < 60 {
        return format!("{}s", total_secs);
    }

    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;

    if hours > 0 {
        format!("{}h {}m {}s", hours, minutes, seconds)
    } else {
        format!("{}m {}s", minutes, seconds)
    }
}
