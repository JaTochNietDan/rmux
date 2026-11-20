//! Deterministic engine-only baseline. Excludes PTY, shaping and GPU presentation.
use anyhow::Result;
use rmux_terminal::Terminal;
use std::{hint::black_box, time::Instant};

fn main() -> Result<()> {
    let panes: usize = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "1".into())
        .parse()?;
    anyhow::ensure!([1, 4, 8, 16].contains(&panes), "use 1, 4, 8 or 16 panes");
    let scrollback_mib: usize = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "4".into())
        .parse()?;
    anyhow::ensure!(scrollback_mib <= 256, "scrollback MiB must be 0–256");
    let scrollback_bytes = scrollback_mib * 1024 * 1024;
    let mut terminals: Vec<_> = (0..panes)
        .map(|_| {
            let mut terminal = Terminal::new(160, 48)?;
            terminal.set_scrollback_limit(scrollback_bytes)?;
            Ok(terminal)
        })
        .collect::<Result<_>>()?;
    let chunk =
        "\x1b[32mbuild\x1b[0m src/main.rs: compiling terminal output 界 e\u{301}\r\n".repeat(128);
    // Warm caches and fill bounded scrollback before measuring.
    for terminal in &mut terminals {
        for _ in 0..512 {
            terminal.write(chunk.as_bytes());
        }
        black_box(terminal.frame()?);
    }
    let rounds = 8192;
    let start = Instant::now();
    let mut snapshots = Vec::with_capacity(rounds * panes / 8);
    for round in 0..rounds {
        for terminal in &mut terminals {
            terminal.write(chunk.as_bytes());
            if round % 8 == 0 {
                let snapshot_start = Instant::now();
                black_box(terminal.frame()?);
                snapshots.push(snapshot_start.elapsed().as_nanos());
            }
        }
    }
    let elapsed = start.elapsed().as_secs_f64();
    snapshots.sort_unstable();
    // The final output rounds are not sampled. Consume them before measuring
    // unchanged snapshots so the idle metric excludes one dirty frame per pane.
    for terminal in &mut terminals {
        black_box(terminal.frame()?);
    }
    let idle_start = Instant::now();
    for _ in 0..1000 {
        for terminal in &mut terminals {
            black_box(terminal.frame()?);
        }
    }
    let idle_ns = idle_start.elapsed().as_nanos() / (1000 * panes) as u128;
    let mib = (chunk.len() * rounds * panes) as f64 / (1024. * 1024.);
    // Query after timing so accounting does not enter parser/snapshot samples.
    let mut resident_bytes = 0u64;
    let mut virtual_bytes = 0u64;
    let mut image_bytes = 0u64;
    let mut retained_scrollback_lines = 0u64;
    for terminal in &mut terminals {
        retained_scrollback_lines += terminal.frame()?.scrollback_lines;
        let memory = terminal.memory_usage()?;
        resident_bytes += memory.primary_resident_bytes + memory.alternate_resident_bytes;
        virtual_bytes += memory.virtual_bytes;
        image_bytes += memory.image_bytes;
    }
    let compression_start = Instant::now();
    let mut compressed_resident_bytes = 0u64;
    let mut compression_supported = true;
    let mut compression_step_max_ns = 0;
    let mut compression_steps = 0;
    for terminal in &mut terminals {
        let mut steps = 0;
        loop {
            steps += 1;
            anyhow::ensure!(steps <= 10000, "compression did not finish");
            let step_started = Instant::now();
            let result = terminal.compress_scrollback_step()?;
            compression_step_max_ns =
                compression_step_max_ns.max(step_started.elapsed().as_nanos());
            compression_steps += 1;
            match result {
                rmux_terminal::Compression::Pending => {}
                rmux_terminal::Compression::Unsupported => {
                    compression_supported = false;
                    break;
                }
                rmux_terminal::Compression::Complete => break,
            }
        }
        let memory = terminal.memory_usage()?;
        compressed_resident_bytes +=
            memory.primary_resident_bytes + memory.alternate_resident_bytes;
    }
    let compression_ns = compression_start.elapsed().as_nanos();
    let mut history_revisit_max_ns = 0;
    for terminal in &mut terminals {
        let started = Instant::now();
        terminal.scroll(rmux_terminal::Scroll::Top)?;
        let frame = terminal.frame()?;
        history_revisit_max_ns = history_revisit_max_ns.max(started.elapsed().as_nanos());
        anyhow::ensure!(
            frame.text().contains("compiling terminal output"),
            "compressed history missing"
        );
        black_box(frame);
    }
    println!(
        "{{\"panes\":{panes},\"columns\":160,\"rows\":48,\"scrollback_bytes_per_pane\":{scrollback_bytes},\"retained_scrollback_lines\":{retained_scrollback_lines},\"input_mib\":{mib:.3},\"elapsed_seconds\":{elapsed:.6},\"mib_per_second\":{:.3},\"snapshot_samples\":{},\"snapshot_p50_ns\":{},\"snapshot_p99_ns\":{},\"unchanged_snapshot_mean_ns\":{idle_ns},\"ghostty_resident_bytes\":{resident_bytes},\"ghostty_virtual_bytes\":{virtual_bytes},\"ghostty_image_bytes\":{image_bytes},\"compression_supported\":{compression_supported},\"compressed_resident_bytes\":{compressed_resident_bytes},\"compression_ns\":{compression_ns},\"compression_steps\":{compression_steps},\"compression_step_max_ns\":{compression_step_max_ns},\"history_revisit_max_ns\":{history_revisit_max_ns}}}",
        mib / elapsed,
        snapshots.len(),
        snapshots[snapshots.len() / 2],
        snapshots[(snapshots.len() - 1) * 99 / 100]
    );
    Ok(())
}
