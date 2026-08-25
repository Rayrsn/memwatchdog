use crate::config::{Config, SelectionPolicy};
use std::fs;
use std::process::id;

extern "C" {
    fn getppid() -> i32;
}

#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub pid: i32,
    pub comm: String,
    pub rss_mb: u64,
}

pub struct ProcessScanner;

impl ProcessScanner {
    /// Scans active processes in `/proc` and selects optimal termination target.
    pub fn find_target_process(config: &Config, page_size: u64) -> Option<ProcessInfo> {
        let proc_dir = fs::read_dir("/proc").ok()?;
        let self_pid = id() as i32;
        let parent_pid = unsafe { getppid() };

        let mut highest_rss: u64 = 0;
        let mut best_target: Option<ProcessInfo> = None;

        let min_rss_requirement = match config.policy {
            SelectionPolicy::MinRssThreshold(min_mb) => min_mb,
            SelectionPolicy::HighestRss => 0,
        };

        for entry in proc_dir.flatten() {
            let file_name = entry.file_name();
            let name_str = match file_name.to_str() {
                Some(s) => s,
                None => continue,
            };

            let pid: i32 = match name_str.parse() {
                Ok(p) => p,
                Err(_) => continue,
            };

            // Exclude kernel threads (PID <= 2), watchdog itself, and parent
            if pid <= 2 || pid == self_pid || pid == parent_pid {
                continue;
            }

            let comm = Self::read_comm(pid);
            if comm.is_empty() || config.whitelist.contains(&comm.to_lowercase()) {
                continue;
            }

            if let Some(rss_mb) = Self::read_rss_mb(pid, page_size) {
                if rss_mb < min_rss_requirement {
                    continue;
                }

                if rss_mb > highest_rss {
                    highest_rss = rss_mb;
                    best_target = Some(ProcessInfo { pid, comm, rss_mb });
                }
            }
        }

        best_target
    }

    fn read_comm(pid: i32) -> String {
        let path = format!("/proc/{}/comm", pid);
        fs::read_to_string(path)
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|_| String::new())
    }

    fn read_rss_mb(pid: i32, page_size: u64) -> Option<u64> {
        let path = format!("/proc/{}/statm", pid);
        let content = fs::read_to_string(path).ok()?;
        let mut parts = content.split_whitespace();

        let _total_pages = parts.next()?;
        let rss_pages = parts.next()?.parse::<u64>().ok()?;

        let rss_bytes = rss_pages * page_size;
        Some(rss_bytes / (1024 * 1024))
    }
}
