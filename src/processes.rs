use std::collections::{HashMap, HashSet};
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};
use sysinfo::{CpuExt, PidExt, ProcessExt, System, SystemExt};
use tokio::sync::mpsc::{self, Sender};
use tokio::sync::Mutex;
use tokio::task;
use tokio::time::interval;

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

// Updates that can be sent from the background task
#[derive(Clone)]
pub enum ProcessUpdate {
    ProcessList(Vec<ProcessInfo>),
    SystemInfo(f32, u64, u64), // cpu, used_mem, total_mem
    LoadingStatus(String),
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

    async fn get_user(&mut self, pid: u32) -> String {
        // Refresh cache every 30 seconds
        if self.last_refresh.elapsed() > Duration::from_secs(30) {
            self.cache.clear();
            self.last_refresh = Instant::now();
        }

        if let Some(user) = self.cache.get(&pid) {
            return user.clone();
        }

        let user = if cfg!(unix) {
            // Use spawn_blocking to avoid blocking the async runtime
            let pid_str = pid.to_string();
            match task::spawn_blocking(move || {
                Command::new("ps")
                    .args(&["-o", "user=", "-p", &pid_str])
                    .output()
            })
            .await
            {
                Ok(Ok(output)) => {
                    let username = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    if username.is_empty() {
                        "unknown".to_string()
                    } else {
                        username
                    }
                }
                _ => "unknown".to_string(),
            }
        } else {
            "unknown".to_string() // Fallback for non-Unix systems
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

    async fn get_thread_count(&mut self, pid: u32) -> Option<usize> {
        // Only refresh thread counts every 5 seconds
        if self.last_refresh.elapsed() > Duration::from_secs(5) {
            self.cache.clear();
            self.last_refresh = Instant::now();
        }

        if let Some(count) = self.cache.get(&pid) {
            return Some(*count);
        }

        if cfg!(unix) {
            let pid_str = pid.to_string();
            let thread_count = tokio::task::spawn_blocking(move || {
                Command::new("ps")
                    .args(&["-o", "nlwp=", "-p", &pid_str])
                    .output()
                    .ok()
                    .and_then(|output| {
                        String::from_utf8_lossy(&output.stdout)
                            .trim()
                            .parse::<usize>()
                            .ok()
                    })
            })
            .await
            .ok()
            .flatten();

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
    system: Arc<Mutex<System>>,
    user_cache: Arc<Mutex<UserCache>>,
    thread_cache: Arc<Mutex<ThreadCache>>,
    process_cache: Arc<Mutex<HashMap<u32, ProcessInfo>>>,
    last_full_refresh: Arc<Mutex<Instant>>,
    tx: Sender<ProcessUpdate>,
    refresh_receiver: mpsc::Receiver<()>,
}

const BATCH_SIZE: usize = 50; // Process information in batches

impl ProcessMonitor {
    pub fn new(tx: Sender<ProcessUpdate>) -> (Self, mpsc::Sender<()>) {
        let mut system = System::new_all();
        system.refresh_all();

        // Create a channel for requesting refreshes
        let (refresh_tx, refresh_rx) = mpsc::channel(10);

        // Store the refresh sender in the app
        let clone_tx = tx.clone();
        tokio::spawn(async move {
            // Provide the refresh sender to the app
            let update = ProcessUpdate::LoadingStatus("Starting up...".to_string());
            let _ = clone_tx.send(update).await;
        });

        let monitor = Self {
            system: Arc::new(Mutex::new(system)),
            user_cache: Arc::new(Mutex::new(UserCache::new())),
            thread_cache: Arc::new(Mutex::new(ThreadCache::new())),
            process_cache: Arc::new(Mutex::new(HashMap::new())),
            last_full_refresh: Arc::new(Mutex::new(Instant::now())),
            tx,
            refresh_receiver: refresh_rx,
        };

        (monitor, refresh_tx)
    }

    pub fn get_refresh_sender(&self) -> mpsc::Sender<()> {
        let (tx, _) = mpsc::channel(10); // Create a dummy sender
        tx
    }

    pub async fn start_monitoring(mut self) {
        // First, send initial loading message
        let _ = self
            .tx
            .send(ProcessUpdate::LoadingStatus(
                "Initializing system monitor...".to_string(),
            ))
            .await;

        // Start with a system info update
        {
            let system = self.system.lock().await;
            let cpu_usage = system.global_cpu_info().cpu_usage();
            let total_memory = system.total_memory();
            let used_memory = system.used_memory();
            let _ = self
                .tx
                .send(ProcessUpdate::SystemInfo(
                    cpu_usage,
                    used_memory,
                    total_memory,
                ))
                .await;
        }

        // Initial process list
        self.collect_and_send_processes(true).await;

        // Clear loading status after initial load is complete
        let _ = self
            .tx
            .send(ProcessUpdate::LoadingStatus("".to_string()))
            .await;

        // Now start regular monitoring
        let mut interval_timer = interval(Duration::from_millis(1000));

        loop {
            tokio::select! {
                // Check for refresh requests
                _ = self.refresh_receiver.recv() => {
                    let _ = self.tx.send(ProcessUpdate::LoadingStatus("Manual refresh requested...".to_string())).await;
                    self.collect_and_send_processes(true).await;
                }

                // Regular timer-based updates
                _ = interval_timer.tick() => {
                    self.collect_and_send_processes(false).await;

                    // Update system info every tick
                    let system = self.system.lock().await;
                    let cpu_usage = system.global_cpu_info().cpu_usage();
                    let total_memory = system.total_memory();
                    let used_memory = system.used_memory();
                    let _ = self.tx.send(ProcessUpdate::SystemInfo(cpu_usage, used_memory, total_memory)).await;
                }
            }
        }
    }

    async fn collect_and_send_processes(&self, force_full_refresh: bool) {
        // Determine if we need a full refresh
        let mut last_full_refresh = self.last_full_refresh.lock().await;
        let is_full_refresh =
            force_full_refresh || last_full_refresh.elapsed() > Duration::from_secs(10);

        if is_full_refresh {
            let _ = self
                .tx
                .send(ProcessUpdate::LoadingStatus(
                    "Collecting process data...".to_string(),
                ))
                .await;

            // Full refresh of system data
            {
                let mut system = self.system.lock().await;
                system.refresh_all();
            }
            *last_full_refresh = Instant::now();
        } else {
            // Partial refresh
            let mut system = self.system.lock().await;
            system.refresh_processes();
            system.refresh_cpu();
            system.refresh_memory();
        }

        // Process information
        let processes = self.get_processes(is_full_refresh).await;

        // Send the updated process list
        let _ = self.tx.send(ProcessUpdate::ProcessList(processes)).await;

        // Clear loading status once done
        if is_full_refresh {
            let _ = self
                .tx
                .send(ProcessUpdate::LoadingStatus("".to_string()))
                .await;
        }
    }

    // Get processes in an async-friendly way
    async fn get_processes(&self, is_full_refresh: bool) -> Vec<ProcessInfo> {
        let mut process_cache = self.process_cache.lock().await;
        let mut processes = Vec::new();
        let mut active_pids = HashSet::new();

        // Collect process data first while holding the lock
        let system_processes: Vec<(
            sysinfo::Pid,
            Vec<String>,
            String,
            f32,
            u64,
            sysinfo::ProcessStatus,
            u64,
            Option<sysinfo::Pid>,
        )> = {
            let system = self.system.lock().await;
            system
                .processes()
                .iter()
                .map(|(pid, process)| {
                    (
                        *pid,
                        process.cmd().to_vec(),
                        process.name().to_string(),
                        process.cpu_usage(),
                        process.memory(),
                        process.status(),
                        process.run_time(),
                        process.parent(),
                    )
                })
                .collect()
        };

        // Process in batches to avoid blocking for too long
        for chunk in system_processes.chunks(BATCH_SIZE) {
            let mut batch_processes = Vec::with_capacity(chunk.len());

            for &(pid, ref cmd, ref name, cpu_usage, memory, status, run_time, parent) in chunk {
                let pid_u32 = pid.as_u32();
                active_pids.insert(pid_u32);

                // Convert status
                let status = match status {
                    sysinfo::ProcessStatus::Run => ProcessStatus::Running,
                    sysinfo::ProcessStatus::Sleep => ProcessStatus::Sleeping,
                    sysinfo::ProcessStatus::Stop => ProcessStatus::Stopped,
                    sysinfo::ProcessStatus::Zombie => ProcessStatus::Zombie,
                    _ => ProcessStatus::Unknown,
                };

                // Only fetch expensive information on full refresh
                let (user, threads, parent_pid) =
                    if is_full_refresh || !process_cache.contains_key(&pid_u32) {
                        let user = if is_full_refresh {
                            let mut user_cache = self.user_cache.lock().await;
                            user_cache.get_user(pid_u32).await
                        } else {
                            "fetching...".to_string()
                        };

                        let threads = if is_full_refresh {
                            let mut thread_cache = self.thread_cache.lock().await;
                            thread_cache.get_thread_count(pid_u32).await
                        } else {
                            None
                        };

                        (user, threads, parent.map(|p| p.as_u32()))
                    } else if let Some(cached) = process_cache.get(&pid_u32) {
                        (cached.user.clone(), cached.threads, cached.parent)
                    } else {
                        ("unknown".to_string(), None, None)
                    };

                // Update existing process or create new
                if let Some(cached_process) = process_cache.get_mut(&pid_u32) {
                    cached_process.update_history(cpu_usage, memory);

                    // Only update these fields on full refresh
                    if is_full_refresh {
                        cached_process.status = status;
                        cached_process.user = user;
                        cached_process.threads = threads;
                        cached_process.parent = parent_pid;
                        cached_process.cmd = cmd.clone();
                    }

                    batch_processes.push(cached_process.clone());
                } else {
                    // New process
                    let process_info = ProcessInfo::new(
                        pid_u32,
                        name.clone(),
                        cpu_usage,
                        memory,
                        status,
                        user,
                        Duration::from_secs(run_time),
                        cmd.clone(),
                        threads,
                        parent_pid,
                    );
                    process_cache.insert(pid_u32, process_info.clone());
                    batch_processes.push(process_info);
                }
            }

            processes.extend(batch_processes);

            // Small delay between batches to avoid blocking UI
            if chunk.len() == BATCH_SIZE {
                // No need to drop system here anymore
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        }

        // Clean up processes that no longer exist
        process_cache.retain(|pid, _| active_pids.contains(pid));

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
}
