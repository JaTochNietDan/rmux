//! Unchanged snapshots only; excludes PTY, shaping, GPU and changed-row work.
use anyhow::Result;
use rmux_terminal::Terminal;
use std::{hint::black_box, time::Instant};

fn main() -> Result<()> {
    for panes in [1, 4, 16] {
        for rows in [24, 48, 120] {
            let mut terminals = (0..panes)
                .map(|_| Terminal::new(160, rows))
                .collect::<Result<Vec<_>>>()?;
            let fixture = "styled \x1b[32mtext\x1b[0m 界 e\u{301}\r\n".repeat(rows as usize);
            let mut frames = Vec::new();
            for terminal in &mut terminals {
                terminal.write(fixture.as_bytes());
                frames.push(terminal.frame()?);
                for _ in 0..100 {
                    black_box(terminal.frame()?);
                }
            }
            let rounds = 20_000;
            let samples = rounds * panes;
            let start = Instant::now();
            for _ in 0..rounds {
                for terminal in &mut terminals {
                    black_box(terminal.frame()?);
                }
            }
            let unchanged_ns = start.elapsed().as_nanos() / samples as u128;
            let start = Instant::now();
            for _ in 0..rounds {
                for frame in &frames {
                    black_box(frame.clone());
                }
            }
            let clone_ns = start.elapsed().as_nanos() / samples as u128;
            // A synthetic comparison of the old vector-copy operation, not a
            // benchmark of an older rmux build or a whole-application speedup.
            let start = Instant::now();
            for _ in 0..rounds {
                for frame in &frames {
                    black_box((
                        frame.rows.as_ref().clone(),
                        frame.selections.as_ref().clone(),
                    ));
                }
            }
            let vector_copy_ns = start.elapsed().as_nanos() / samples as u128;
            println!(
                "{{\"panes\":{panes},\"columns\":160,\"rows\":{rows},\"samples\":{samples},\"unchanged_frame_mean_ns\":{unchanged_ns},\"frame_clone_mean_ns\":{clone_ns},\"synthetic_vector_copy_mean_ns\":{vector_copy_ns}}}"
            );
        }
    }
    Ok(())
}
