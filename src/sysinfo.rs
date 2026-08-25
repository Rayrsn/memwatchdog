use std::fs::File;
use std::io::{BufRead, BufReader};

extern "C" {
    fn sysconf(name: i32) -> i64;
}

const _SC_PAGESIZE: i32 = 30;

#[derive(Debug, Clone, Copy, Default)]
pub struct MemoryStats {
    pub total_mb: u64,
    pub available_mb: u64,
    pub free_mb: u64,
    pub cached_mb: u64,
}

pub struct SysInfo;

impl SysInfo {
    /// Reads `/proc/meminfo` and returns current MemoryStats
    pub fn get_memory_stats() -> Option<MemoryStats> {
        let file = File::open("/proc/meminfo").ok()?;
        let reader = BufReader::new(file);

        let mut total_kb: Option<u64> = None;
        let mut avail_kb: Option<u64> = None;
        let mut free_kb: Option<u64> = None;
        let mut cached_kb: Option<u64> = None;

        for line in reader.lines().flatten() {
            if line.starts_with("MemTotal:") {
                total_kb = parse_meminfo_line(&line);
            } else if line.starts_with("MemAvailable:") {
                avail_kb = parse_meminfo_line(&line);
            } else if line.starts_with("MemFree:") {
                free_kb = parse_meminfo_line(&line);
            } else if line.starts_with("Cached:") {
                cached_kb = parse_meminfo_line(&line);
            }
        }

        let total_kb = total_kb?;
        let free_kb = free_kb.unwrap_or(0);
        let avail_kb = avail_kb.unwrap_or(free_kb);
        let cached_kb = cached_kb.unwrap_or(0);

        Some(MemoryStats {
            total_mb: total_kb / 1024,
            available_mb: avail_kb / 1024,
            free_mb: free_kb / 1024,
            cached_mb: cached_kb / 1024,
        })
    }

    /// Gets memory page size in bytes (usually 4096)
    pub fn page_size() -> u64 {
        let size = unsafe { sysconf(_SC_PAGESIZE) };
        if size > 0 {
            size as u64
        } else {
            4096
        }
    }
}

fn parse_meminfo_line(line: &str) -> Option<u64> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() >= 2 {
        parts[1].parse::<u64>().ok()
    } else {
        None
    }
}
