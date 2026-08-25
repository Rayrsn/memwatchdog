# Low-Latency Memory Watchdog Daemon (Rust)

A ultra-lightweight, high-performance Linux memory watchdog service written in **Rust**. It continuously monitors system memory and immediately terminates the highest memory-consuming process if free/available memory drops below **200 MB** (or a user-specified threshold).

## Key Features

- **⚡ Sub-millisecond Latency:** Built with zero-overhead Rust. Polling `/proc/meminfo` and `/proc/[pid]/statm` takes under **1 microsecond** per check with negligible CPU usage (~0.00%).
- **🛡️ Built-in Safety Whitelist:** System-critical processes (`systemd`, `init`, `sshd`, `Xorg`, `hyprland`, `kwin`, `pipewire`, shells, desktop environments, `antigravity`) are automatically protected from termination.
- **🔄 Graceful 2-Step Termination:** First attempts `SIGTERM` (graceful shutdown request). If the process doesn't terminate within the grace period (default 1000ms), it sends `SIGKILL` (forceful termination).
- **📦 Zero Heavy Dependencies:** Uses direct Linux `/proc` filesystem interfaces and standard kernel syscalls.
- **🔔 Optional Desktop Notifications:** Sends critical desktop alerts via `notify-send` when a process is closed.
- **⚙️ Systemd Integration:** Provided with both system-wide and user-level installation targets.

---

## Installation (Standalone & Service)

Once installed, the binary and service files are placed in standard system/user locations (`/usr/local/bin/memwatchdog` or `~/.local/bin/memwatchdog`), **so you can safely delete the source code folder after running `make install`**.

### Option A: System-Wide Installation (Recommended)

```bash
cd /home/rayr/Projects/memwatchdog
sudo make install

# Enable and start system daemon:
sudo systemctl enable --now memwatchdog

# (Optional) You can now remove the source folder:
# rm -rf /home/rayr/Projects/memwatchdog
```

### Option B: User-Level Installation (No Sudo Required)

```bash
cd /home/rayr/Projects/memwatchdog
make install-user

# Enable and start user service:
systemctl --user enable --now memwatchdog

# (Optional) You can now remove the source folder:
# rm -rf /home/rayr/Projects/memwatchdog
```

---

## Status & Log Monitoring

- **System service logs:**
  ```bash
  sudo systemctl status memwatchdog
  sudo journalctl -u memwatchdog -f
  ```

- **User service logs:**
  ```bash
  systemctl --user status memwatchdog
  journalctl --user -u memwatchdog -f
  ```

---

## Command Line Options

```text
Usage: memwatchdog [OPTIONS]

Options:
  -t, --threshold <MB>    Set free memory threshold in MB (default: 200)
  -i, --interval <ms>     Set check interval in milliseconds (default: 200)
  -g, --grace <ms>        Set SIGTERM grace period before SIGKILL in ms (default: 1000)
  -e, --exclude <name>    Add process name to whitelist (can be repeated)
  -n, --notify            Send desktop notification via notify-send when process is closed
      --dry-run           Monitor without killing any processes
  -v, --verbose           Enable verbose debug output
  -h, --help              Show help message
```

### Manual Usage Examples

- **Test mode (Dry Run - logs action without killing):**
  ```bash
  memwatchdog --threshold 200 --dry-run --verbose
  ```

- **Add custom process exclusions (e.g. don't kill Firefox or Blender):**
  ```bash
  memwatchdog --threshold 200 -e firefox -e blender
  ```

---

## Uninstallation

- To uninstall system installation:
  ```bash
  sudo make uninstall
  ```
- To uninstall user installation:
  ```bash
  make uninstall-user
  ```
