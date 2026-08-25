#define _GNU_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <dirent.h>
#include <signal.h>
#include <fcntl.h>
#include <time.h>
#include <errno.h>
#include <stdbool.h>
#include <stdarg.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <getopt.h>

#define DEFAULT_THRESHOLD_MB 200
#define DEFAULT_INTERVAL_MS 200
#define DEFAULT_GRACE_MS 1000
#define MAX_WHITELIST 64
#define MAX_PATH 512
#define MAX_COMM 256

typedef struct {
    long threshold_mb;
    int interval_ms;
    int grace_ms;
    bool verbose;
    bool dry_run;
    bool daemon_mode;
    bool notify;
    char *whitelist[MAX_WHITELIST];
    int whitelist_count;
} Config;

static volatile bool running = true;

// Default whitelisted processes essential for system stability
static const char *default_whitelist[] = {
    "systemd", "init", "sshd", "Xorg", "Xwayland",
    "hyprland", "kwin", "mutter", "gnome-shell", "sway",
    "dbus-daemon", "dbus-broker", "pipewire", "wireplumber",
    "antigravity", "agy", "bash", "zsh", "fish", "tmux", "screen",
    NULL
};

static void handle_signal(int sig) {
    (void)sig;
    running = false;
}

static void log_msg(const char *level, const char *fmt, ...) {
    time_t now = time(NULL);
    struct tm tm_buf;
    localtime_r(&now, &tm_buf);
    char time_str[32];
    strftime(time_str, sizeof(time_str), "%Y-%m-%d %H:%M:%S", &tm_buf);

    va_list args;
    va_start(args, fmt);
    fprintf(stderr, "[%s] [%s] ", time_str, level);
    vfprintf(stderr, fmt, args);
    fprintf(stderr, "\n");
    va_end(args);
}

// Get available memory in Megabytes from /proc/meminfo
static long get_available_memory_mb(void) {
    FILE *f = fopen("/proc/meminfo", "r");
    if (!f) {
        log_msg("ERROR", "Failed to open /proc/meminfo: %s", strerror(errno));
        return -1;
    }

    char line[256];
    long mem_avail_kb = -1;
    long mem_free_kb = -1;

    while (fgets(line, sizeof(line), f)) {
        if (sscanf(line, "MemAvailable: %ld kB", &mem_avail_kb) == 1) {
            break;
        }
        if (sscanf(line, "MemFree: %ld kB", &mem_free_kb) == 1) {
            // fallback if MemAvailable not found immediately
        }
    }
    fclose(f);

    long target_kb = (mem_avail_kb != -1) ? mem_avail_kb : mem_free_kb;
    if (target_kb == -1) {
        return -1;
    }

    return target_kb / 1024;
}

static bool is_whitelisted(const char *comm, const Config *config) {
    if (!comm || strlen(comm) == 0) return true;

    // Check user whitelist
    for (int i = 0; i < config->whitelist_count; i++) {
        if (strcasecmp(comm, config->whitelist[i]) == 0) {
            return true;
        }
    }

    // Check default whitelist
    for (int i = 0; default_whitelist[i] != NULL; i++) {
        if (strcasecmp(comm, default_whitelist[i]) == 0) {
            return true;
        }
    }

    return false;
}

// Get process command/name from /proc/<pid>/comm
static void get_process_comm(pid_t pid, char *buf, size_t size) {
    char path[MAX_PATH];
    snprintf(path, sizeof(path), "/proc/%d/comm", pid);
    FILE *f = fopen(path, "r");
    if (f) {
        if (fgets(buf, size, f)) {
            size_t len = strlen(buf);
            if (len > 0 && buf[len - 1] == '\n') {
                buf[len - 1] = '\0';
            }
        } else {
            snprintf(buf, size, "unknown");
        }
        fclose(f);
    } else {
        snprintf(buf, size, "unknown");
    }
}

// Get Process Resident Set Size (RSS) in Megabytes
static long get_process_rss_mb(pid_t pid, long page_size) {
    char path[MAX_PATH];
    snprintf(path, sizeof(path), "/proc/%d/statm", pid);
    FILE *f = fopen(path, "r");
    if (!f) return -1;

    long size_pages, rss_pages;
    if (fscanf(f, "%ld %ld", &size_pages, &rss_pages) != 2) {
        fclose(f);
        return -1;
    }
    fclose(f);

    long rss_bytes = rss_pages * page_size;
    return rss_bytes / (1024 * 1024);
}

