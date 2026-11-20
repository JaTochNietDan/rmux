//! Dirty extraction and retained row identities; no PTY, shaping or GPU.
use anyhow::Result;
use rmux_terminal::Terminal;
use std::{hint::black_box, sync::Arc, time::Instant};

fn main() -> Result<()> {
    for panes in [1, 4, 16] {
        for identical in [true, false] {
            let mut terminals = (0..panes)
                .map(|_| Terminal::new(160, 48))
                .collect::<Result<Vec<_>>>()?;
            let fixture = "styled \x1b[32mtext\x1b[0m 界 e\u{301}\r\n".repeat(48);
            let mut previous = Vec::new();
            for terminal in &mut terminals {
                terminal.write(fixture.as_bytes());
                terminal.write(b"\x1b[1;1Htick-0");
                previous.push(terminal.frame()?);
            }
            let rounds = 2000;
            let mut retained = 0u64;
            let mut frame_ns = 0u128;
            for round in 0..rounds {
                let text = if identical || round % 2 == 1 {
                    b"\x1b[1;1Htick-0"
                } else {
                    b"\x1b[1;1Htick-1"
                };
                for (terminal, previous) in terminals.iter_mut().zip(&mut previous) {
                    terminal.write(text);
                    let start = Instant::now();
                    let next = terminal.frame()?;
                    frame_ns += start.elapsed().as_nanos();
                    retained += previous
                        .rows
                        .iter()
                        .zip(next.rows.iter())
                        .filter(|(old, new)| Arc::ptr_eq(old, new))
                        .count() as u64;
                    *previous = black_box(next);
                }
            }
            let samples = rounds * panes;
            let scenario = if identical {
                "identical_rewrite"
            } else {
                "one_row_changes"
            };
            println!(
                "{{\"scenario\":\"{scenario}\",\"panes\":{panes},\"columns\":160,\"rows\":48,\"samples\":{samples},\"frame_mean_ns\":{},\"retained_rows\":{retained},\"observed_rows\":{}}}",
                frame_ns / samples as u128,
                samples * 48
            );
        }
    }
    Ok(())
}
