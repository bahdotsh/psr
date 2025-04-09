use std::collections::{HashMap, HashSet};
use std::process::Command;
use std::time::{Duration, Instant};
use sysinfo::CpuExt;
use sysinfo::{PidExt, ProcessExt, System, SystemExt};

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProcessStatus {
    Running,
    Sleeping,
    Stopped,
    Zombie,
    Unknown,
}

impl std::fmt::Display for ProcessStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProcessStatus::Running => write!(f, "Running"),
            ProcessStatus::Sleeping => write!(f, "Sleeping"),
            ProcessStatus::Stopped => write!(f, "Stopped"),
            ProcessStatus::Zombie => write!(f, "Zombie"),
            ProcessStatus::Unknown => write!(f, "Unknown"),
        }
    }
}

#[derive(Clone)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub cpu_usage: f32,
    pub memory: u64,
    pub status: ProcessStatus,
    pub user: String,
    pub start_time: Duration,
    pub cmd: Vec<String>,
    pub threads: Option<usize>,
    pub parent: Option<u32>,
    // History for graphs
    pub cpu_history: Vec<f32>,
    pub memory_history: Vec<u64>,
    pub last_updated: Instant,
}

impl ProcessInfo {
    fn new(
        pid: u32,
        name: String,
        cpu_usage: f32,
        memory: u64,
        status: ProcessStatus,
        user: String,
        start_time: Duration,
        cmd: Vec<String>,
        threads: Option<usize>,
        parent: Option<u32>,
    ) -> Self {
        Self {
            pid,
            name,
            cpu_usage,
            memory,
            status,
            user,
            start_time,
            cmd,
            threads,
            parent,
            cpu_history: vec![cpu_usage],
            memory_history: vec![memory],
            last_updated: Instant::now(),
        }
    }

    pub fn update_history(&mut self, cpu: f32, memory: u64) {
        // Keep only last 60 data points for charts
        if self.cpu_history.len() >= 60 {
            self.cpu_history.remove(0);
            self.memory_history.remove(0);
        }

        self.cpu_usage = cpu;
        self.memory = memory;
        self.cpu_history.push(cpu);
        self.memory_history.push(memory);
        self.last_updated = Instant::now();
    }
}

// Cache for user information to reduce system calls
struct UserCache {
    cache: HashMap<u32, String>,
    last_refresh: Instant,
}

impl UserCache {
    fn new() -> Self {
        Self {
            cache: HashMap::new(),
            last_refresh: Instant::now(),
        }
    }

    fn get_user(&mut self, pid: u32) -> String {
        // Refresh cache every 30 seconds
        if self.last_refresh.elapsed() > Duration::from_secs(30) {
            self.cache.clear();
            self.last_refresh = Instant::now();
        }

        if let Some(user) = self.cache.get(&pid) {
            return user.clone();
        }

        let user = if cfg!(unix) {
            Command::new("ps")
                .args(&["-o", "user=", "-p", &pid.to_string()])
                .output()
                .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
                .unwrap_or_else(|_| "unknown".to_string())
        } else {
            "unknown".to_string() // Simplified for Windows
        };

        self.cache.insert(pid, user.clone());
        user
    }
}

// Thread cache to avoid expensive operations
struct ThreadCache {
    cache: HashMap<u32, usize>,
    last_refresh: Instant,
}

impl ThreadCache {
    fn new() -> Self {
        Self {
            cache: HashMap::new(),
            last_refresh: Instant::now(),
        }
    }

    fn get_thread_count(&mut self, pid: u32) -> Option<usize> {
        // Only refresh thread counts every 5 seconds
        if self.last_refresh.elapsed() > Duration::from_secs(5) {
            self.cache.clear();
            self.last_refresh = Instant::now();
        }

        if let Some(count) = self.cache.get(&pid) {
            return Some(*count);
        }

        if cfg!(unix) {
            let thread_count = Command::new("ps")
                .args(&["-o", "nlwp=", "-p", &pid.to_string()])
                .output()
                .ok()
                .and_then(|output| {
                    String::from_utf8_lossy(&output.stdout)
                        .trim()
                        .parse::<usize>()
                        .ok()
                });

            if let Some(count) = thread_count {
                self.cache.insert(pid, count);
            }

            thread_count
        } else {
            None
        }
    }
}

