# Low-Latency Modular Memory Watchdog Daemon (Rust)

A highly modular, scalable, and ultra-lightweight Linux memory watchdog daemon written in **Rust**. It continuously monitors system memory and immediately terminates the highest memory-consuming process if free/available memory drops below **200 MB** (or a user-specified threshold).

---

## 🏗️ Modular Architecture

The codebase is organized into decoupled, single-responsibility modules:

- [`src/config.rs`](file:///home/rayr/Projects/memwatchdog/src/config.rs): Handles CLI parsing, config file parsing (`/etc/memwatchdog.conf` & `~/.config/memwatchdog.conf`), default whitelists, and runtime policies.
- [`src/sysinfo.rs`](file:///home/rayr/Projects/memwatchdog/src/sysinfo.rs): High-performance `/proc/meminfo` parser and kernel page-size detection.
- [`src/process.rs`](file:///home/rayr/Projects/memwatchdog/src/process.rs): Active process scanner, PID whitelist filter, and process selection policies.
- [`src/terminator.rs`](file:///home/rayr/Projects/memwatchdog/src/terminator.rs): Graceful termination logic (`SIGTERM` $\rightarrow$ grace wait $\rightarrow$ `SIGKILL`).
- [`src/notifier.rs`](file:///home/rayr/Projects/memwatchdog/src/notifier.rs): Desktop alerts (`notify-send`) and formatted timestamp logging.
- [`src/watchdog.rs`](file:///home/rayr/Projects/memwatchdog/src/watchdog.rs): Main engine event loop and multi-tier monitoring orchestrator.

---

## ⚡ Scalability & Enterprise Features

1. **Multi-Tier Threshold Alerts:**
   - **Warning Tier:** Logs memory warnings when free RAM falls below warning threshold (default: `500 MB`).
   - **Critical Tier:** Triggers process termination when free RAM drops below critical threshold (default: `200 MB`).

2. **Configuration File Support (`/etc/memwatchdog.conf` or `~/.config/memwatchdog.conf`):**
   ```ini
   threshold_mb=200
   warning_threshold_mb=500
   interval_ms=200
   grace_ms=1000
   notify=true
   exclude=firefox,chrome,blender
   ```

3. **Flexible Process Selection Policies:**
   - Default: Terminate process using highest RSS RAM.
   - Filtered mode (`-m 100` / `--min-target-rss 100`): Only target processes consuming at least 100 MB RAM to prevent killing minor background helper processes.

4. **Safety Protection Whitelist:**
   Automatically protects system-critical processes:
   - `systemd`, `init`, `sshd`, `Xorg`, `Xwayland`, `hyprland`, `kwin`, `mutter`, `gnome-shell`, `sway`, `pipewire`, `antigravity`, shells (`bash`, `zsh`, `fish`), and terminal multiplexers (`tmux`, `screen`).

---

## 🛠️ Build & Installation

### Option A: System-Wide Installation (Recommended)

```bash
cd /home/rayr/Projects/memwatchdog
sudo make install

# Enable and start system daemon:
sudo systemctl enable --now memwatchdog

# (Optional) Remove source directory:
# rm -rf /home/rayr/Projects/memwatchdog
```

### Option B: User-Level Installation (No Sudo Required)

```bash
cd /home/rayr/Projects/memwatchdog
make install-user

# Enable and start user service:
systemctl --user enable --now memwatchdog

# (Optional) Remove source directory:
# rm -rf /home/rayr/Projects/memwatchdog
```

---

## 🖥️ Command Line Usage

```text
Usage: memwatchdog [OPTIONS]

Options:
  -t, --threshold <MB>         Critical free memory threshold in MB (default: 200)
  -w, --warning-threshold <MB> Warning free memory threshold in MB (default: 500)
  -i, --interval <ms>          Check interval in milliseconds (default: 200)
  -g, --grace <ms>             SIGTERM grace period before SIGKILL in ms (default: 1000)
  -m, --min-target-rss <MB>    Only target processes using at least X MB RAM
  -e, --exclude <name>         Add process name to whitelist (can be repeated)
  -c, --config <file>          Load custom configuration file
  -n, --notify                 Send desktop notification via notify-send
      --dry-run                Monitor without killing any processes
  -v, --verbose                Enable verbose debug output
  -h, --help                   Show help message
```
