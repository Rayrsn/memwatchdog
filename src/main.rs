use std::collections::HashSet;
use std::env;
use std::fs;
use std::io::{self, BufRead};
use std::path::Path;
use std::process::{Command, std::process::id};
use std::thread;
use std::time::{Duration, Instant};

extern "C" {
    fn kill(pid: i32, sig: i32) -> i32;
    fn sysconf(name: i32) -> i64;
    fn getppid() -> i32;
}

const _SC_PAGESIZE: i32 = 30;
const SIGTERM: i32 = 15;
const SIGKILL: i32 = 9;

#[derive(Debug, Clone)]
struct Config {
    threshold_mb: u64,
    interval_ms: u64,
    grace_ms: u64,
    verbose: bool,
    dry_run: bool,
    notify: bool,
    whitelist: HashSet<String>,
}

impl Default for Config {
    fn default() -> Self {
        let default_procs = [
            "systemd", "init", "sshd", "Xorg", "Xwayland",
            "hyprland", "kwin", "mutter", "gnome-shell", "sway",
            "dbus-daemon", "dbus-broker", "pipewire", "wireplumber",
            "antigravity", "agy", "bash", "zsh", "fish", "tmux", "screen",
            "memwatchdog",
        ];

        let mut whitelist = HashSet::new();
        for proc in default_procs {
            whitelist.insert(proc.to_lowercase());
        }

        Config {
            threshold_mb: 200,
            interval_ms: 200,
            grace_ms: 1000,
            verbose: false,
            dry_run: false,
            notify: false,
            whitelist,
        }
    }
}

#[derive(Debug)]
struct TargetProcess {
    pid: i32,
    comm: String,
    rss_mb: u64,
}

fn log_msg(level: &str, msg: &str) {
    let now = chrono_now();
    eprintln!("[{}] [{}] {}", now, level, msg);
}

fn chrono_now() -> String {
    let mut ts = [0u8; 32];
    unsafe {
        let mut now: libc_time_t = 0;
        time(&mut now);
        let tm = localtime(&now);
        if !tm.is_null() {
            let len = strftime(
                ts.as_mut_ptr() as *mut i8,
                ts.len(),
                b"%Y-%m-%d %H:%M:%S\0".as_ptr() as *const i8,
                tm,
            );
            if len > 0 {
                return String::from_utf8_lossy(&ts[..len]).to_string();
            }
        }
    }
    format!("{:?}", Instant::now())
}

#[repr(C)]
type libc_time_t = i64;

extern "C" {
    fn time(tloc: *mut libc_time_t) -> libc_time_t;
    fn localtime(timep: *const libc_time_t) -> *mut libc_tm;
    fn strftime(
        s: *mut i8,
        max: usize,
        format: *const i8,
        tm: *const libc_tm,
    ) -> usize;
}

#[repr(C)]
struct libc_tm {
    tm_sec: i32,
    tm_min: i32,
    tm_hour: i32,
    tm_mday: i32,
    tm_mon: i32,
    tm_year: i32,
    tm_wday: i32,
    tm_yday: i32,
    tm_isdst: i32,
}

/// Reads `/proc/meminfo` and returns available RAM in Megabytes.
fn get_available_memory_mb() -> Option<u64> {
    let file = fs::File::open("/proc/meminfo").ok()?;
    let reader = io::BufReader::new(file);

    let mut mem_avail_kb: Option<u64> = None;
    let mut mem_free_kb: Option<u64> = None;

    for line in reader.lines().flatten() {
        if line.starts_with("MemAvailable:") {
            if let Some(val) = parse_meminfo_line(&line) {
                mem_avail_kb = Some(val);
                break;
            }
        } else if line.starts_with("MemFree:") {
            if let Some(val) = parse_meminfo_line(&line) {
                mem_free_kb = Some(val);
            }
        }
    }

    let target_kb = mem_avail_kb.or(mem_free_kb)?;
    Some(target_kb / 1024)
}

fn parse_meminfo_line(line: &str) -> Option<u64> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() >= 2 {
        parts[1].parse::<u64>().ok()
    } else {
        None
    }
}

