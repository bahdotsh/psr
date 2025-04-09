use crate::processes::{ProcessInfo, ProcessMonitor};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq)]
pub enum SortKey {
    Pid,
    Name,
    Cpu,
    Memory,
    Status,
    User,
    StartTime,
}
#[allow(dead_code)]
impl SortKey {
    pub fn as_str(&self) -> &'static str {
        match self {
            SortKey::Pid => "PID",
            SortKey::Name => "Name",
            SortKey::Cpu => "CPU%",
            SortKey::Memory => "Memory",
            SortKey::Status => "Status",
            SortKey::User => "User",
            SortKey::StartTime => "Start Time",
        }
    }
}

pub struct SystemResources {
    pub cpu_usage: f32,
    pub used_memory: u64,
    pub total_memory: u64,
    pub cpu_history: Vec<f32>,
    pub memory_history: Vec<f32>, // Percentage of memory used
}

impl SystemResources {
    pub fn new() -> Self {
        Self {
            cpu_usage: 0.0,
            used_memory: 0,
            total_memory: 1, // Avoid division by zero
            cpu_history: vec![0.0; 60],
            memory_history: vec![0.0; 60],
        }
    }

    pub fn update(&mut self, cpu: f32, used: u64, total: u64) {
        self.cpu_usage = cpu;
        self.used_memory = used;
        self.total_memory = total;

        // Update history
        if self.cpu_history.len() >= 60 {
            self.cpu_history.remove(0);
            self.memory_history.remove(0);
        }

        self.cpu_history.push(cpu);
        let memory_percent = (used as f32 / total as f32) * 100.0;
        self.memory_history.push(memory_percent);
    }

    pub fn memory_percentage(&self) -> f32 {
        (self.used_memory as f32 / self.total_memory as f32) * 100.0
    }
}

pub struct App {
    pub process_monitor: ProcessMonitor,
    pub processes: Vec<ProcessInfo>,
    pub selected_index: usize,
    pub previous_selected_pid: Option<u32>, // Track selected process between updates
    pub current_tab: usize,
    pub tabs: Vec<&'static str>,
    pub sort_key: SortKey,
    pub sort_ascending: bool,
    pub system_resources: SystemResources,
    last_ui_refresh: Instant,
    last_data_refresh: Instant,
    ui_refresh_interval: Duration,
    data_refresh_interval: Duration,
    pub filter: String,
    pub show_help: bool,
}

impl App {
    pub fn new(process_monitor: ProcessMonitor) -> Self {
        let mut app = Self {
            process_monitor,
            processes: Vec::new(),
            selected_index: 0,
            previous_selected_pid: None,
            current_tab: 0,
            tabs: vec!["Dashboard", "All Processes", "User", "System", "Detailed"],
            sort_key: SortKey::Cpu,
            sort_ascending: false,
            system_resources: SystemResources::new(),
            last_ui_refresh: Instant::now(),
            last_data_refresh: Instant::now(),
            ui_refresh_interval: Duration::from_millis(33), // ~30fps
            data_refresh_interval: Duration::from_millis(1000), // 1 second data updates
            filter: String::new(),
            show_help: false,
        };
        app.refresh_all_data();
        app
    }

    pub fn next(&mut self) {
        if !self.processes.is_empty() {
            self.previous_selected_pid = Some(self.processes[self.selected_index].pid);
            self.selected_index = (self.selected_index + 1) % self.processes.len();
        }
    }

    pub fn previous(&mut self) {
        if !self.processes.is_empty() {
            self.previous_selected_pid = Some(self.processes[self.selected_index].pid);
            self.selected_index = if self.selected_index > 0 {
                self.selected_index - 1
            } else {
                self.processes.len() - 1
            };
        }
    }

    pub fn next_tab(&mut self) {
        self.current_tab = (self.current_tab + 1) % self.tabs.len();
    }

    pub fn previous_tab(&mut self) {
        self.current_tab = if self.current_tab > 0 {
            self.current_tab - 1
        } else {
            self.tabs.len() - 1
        };
    }

    pub fn toggle_help(&mut self) {
        self.show_help = !self.show_help;
    }