typedef struct {
    pid_t pid;
    char comm[MAX_COMM];
    long rss_mb;
} TargetProcess;

// Find process with highest RSS memory usage
static bool find_highest_memory_process(const Config *config, TargetProcess *target) {
    DIR *dir = opendir("/proc");
    if (!dir) {
        log_msg("ERROR", "Failed to open /proc directory: %s", strerror(errno));
        return false;
    }

    pid_t self_pid = getpid();
    pid_t parent_pid = getppid();
    long page_size = sysconf(_SC_PAGESIZE);

    target->pid = -1;
    target->rss_mb = -1;
    target->comm[0] = '\0';

    struct dirent *entry;
    while ((entry = readdir(dir)) != NULL) {
        // PIDs are directory names containing digits
        if (entry->d_type != DT_DIR) continue;

        char *endptr;
        pid_t pid = strtol(entry->d_name, &endptr, 10);
        if (*endptr != '\0' || pid <= 2) continue; // Skip kernel threads (PID <= 2) and non-PIDs
        if (pid == self_pid || pid == parent_pid) continue; // Skip self & parent

        char comm[MAX_COMM];
        get_process_comm(pid, comm, sizeof(comm));

        if (is_whitelisted(comm, config)) {
            continue;
        }

        long rss_mb = get_process_rss_mb(pid, page_size);
        if (rss_mb > target->rss_mb) {
            target->rss_mb = rss_mb;
            target->pid = pid;
            strncpy(target->comm, comm, sizeof(target->comm) - 1);
            target->comm[sizeof(target->comm) - 1] = '\0';
        }
    }

    closedir(dir);
    return target->pid != -1;
}

static void send_desktop_notification(pid_t pid, const char *comm, long rss_mb, long free_mem_mb) {
    char cmd[512];
    snprintf(cmd, sizeof(cmd),
             "notify-send -u critical 'Memory Watchdog Alert' "
             "'Closed %s (PID %d) using %ld MB RAM. Free RAM was %ld MB.' 2>/dev/null",
             comm, pid, rss_mb, free_mem_mb);
    int ret = system(cmd);
    (void)ret;
}

static void terminate_process(const TargetProcess *target, long free_mem_mb, const Config *config) {
    log_msg("WARN", "CRITICAL MEMORY: Available RAM is %ld MB (Threshold: %ld MB). Target: %s (PID: %d) using %ld MB RAM.",
            free_mem_mb, config->threshold_mb, target->comm, target->pid, target->rss_mb);

    if (config->dry_run) {
        log_msg("INFO", "[DRY RUN] Would send SIGTERM then SIGKILL to PID %d (%s).", target->pid, target->comm);
        return;
    }

    log_msg("INFO", "Sending SIGTERM to process %s (PID %d)...", target->comm, target->pid);
    if (kill(target->pid, SIGTERM) != 0) {
        log_msg("ERROR", "Failed to send SIGTERM to PID %d: %s", target->pid, strerror(errno));
        return;
    }

    // Grace period wait
    int elapsed = 0;
    int check_step = 50; // ms
    while (elapsed < config->grace_ms) {
        usleep(check_step * 1000);
        elapsed += check_step;
        if (kill(target->pid, 0) != 0) { // Process no longer exists
            log_msg("INFO", "Process %s (PID %d) terminated gracefully.", target->comm, target->pid);
            if (config->notify) send_desktop_notification(target->pid, target->comm, target->rss_mb, free_mem_mb);
            return;
        }
    }

    // Process still alive after grace period -> SIGKILL
    log_msg("WARN", "Process %s (PID %d) did not exit after %d ms. Sending SIGKILL...",
            target->comm, target->pid, config->grace_ms);
    if (kill(target->pid, SIGKILL) == 0) {
        log_msg("INFO", "Process %s (PID %d) forcefully killed with SIGKILL.", target->comm, target->pid);
        if (config->notify) send_desktop_notification(target->pid, target->comm, target->rss_mb, free_mem_mb);
    } else {
        log_msg("ERROR", "Failed to send SIGKILL to PID %d: %s", target->pid, strerror(errno));
    }
}

