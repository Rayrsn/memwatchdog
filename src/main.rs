mod config;
mod notifier;
mod process;
mod sysinfo;
mod terminator;
mod watchdog;

use config::Config;
use watchdog::WatchdogEngine;

fn main() {
    let config = Config::parse();
    let mut engine = WatchdogEngine::new(config);
    engine.run();
}
