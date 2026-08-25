use crate::config::Config;
use crate::notifier::{Logger, Notifier};
use crate::process::ProcessInfo;
use std::thread;
use std::time::{Duration, Instant};

extern "C" {
    fn kill(pid: i32, sig: i32) -> i32;
}

const SIGTERM: i32 = 15;
const SIGKILL: i32 = 9;

pub struct ProcessTerminator;

impl ProcessTerminator {
    pub fn terminate(target: &ProcessInfo, free_mem_mb: u64, config: &Config) {
        Logger::log(
            "WARN",
            &format!(
                "CRITICAL MEMORY: Available RAM is {} MB (Threshold: {} MB). Target: {} (PID: {}) using {} MB RAM.",
                free_mem_mb, config.critical_threshold_mb, target.comm, target.pid, target.rss_mb
            ),
        );

        if config.dry_run {
            Logger::log(
                "INFO",
                &format!(
                    "[DRY RUN] Would send SIGTERM then SIGKILL to PID {} ({}).",
                    target.pid, target.comm
                ),
            );
            return;
        }

        Logger::log(
            "INFO",
            &format!("Sending SIGTERM to process {} (PID {})...", target.comm, target.pid),
        );

        unsafe {
            if kill(target.pid, SIGTERM) != 0 {
                Logger::log("ERROR", &format!("Failed to send SIGTERM to PID {}.", target.pid));
                return;
            }
        }

        // Wait grace period
        let start = Instant::now();
        let grace_duration = Duration::from_millis(config.grace_ms);
        let poll_interval = Duration::from_millis(50);

        while start.elapsed() < grace_duration {
            thread::sleep(poll_interval);
            let is_alive = unsafe { kill(target.pid, 0) == 0 };
            if !is_alive {
                Logger::log(
                    "INFO",
                    &format!("Process {} (PID {}) exited gracefully.", target.comm, target.pid),
                );
                if config.notify {
                    Notifier::send_desktop_notification(target.pid, &target.comm, target.rss_mb, free_mem_mb);
                }
                return;
            }
        }

        // Send SIGKILL if process refuses SIGTERM
        Logger::log(
            "WARN",
            &format!(
                "Process {} (PID {}) did not exit after {} ms. Sending SIGKILL...",
                target.comm, target.pid, config.grace_ms
            ),
        );

        unsafe {
            if kill(target.pid, SIGKILL) == 0 {
                Logger::log(
                    "INFO",
                    &format!("Process {} (PID {}) killed with SIGKILL.", target.comm, target.pid),
                );
                if config.notify {
                    Notifier::send_desktop_notification(target.pid, &target.comm, target.rss_mb, free_mem_mb);
                }
            } else {
                Logger::log("ERROR", &format!("Failed to send SIGKILL to PID {}.", target.pid));
            }
        }
    }
}