static void print_usage(const char *prog) {
    printf("Memory Watchdog Daemon - Low Latency RAM Protector\n");
    printf("Usage: %s [OPTIONS]\n\n", prog);
    printf("Options:\n");
    printf("  -t, --threshold <MB>    Set free memory threshold in MB (default: %d)\n", DEFAULT_THRESHOLD_MB);
    printf("  -i, --interval <ms>     Set check interval in milliseconds (default: %d)\n", DEFAULT_INTERVAL_MS);
    printf("  -g, --grace <ms>        Set SIGTERM grace period before SIGKILL in ms (default: %d)\n", DEFAULT_GRACE_MS);
    printf("  -e, --exclude <name>    Add process name to whitelist (can be specified multiple times)\n");
    printf("  -d, --daemon            Run in daemon background mode\n");
    printf("  -n, --notify            Send desktop notification via notify-send when a process is closed\n");
    printf("      --dry-run           Monitor without actually killing any process\n");
    printf("  -v, --verbose           Enable verbose log messages\n");
    printf("  -h, --help              Show this help message\n");
}

int main(int argc, char *argv[]) {
    Config config = {
        .threshold_mb = DEFAULT_THRESHOLD_MB,
        .interval_ms = DEFAULT_INTERVAL_MS,
        .grace_ms = DEFAULT_GRACE_MS,
        .verbose = false,
        .dry_run = false,
        .daemon_mode = false,
        .notify = false,
        .whitelist_count = 0
    };

    static struct option long_options[] = {
        {"threshold", required_argument, 0, 't'},
        {"interval",  required_argument, 0, 'i'},
        {"grace",     required_argument, 0, 'g'},
        {"exclude",   required_argument, 0, 'e'},
        {"daemon",    no_argument,       0, 'd'},
        {"notify",    no_argument,       0, 'n'},
        {"dry-run",   no_argument,       0, 1000},
        {"verbose",   no_argument,       0, 'v'},
        {"help",      no_argument,       0, 'h'},
        {0, 0, 0, 0}
    };

    int opt;
    while ((opt = getopt_long(argc, argv, "t:i:g:e:dnvh", long_options, NULL)) != -1) {
        switch (opt) {
            case 't':
                config.threshold_mb = atol(optarg);
                break;
            case 'i':
                config.interval_ms = atoi(optarg);
                break;
            case 'g':
                config.grace_ms = atoi(optarg);
                break;
            case 'e':
                if (config.whitelist_count < MAX_WHITELIST) {
                    config.whitelist[config.whitelist_count++] = strdup(optarg);
                }
                break;
            case 'd':
                config.daemon_mode = true;
                break;
            case 'n':
                config.notify = true;
                break;
            case 1000:
                config.dry_run = true;
                break;
            case 'v':
                config.verbose = true;
                break;
            case 'h':
                print_usage(argv[0]);
                return 0;
            default:
                print_usage(argv[0]);
                return 1;
        }
    }

    if (config.daemon_mode) {
        if (daemon(0, 0) != 0) {
            log_msg("ERROR", "Failed to daemonize: %s", strerror(errno));
            return 1;
        }
    }

    signal(SIGINT, handle_signal);
    signal(SIGTERM, handle_signal);

    log_msg("INFO", "Memory Watchdog started. Threshold: %ld MB, Interval: %d ms, Dry-Run: %s",
            config.threshold_mb, config.interval_ms, config.dry_run ? "YES" : "NO");

    useconds_t sleep_us = config.interval_ms * 1000;

    while (running) {
        long free_mem_mb = get_available_memory_mb();

        if (free_mem_mb >= 0) {
            if (config.verbose) {
                log_msg("DEBUG", "Available memory: %ld MB", free_mem_mb);
            }

            if (free_mem_mb < config.threshold_mb) {
                TargetProcess target;
                if (find_highest_memory_process(&config, &target)) {
                    terminate_process(&target, free_mem_mb, &config);
                } else {
                    log_msg("WARN", "Memory low (%ld MB < %ld MB), but no suitable target process found to terminate.",
                            free_mem_mb, config.threshold_mb);
                }
            }
        }

        usleep(sleep_us);
    }

    log_msg("INFO", "Memory Watchdog shutting down gracefully.");

    for (int i = 0; i < config.whitelist_count; i++) {
        free(config.whitelist[i]);
    }

    return 0;
}
