use std::process::Command;
use std::time::Instant;

type LibcTimeT = i64;

extern "C" {
    fn time(tloc: *mut LibcTimeT) -> LibcTimeT;
    fn localtime(timep: *const LibcTimeT) -> *mut LibcTm;
    fn strftime(
        s: *mut i8,
        max: usize,
        format: *const i8,
        tm: *const LibcTm,
    ) -> usize;
}

#[repr(C)]
struct LibcTm {
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

pub struct Logger;

impl Logger {
    pub fn log(level: &str, msg: &str) {
        let timestamp = Self::timestamp();
        eprintln!("[{}] [{}] {}", timestamp, level, msg);
    }

    fn timestamp() -> String {
        let mut ts = [0u8; 32];
        unsafe {
            let mut now: LibcTimeT = 0;
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
}

pub struct Notifier;

impl Notifier {
    pub fn send_desktop_notification(pid: i32, comm: &str, rss_mb: u64, free_mem_mb: u64) {
        let body = format!(
            "Closed {} (PID {}) using {} MB RAM. Available RAM was {} MB.",
            comm, pid, rss_mb, free_mem_mb
        );
        let _ = Command::new("notify-send")
            .args(["-u", "critical", "Memory Watchdog Alert", &body])
            .status();
    }
}
