use std::env;
use std::process::id;
use std::thread;
use std::time::Duration;

fn main() {
    let args: Vec<String> = env::args().collect();
    let target_mb: usize = if args.len() > 1 {
        args[1].parse().unwrap_or(2000)
    } else {
        2000 // Default: allocate up to 2000 MB RAM
    };

    let step_mb: usize = 100; // Allocate 100 MB at a time
    let delay_ms: u64 = 250;  // 250ms delay between allocation steps

    println!("🔥 [memstress] Memory Stress Test started (PID: {})", id());
    println!("Target allocation: {} MB (Allocating {} MB every {} ms)", target_mb, step_mb, delay_ms);
    println!("Press Ctrl+C to stop manually if not closed by watchdog.");
    println!("--------------------------------------------------");

    let mut buffers: Vec<Vec<u8>> = Vec::new();
    let mut allocated_mb = 0;

    while allocated_mb < target_mb {
        let chunk_size = step_mb * 1024 * 1024;
        let mut chunk = vec![0u8; chunk_size];

        // Touch pages every 4KB to force Linux kernel physical RSS page allocation
        for i in (0..chunk_size).step_by(4096) {
            chunk[i] = (i % 255) as u8;
        }

        buffers.push(chunk);
        allocated_mb += step_mb;

        println!("[memstress PID {}] Allocated {} MB physical RAM...", id(), allocated_mb);
        thread::sleep(Duration::from_millis(delay_ms));
    }

    println!("[memstress PID {}] Target {} MB reached. Holding memory for 60 seconds...", id(), allocated_mb);
    thread::sleep(Duration::from_secs(60));
}