    // Smoothly update data without disrupting navigation
    pub fn refresh_all_data(&mut self) {
        // Remember which process was selected
        let selected_pid = if !self.processes.is_empty() {
            Some(self.processes[self.selected_index].pid)
        } else {
            self.previous_selected_pid
        };

        // Update system resource info
        let (cpu, used_mem, total_mem) = self.process_monitor.get_system_info();
        self.system_resources.update(cpu, used_mem, total_mem);

        // Get updated processes
        self.processes = self.process_monitor.get_processes();

        // Apply filtering if needed
        if !self.filter.is_empty() {
            let filter = self.filter.to_lowercase();
            self.processes.retain(|p| {
                p.name.to_lowercase().contains(&filter)
                    || p.pid.to_string().contains(&filter)
                    || p.user.to_lowercase().contains(&filter)
            });
        }

        // Sort processes
        self.sort_processes();

        // Maintain selection through updates
        if let Some(pid) = selected_pid {
            if let Some(index) = self.processes.iter().position(|p| p.pid == pid) {
                self.selected_index = index;
            } else if !self.processes.is_empty() {
                self.selected_index = 0;
            }
        }

        self.last_data_refresh = Instant::now();
    }

    pub fn should_refresh_ui(&self) -> bool {
        self.last_ui_refresh.elapsed() >= self.ui_refresh_interval
    }

    pub fn should_refresh_data(&self) -> bool {
        self.last_data_refresh.elapsed() >= self.data_refresh_interval
    }

    pub fn refresh_ui(&mut self) {
        self.last_ui_refresh = Instant::now();
    }

    pub fn toggle_sort(&mut self) {
        self.sort_ascending = !self.sort_ascending;
        self.sort_processes();
    }

    pub fn set_sort_key(&mut self, key: SortKey) {
        if self.sort_key == key {
            self.sort_ascending = !self.sort_ascending;
        } else {
            self.sort_key = key;
            self.sort_ascending = false; // Default to descending for new sort key
        }
        self.sort_processes();
    }

    pub fn sort_processes(&mut self) {
        match self.sort_key {
            SortKey::Pid => {
                self.processes.sort_by(|a, b| {
                    if self.sort_ascending {
                        a.pid.cmp(&b.pid)
                    } else {
                        b.pid.cmp(&a.pid)
                    }
                });
            }
            SortKey::Name => {
                self.processes.sort_by(|a, b| {
                    if self.sort_ascending {
                        a.name.cmp(&b.name)
                    } else {
                        b.name.cmp(&a.name)
                    }
                });
            }
            SortKey::Cpu => {
                self.processes.sort_by(|a, b| {
                    if self.sort_ascending {
                        a.cpu_usage
                            .partial_cmp(&b.cpu_usage)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    } else {
                        b.cpu_usage
                            .partial_cmp(&a.cpu_usage)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    }
                });
            }
            SortKey::Memory => {
                self.processes.sort_by(|a, b| {
                    if self.sort_ascending {
                        a.memory
                            .partial_cmp(&b.memory)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    } else {
                        b.memory
                            .partial_cmp(&a.memory)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    }
                });
            }
            SortKey::Status => {
                self.processes.sort_by(|a, b| {
                    if self.sort_ascending {
                        a.status.cmp(&b.status)
                    } else {
                        b.status.cmp(&a.status)
                    }
                });
            }
            SortKey::User => {
                self.processes.sort_by(|a, b| {
                    if self.sort_ascending {
                        a.user.cmp(&b.user)
                    } else {
                        b.user.cmp(&a.user)
                    }
                });
            }
            SortKey::StartTime => {
                self.processes.sort_by(|a, b| {
                    if self.sort_ascending {
                        a.start_time.cmp(&b.start_time)
                    } else {
                        b.start_time.cmp(&a.start_time)
                    }
                });
            }
        }
    }

    pub fn kill_selected_process(&mut self) {
        if !self.processes.is_empty() {
            let pid = self.processes[self.selected_index].pid;
            self.process_monitor.kill_process(pid);
            self.refresh_all_data();
        }
    }

    pub fn add_to_filter(&mut self, c: char) {
        self.filter.push(c);
        self.refresh_all_data(); // Apply filter immediately
    }

    pub fn backspace_filter(&mut self) {
        self.filter.pop();
        self.refresh_all_data(); // Apply filter immediately
    }

    // Get the top CPU and memory processes for dashboard
    pub fn top_processes(&self, count: usize) -> (Vec<&ProcessInfo>, Vec<&ProcessInfo>) {
        let mut cpu_sorted = self.processes.iter().collect::<Vec<_>>();
        let mut mem_sorted = self.processes.iter().collect::<Vec<_>>();

        cpu_sorted.sort_by(|a, b| {
            b.cpu_usage
                .partial_cmp(&a.cpu_usage)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        mem_sorted.sort_by(|a, b| {
            b.memory
                .partial_cmp(&a.memory)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        (
            cpu_sorted.into_iter().take(count).collect(),
            mem_sorted.into_iter().take(count).collect(),
        )
    }
}
