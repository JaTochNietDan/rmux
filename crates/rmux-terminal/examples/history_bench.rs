//! Synchronous export duration approximates engine-lock hold time. Excludes
//! scheduler/lock acquisition, RPC serialization, PTY, UI and GPU work.
use anyhow::Result;
use rmux_terminal::{Compression, Terminal};
use std::{hint::black_box, time::Instant};

fn main() -> Result<()> {
    let line = "0123456789 abcdefghijklmnopqrstuvwxyz 界 e\u{301} repeated terminal output\r\n";
    let batch = line.repeat(256);
    for scrollback_mib in [4, 64] {
        let mut terminal = Terminal::new(100, 32)?;
        terminal.set_scrollback_limit(scrollback_mib * 1024 * 1024)?;
        for _ in 0..400 { terminal.write(batch.as_bytes()); }
        let retained_rows = terminal.frame()?.scrollback_lines;
        for limit in [64 * 1024, 1024 * 1024, 16 * 1024 * 1024] {
            // Recompress before each limit to capture the first access separately
            // from repeated exports over already-restored pages.
            let mut steps = 0;
            let compression = loop {
                steps += 1;
                anyhow::ensure!(steps <= 100_000, "compression did not finish");
                match terminal.compress_scrollback_step()? {
                    Compression::Pending => continue,
                    Compression::Complete => break "complete",
                    Compression::Unsupported => break "unsupported",
                }
            };
            let start = Instant::now();
            let (text, truncated) = terminal.history_text_limited(limit)?;
            let first_us = start.elapsed().as_micros();
            let bytes = text.len();
            black_box(text);
            let mut samples = Vec::with_capacity(20);
            for _ in 0..20 {
                let start = Instant::now();
                black_box(terminal.history_text_limited(limit)?);
                samples.push(start.elapsed().as_micros());
            }
            samples.sort_unstable();
            println!("{{\"scrollback_mib\":{scrollback_mib},\"retained_rows\":{retained_rows},\"limit_bytes\":{limit},\"output_bytes\":{bytes},\"truncated\":{truncated},\"compression\":\"{compression}\",\"first_us\":{first_us},\"samples\":20,\"p50_us\":{},\"p95_us\":{},\"max_us\":{}}}", samples[9], samples[18], samples[19]);
        }
    }
    Ok(())
}
