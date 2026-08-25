use crate::config::Config;
use crate::notifier::{Logger, Notifier};
use crate::process::ProcessScanner;
use crate::sysinfo::SysInfo;
use crate::terminator::ProcessTerminator;
use std::thread;
use std::time::Duration;

pub struct WatchdogEngine {
    config: Config,
    page_size: u64,
}

impl WatchdogEngine {
    pub fn new(config: Config) -> Self {
        let page_size = SysInfo::page_size();
        WatchdogEngine { config, page_size }
    }

    pub fn run(&mut self) {
        Logger::log(
            "INFO",
            &format!(
                "Watchdog Engine started. Critical Threshold: {} MB, Warning: {} MB, Interval: {} ms, Dry-Run: {}",
                self.config.critical_threshold_mb,
                self.config.warning_threshold_mb,
                self.config.interval_ms,
                self.config.dry_run
            ),
        );

        let sleep_duration = Duration::from_millis(self.config.interval_ms);
        let mut last_warned = false;

        loop {
            if let Some(stats) = SysInfo::get_memory_stats() {
                if self.config.verbose {
                    Logger::log(
                        "DEBUG",
                        &format!(
                            "RAM Stats -> Total: {} MB | Available: {} MB | Free: {} MB | Cached: {} MB",
                            stats.total_mb, stats.available_mb, stats.free_mb, stats.cached_mb
                        ),
                    );
                }

                if stats.available_mb < self.config.critical_threshold_mb {
                    if let Some(target) = ProcessScanner::find_target_process(&self.config, self.page_size) {
                        ProcessTerminator::terminate(&target, stats.available_mb, &self.config);
                    } else {
                        Logger::log(
                            "WARN",
                            &format!(
                                "Critical RAM low ({} MB < {} MB), but no non-whitelisted candidate process found.",
                                stats.available_mb, self.config.critical_threshold_mb
                            ),
                        );
                    }
                    last_warned = false;
                } else if stats.available_mb < self.config.warning_threshold_mb {
                    if !last_warned {
                        Logger::log(
                            "WARN",
                            &format!(
                                "Memory Warning: Available RAM is {} MB (< Warning Threshold {} MB).",
                                stats.available_mb, self.config.warning_threshold_mb
                            ),
                        );
                        if self.config.notify {
                            Notifier::send_warning_notification(
                                stats.available_mb,
                                self.config.warning_threshold_mb,
                            );
                        }
                        last_warned = true;
                    }
                } else {
                    last_warned = false;
                }
            }

            thread::sleep(sleep_duration);
        }
    }
}