pub struct ProcessMonitor {
    system: System,
    user_cache: UserCache,
    thread_cache: ThreadCache,
    process_cache: HashMap<u32, ProcessInfo>,
    last_full_refresh: Instant,
}

impl ProcessMonitor {
    pub fn new() -> Self {
        let mut system = System::new_all();
        system.refresh_all();

        Self {
            system,
            user_cache: UserCache::new(),
            thread_cache: ThreadCache::new(),
            process_cache: HashMap::new(),
            last_full_refresh: Instant::now(),
        }
    }

    // Efficient process data collection
    pub fn get_processes(&mut self) -> Vec<ProcessInfo> {
        // Full refresh every 10 seconds, partial refresh for CPU/memory otherwise
        let is_full_refresh = self.last_full_refresh.elapsed() > Duration::from_secs(10);

        if is_full_refresh {
            self.system.refresh_all();
            self.last_full_refresh = Instant::now();
        } else {
            self.system.refresh_processes();
            self.system.refresh_cpu();
            self.system.refresh_memory();
        }

        let mut processes = Vec::new();
        let mut active_pids = HashSet::new();

        for (pid, process) in self.system.processes() {
            let pid_u32 = pid.as_u32();
            active_pids.insert(pid_u32);

            // Convert status
            let status = match process.status() {
                sysinfo::ProcessStatus::Run => ProcessStatus::Running,
                sysinfo::ProcessStatus::Sleep => ProcessStatus::Sleeping,
                sysinfo::ProcessStatus::Stop => ProcessStatus::Stopped,
                sysinfo::ProcessStatus::Zombie => ProcessStatus::Zombie,
                _ => ProcessStatus::Unknown,
            };

            // Only fetch expensive information on full refresh
            let (user, threads, parent) =
                if is_full_refresh || !self.process_cache.contains_key(&pid_u32) {
                    (
                        self.user_cache.get_user(pid_u32),
                        self.thread_cache.get_thread_count(pid_u32),
                        process.parent().map(|p| p.as_u32()),
                    )
                } else if let Some(cached) = self.process_cache.get(&pid_u32) {
                    (cached.user.clone(), cached.threads, cached.parent)
                } else {
                    ("unknown".to_string(), None, None)
                };

            // Update existing process or create new
            if let Some(cached_process) = self.process_cache.get_mut(&pid_u32) {
                cached_process.update_history(process.cpu_usage(), process.memory());

                // Only update these fields on full refresh
                if is_full_refresh {
                    cached_process.status = status;
                    cached_process.user = user;
                    cached_process.threads = threads;
                    cached_process.parent = parent;
                    cached_process.cmd = process.cmd().to_vec();
                }

                processes.push(cached_process.clone());
            } else {
                // New process
                let process_info = ProcessInfo::new(
                    pid_u32,
                    process.name().to_string(),
                    process.cpu_usage(),
                    process.memory(),
                    status,
                    user,
                    Duration::from_secs(process.run_time()),
                    process.cmd().to_vec(),
                    threads,
                    parent,
                );
                self.process_cache.insert(pid_u32, process_info.clone());
                processes.push(process_info);
            }
        }

        // Clean up processes that no longer exist
        self.process_cache
            .retain(|pid, _| active_pids.contains(pid));

        processes
    }

    pub fn kill_process(&self, pid: u32) -> bool {
        if cfg!(unix) {
            Command::new("kill")
                .arg("-9")
                .arg(pid.to_string())
                .status()
                .map(|status| status.success())
                .unwrap_or(false)
        } else if cfg!(windows) {
            Command::new("taskkill")
                .args(&["/F", "/PID", &pid.to_string()])
                .status()
                .map(|status| status.success())
                .unwrap_or(false)
        } else {
            false
        }
    }

    // Get system-wide CPU and memory information
    pub fn get_system_info(&mut self) -> (f32, u64, u64) {
        self.system.refresh_cpu();
        self.system.refresh_memory();

        let cpu_usage = self.system.global_cpu_info().cpu_usage();
        let total_memory = self.system.total_memory();
        let used_memory = self.system.used_memory();

        (cpu_usage, used_memory, total_memory)
    }
}