/// Reads executable name from `/proc/<pid>/comm`
fn get_process_comm(pid: i32) -> String {
    let path = format!("/proc/{}/comm", pid);
    fs::read_to_string(path)
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string())
}

/// Reads Resident Set Size (RSS) in Megabytes from `/proc/<pid>/statm`
fn get_process_rss_mb(pid: i32, page_size: u64) -> Option<u64> {
    let path = format!("/proc/{}/statm", pid);
    let content = fs::read_to_string(path).ok()?;
    let mut parts = content.split_whitespace();

    let _total_pages = parts.next()?;
    let rss_pages = parts.next()?.parse::<u64>().ok()?;

    let rss_bytes = rss_pages * page_size;
    Some(rss_bytes / (1024 * 1024))
}

/// Scans `/proc` to find non-whitelisted process using maximum RSS memory.
fn find_highest_memory_process(config: &Config, page_size: u64) -> Option<TargetProcess> {
    let proc_dir = fs::read_dir("/proc").ok()?;
    let self_pid = id() as i32;
    let parent_pid = unsafe { getppid() };

    let mut highest_rss: u64 = 0;
    let mut best_target: Option<TargetProcess> = None;

    for entry in proc_dir.flatten() {
        let file_name = entry.file_name();
        let name_str = file_name.to_str()?;

        // Only numeric directories are PIDs
        let pid: i32 = match name_str.parse() {
            Ok(p) => p,
            Err(_) => continue,
        };

        // Skip kernel threads (PID <= 2), self, and parent
        if pid <= 2 || pid == self_pid || pid == parent_pid {
            continue;
        }

        let comm = get_process_comm(pid);
        if comm.is_empty() || config.whitelist.contains(&comm.to_lowercase()) {
            continue;
        }

        if let Some(rss_mb) = get_process_rss_mb(pid, page_size) {
            if rss_mb > highest_rss {
                highest_rss = rss_mb;
                best_target = Some(TargetProcess { pid, comm, rss_mb });
            }
        }
    }

    best_target
}

fn send_desktop_notification(pid: i32, comm: &str, rss_mb: u64, free_mem_mb: u64) {
    let body = format!(
        "Closed {} (PID {}) using {} MB RAM. Available RAM was {} MB.",
        comm, pid, rss_mb, free_mem_mb
    );
    let _ = Command::new("notify-send")
        .args(["-u", "critical", "Memory Watchdog Alert", &body])
        .status();
}

fn terminate_process(target: &TargetProcess, free_mem_mb: u64, config: &Config) {
    log_msg(
        "WARN",
        &format!(
            "LOW MEMORY: Available RAM is {} MB (Threshold: {} MB). Target: {} (PID: {}) using {} MB RAM.",
            free_mem_mb, config.threshold_mb, target.comm, target.pid, target.rss_mb
        ),
    );

    if config.dry_run {
        log_msg(
            "INFO",
            &format!(
                "[DRY RUN] Would send SIGTERM then SIGKILL to PID {} ({}).",
                target.pid, target.comm
            ),
        );
        return;
    }

    log_msg(
        "INFO",
        &format!("Sending SIGTERM to process {} (PID {})...", target.comm, target.pid),
    );

    unsafe {
        if kill(target.pid, SIGTERM) != 0 {
            log_msg(
                "ERROR",
                &format!("Failed to send SIGTERM to PID {}.", target.pid),
            );
            return;
        }
    }

    // Wait during grace period
    let start = Instant::now();
    let grace_duration = Duration::from_millis(config.grace_ms);
    let poll_interval = Duration::from_millis(50);

    while start.elapsed() < grace_duration {
        thread::sleep(poll_interval);
        // Check if process is still alive using kill(pid, 0)
        let is_alive = unsafe { kill(target.pid, 0) == 0 };
        if !is_alive {
            log_msg(
                "INFO",
                &format!("Process {} (PID {}) exited gracefully.", target.comm, target.pid),
            );
            if config.notify {
                send_desktop_notification(target.pid, &target.comm, target.rss_mb, free_mem_mb);
            }
            return;
        }
    }

    // Process didn't exit -> Send SIGKILL
    log_msg(
        "WARN",
        &format!(
            "Process {} (PID {}) did not exit after {} ms. Sending SIGKILL...",
            target.comm, target.pid, config.grace_ms
        ),
    );

    unsafe {
        if kill(target.pid, SIGKILL) == 0 {
            log_msg(
                "INFO",
                &format!("Process {} (PID {}) killed with SIGKILL.", target.comm, target.pid),
            );
            if config.notify {
                send_desktop_notification(target.pid, &target.comm, target.rss_mb, free_mem_mb);
            }
        } else {
            log_msg(
                "ERROR",
                &format!("Failed to send SIGKILL to PID {}.", target.pid),
            );
        }
    }
}

