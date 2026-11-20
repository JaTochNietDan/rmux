//! Selection formatting only; excludes clipboard, scheduling, PTY and UI latency.
use anyhow::Result;
use rmux_terminal::Terminal;
use std::{hint::black_box, time::Instant};

fn main() -> Result<()> {
    let batch = "0123456789 abcdefghijklmnopqrstuvwxyz 界 e\u{301} terminal output\r\n".repeat(256);
    for scrollback_mib in [4, 64] {
        let mut terminal = Terminal::new(100, 32)?;
        terminal.set_scrollback_limit(scrollback_mib * 1024 * 1024)?;
        for _ in 0..400 { terminal.write(batch.as_bytes()); }
        terminal.select_all()?;
        let retained_rows = terminal.frame()?.scrollback_lines;
        for limit in [4096, 16 * 1024 * 1024] {
            let expected = terminal.selected_text_limited(limit);
            let output_bytes = expected.as_ref().map(|text| text.as_ref().map_or(0, String::len)).unwrap_or(0);
            let rejected = expected.is_err();
            let mut samples = Vec::with_capacity(20);
            for _ in 0..20 {
                let start = Instant::now();
                let result = black_box(terminal.selected_text_limited(limit));
                samples.push(start.elapsed().as_micros());
                anyhow::ensure!(result.is_err() == rejected, "inconsistent export result");
                if let Ok(text) = result { anyhow::ensure!(text == *expected.as_ref().unwrap(), "export changed"); }
            }
            samples.sort_unstable();
            println!("{{\"scrollback_mib\":{scrollback_mib},\"retained_rows\":{retained_rows},\"limit_bytes\":{limit},\"output_bytes\":{output_bytes},\"rejected\":{rejected},\"samples\":20,\"p50_us\":{},\"p95_us\":{},\"max_us\":{}}}", samples[9], samples[18], samples[19]);
        }
    }
    Ok(())
}
