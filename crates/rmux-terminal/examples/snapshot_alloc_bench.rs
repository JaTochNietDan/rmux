//! Rust allocation traffic during snapshots, excluding parser writes and setup.
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};
use std::time::Instant;

struct CountingAllocator;
static TRACK: AtomicBool = AtomicBool::new(false);
static CALLS: AtomicU64 = AtomicU64::new(0);
static BYTES: AtomicU64 = AtomicU64::new(0);

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if TRACK.load(Relaxed) {
            CALLS.fetch_add(1, Relaxed);
            BYTES.fetch_add(layout.size() as u64, Relaxed);
        }
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if TRACK.load(Relaxed) {
            CALLS.fetch_add(1, Relaxed);
            BYTES.fetch_add(layout.size() as u64, Relaxed);
        }
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        if TRACK.load(Relaxed) {
            CALLS.fetch_add(1, Relaxed);
            BYTES.fetch_add(size as u64, Relaxed);
        }
        unsafe { System.realloc(ptr, layout, size) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

fn main() -> anyhow::Result<()> {
    const ITERATIONS: usize = 200;
    for count in [1, 8, 32] {
        for scenario in ["identical_ascii", "late_ascii_edit", "large_grapheme_style"] {
            let text = if scenario == "large_grapheme_style" {
                format!("a{}", "\u{301}".repeat(40))
            } else {
                "x".repeat(100)
            };
            let writes = if scenario == "identical_ascii" {
                [format!("\x1b[H{text}"), format!("\x1b[H{text}")]
            } else if scenario == "late_ascii_edit" {
                [format!("\x1b[H{text}a"), format!("\x1b[H{text}b")]
            } else {
                [format!("\x1b[H\x1b[31m{text}"), format!("\x1b[H\x1b[32m{text}")]
            };
            let mut terminals = (0..count).map(|_| rmux_terminal::Terminal::new(120, 24))
                .collect::<anyhow::Result<Vec<_>>>()?;
            for terminal in &mut terminals { terminal.write(writes[1].as_bytes()); }
            // Retain an external frame as the UI does, exercising copy-on-write.
            let mut frames = terminals.iter_mut().map(|terminal| terminal.frame())
                .collect::<anyhow::Result<Vec<_>>>()?;
            CALLS.store(0, Relaxed);
            BYTES.store(0, Relaxed);
            let mut elapsed = std::time::Duration::ZERO;
            for iteration in 0..ITERATIONS {
                for terminal in &mut terminals { terminal.write(writes[iteration % 2].as_bytes()); }
                TRACK.store(true, Relaxed);
                let start = Instant::now();
                let result = (|| -> anyhow::Result<()> {
                    for (terminal, frame) in terminals.iter_mut().zip(&mut frames) {
                        *frame = terminal.frame()?;
                    }
                    Ok(())
                })();
                elapsed += start.elapsed();
                TRACK.store(false, Relaxed);
                result?;
            }
            assert!(frames.iter().all(|frame| frame.text().contains(&text)));
            println!("{{\"terminals\":{count},\"scenario\":\"{scenario}\",\"frames\":{},\"rust_alloc_calls\":{},\"rust_requested_bytes\":{},\"snapshot_total_us\":{}}}",
                count * ITERATIONS, CALLS.load(Relaxed), BYTES.load(Relaxed), elapsed.as_micros());
        }
    }
    Ok(())
}