fn print_usage(prog: &str) {
    println!("Memory Watchdog Daemon (Rust High-Performance Edition)");
    println!("Usage: {} [OPTIONS]\n", prog);
    println!("Options:");
    println!("  -t, --threshold <MB>    Set free memory threshold in MB (default: 200)");
    println!("  -i, --interval <ms>     Set check interval in milliseconds (default: 200)");
    println!("  -g, --grace <ms>        Set SIGTERM grace period before SIGKILL in ms (default: 1000)");
    println!("  -e, --exclude <name>    Add process name to whitelist (can be repeated)");
    println!("  -n, --notify            Send desktop notification via notify-send when process is closed");
    println!("      --dry-run           Monitor without killing any processes");
    println!("  -v, --verbose           Enable verbose debug output");
    println!("  -h, --help              Show this help message");
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut config = Config::default();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-t" | "--threshold" => {
                if i + 1 < args.len() {
                    config.threshold_mb = args[i + 1].parse().unwrap_or(200);
                    i += 1;
                }
            }
            "-i" | "--interval" => {
                if i + 1 < args.len() {
                    config.interval_ms = args[i + 1].parse().unwrap_or(200);
                    i += 1;
                }
            }
            "-g" | "--grace" => {
                if i + 1 < args.len() {
                    config.grace_ms = args[i + 1].parse().unwrap_or(1000);
                    i += 1;
                }
            }
            "-e" | "--exclude" => {
                if i + 1 < args.len() {
                    config.whitelist.insert(args[i + 1].to_lowercase());
                    i += 1;
                }
            }
            "-n" | "--notify" => {
                config.notify = true;
            }
            "--dry-run" => {
                config.dry_run = true;
            }
            "-v" | "--verbose" => {
                config.verbose = true;
            }
            "-h" | "--help" => {
                print_usage(&args[0]);
                return;
            }
            _ => {
                eprintln!("Unknown option: {}", args[i]);
                print_usage(&args[0]);
                return;
            }
        }
        i += 1;
    }

    let page_size_raw = unsafe { sysconf(_SC_PAGESIZE) };
    let page_size = if page_size_raw > 0 {
        page_size_raw as u64
    } else {
        4096
    };

    log_msg(
        "INFO",
        &format!(
            "Rust Memory Watchdog started. Threshold: {} MB, Interval: {} ms, Dry-Run: {}",
            config.threshold_mb, config.interval_ms, config.dry_run
        ),
    );

    let sleep_duration = Duration::from_millis(config.interval_ms);

    loop {
        if let Some(free_mem_mb) = get_available_memory_mb() {
            if config.verbose {
                log_msg("DEBUG", &format!("Available memory: {} MB", free_mem_mb));
            }

            if free_mem_mb < config.threshold_mb {
                if let Some(target) = find_highest_memory_process(&config, page_size) {
                    terminate_process(&target, free_mem_mb, &config);
                } else {
                    log_msg(
                        "WARN",
                        &format!(
                            "Memory low ({} MB < {} MB), but no non-whitelisted target process found.",
                            free_mem_mb, config.threshold_mb
                        ),
                    );
                }
            }
        }

        thread::sleep(sleep_duration);
    }
}
