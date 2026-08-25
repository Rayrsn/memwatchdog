use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionPolicy {
    HighestRss,
    MinRssThreshold(u64), // Only target processes using at least X MB RAM
}

#[derive(Debug, Clone)]
pub struct Config {
    pub critical_threshold_mb: u64,
    pub warning_threshold_mb: u64,
    pub interval_ms: u64,
    pub grace_ms: u64,
    pub policy: SelectionPolicy,
    pub verbose: bool,
    pub dry_run: bool,
    pub notify: bool,
    pub whitelist: HashSet<String>,
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
            critical_threshold_mb: 200,
            warning_threshold_mb: 500,
            interval_ms: 200,
            grace_ms: 1000,
            policy: SelectionPolicy::HighestRss,
            verbose: false,
            dry_run: false,
            notify: false,
            whitelist,
        }
    }
}

impl Config {
    pub fn parse() -> Self {
        let args: Vec<String> = env::args().collect();
        let mut config = Config::default();

        // Load config file if present
        config.load_config_file("/etc/memwatchdog.conf");
        if let Ok(home) = env::var("HOME") {
            let user_conf = format!("{}/.config/memwatchdog.conf", home);
            config.load_config_file(&user_conf);
        }

        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "-t" | "--threshold" => {
                    if i + 1 < args.len() {
                        if let Ok(val) = args[i + 1].parse() {
                            config.critical_threshold_mb = val;
                        }
                        i += 1;
                    }
                }
                "-w" | "--warning-threshold" => {
                    if i + 1 < args.len() {
                        if let Ok(val) = args[i + 1].parse() {
                            config.warning_threshold_mb = val;
                        }
                        i += 1;
                    }
                }
                "-i" | "--interval" => {
                    if i + 1 < args.len() {
                        if let Ok(val) = args[i + 1].parse() {
                            config.interval_ms = val;
                        }
                        i += 1;
                    }
                }
                "-g" | "--grace" => {
                    if i + 1 < args.len() {
                        if let Ok(val) = args[i + 1].parse() {
                            config.grace_ms = val;
                        }
                        i += 1;
                    }
                }
                "-e" | "--exclude" => {
                    if i + 1 < args.len() {
                        config.whitelist.insert(args[i + 1].to_lowercase());
                        i += 1;
                    }
                }
                "-m" | "--min-target-rss" => {
                    if i + 1 < args.len() {
                        if let Ok(val) = args[i + 1].parse() {
                            config.policy = SelectionPolicy::MinRssThreshold(val);
                        }
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
                "-c" | "--config" => {
                    if i + 1 < args.len() {
                        config.load_config_file(&args[i + 1]);
                        i += 1;
                    }
                }
                "-h" | "--help" => {
                    print_usage(&args[0]);
                    std::process::exit(0);
                }
                _ => {
                    eprintln!("Unknown option: {}", args[i]);
                    print_usage(&args[0]);
                    std::process::exit(1);
                }
            }
            i += 1;
        }

        config
    }

    fn load_config_file<P: AsRef<Path>>(&mut self, path: P) {
        let path = path.as_ref();
        if !path.exists() {
            return;
        }

        if let Ok(content) = fs::read_to_string(path) {
            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }

                let parts: Vec<&str> = line.splitn(2, '=').collect();
                if parts.len() == 2 {
                    let key = parts[0].trim().to_lowercase();
                    let val = parts[1].trim();

                    match key.as_str() {
                        "threshold_mb" => {
                            if let Ok(v) = val.parse() {
                                self.critical_threshold_mb = v;
                            }
                        }
                        "warning_threshold_mb" => {
                            if let Ok(v) = val.parse() {
                                self.warning_threshold_mb = v;
                            }
                        }
                        "interval_ms" => {
                            if let Ok(v) = val.parse() {
                                self.interval_ms = v;
                            }
                        }
                        "grace_ms" => {
                            if let Ok(v) = val.parse() {
                                self.grace_ms = v;
                            }
                        }
                        "notify" => {
                            self.notify = val.eq_ignore_ascii_case("true") || val == "1";
                        }
                        "exclude" => {
                            for item in val.split(',') {
                                let item = item.trim().to_lowercase();
                                if !item.is_empty() {
                                    self.whitelist.insert(item);
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

pub fn print_usage(prog: &str) {
    println!("Memory Watchdog Daemon (Modular Rust Edition)");
    println!("Usage: {} [OPTIONS]\n", prog);
    println!("Options:");
    println!("  -t, --threshold <MB>         Critical free memory threshold in MB (default: 200)");
    println!("  -w, --warning-threshold <MB> Warning free memory threshold in MB (default: 500)");
    println!("  -i, --interval <ms>          Check interval in milliseconds (default: 200)");
    println!("  -g, --grace <ms>             SIGTERM grace period before SIGKILL in ms (default: 1000)");
    println!("  -m, --min-target-rss <MB>    Only target processes using at least X MB RAM");
    println!("  -e, --exclude <name>         Add process name to whitelist (can be repeated)");
    println!("  -c, --config <file>          Load configuration file");
    println!("  -n, --notify                 Send desktop notification via notify-send");
    println!("      --dry-run                Monitor without killing any processes");
    println!("  -v, --verbose                Enable verbose debug output");
    println!("  -h, --help                   Show this help message");
}
