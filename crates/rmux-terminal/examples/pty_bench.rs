//! PTY + engine pressure test. No GUI, glyph shaping or presentation timing.
use anyhow::{Result, ensure};
use portable_pty::CommandBuilder;
use rmux_terminal::session::Session;
use std::{
    hint::black_box,
    io::{self, BufRead, Write},
    thread,
    time::{Duration, Instant},
};

fn child(mode: &str) -> Result<()> {
    let mut output = io::stdout().lock();
    if mode == "--flood" {
        let chunk = "\x1b[32mbuild\x1b[0m output pressure 界 e\u{301}\r\n".repeat(256);
        loop {
            output.write_all(chunk.as_bytes())?;
        }
    }
    writeln!(output, "responder-ready")?;
    output.flush()?;
    for line in io::stdin().lock().lines() {
        // Responses deliberately differ from input so local PTY echo cannot
        // satisfy the latency measurement.
        writeln!(output, "pong-{}", line?.trim())?;
        output.flush()?;
    }
    Ok(())
}

fn spawn(mode: &str) -> Result<Session> {
    let mut command = CommandBuilder::new(std::env::current_exe()?);
    command.arg(mode);
    command.env("TERM", "xterm-256color");
    Session::spawn(command, 160, 48)
}

fn percentile(values: &[u128], percent: usize) -> u128 {
    // Nearest-rank percentile, with a minimum rank of one.
    values[(values.len() * percent).div_ceil(100).max(1) - 1]
}

fn benchmark(panes: usize) -> Result<()> {
    ensure!([1, 4, 8, 16].contains(&panes), "use 1, 4, 8 or 16 panes");
    let responder = spawn("--echo")?;
    let busy: Vec<_> = (1..panes)
        .map(|_| spawn("--flood"))
        .collect::<Result<_>>()?;
    let ready_deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if responder.frame()?.text().contains("responder-ready")
            && busy.iter().all(|s| s.output_bytes() > 0)
        {
            break;
        }
        ensure!(
            Instant::now() < ready_deadline,
            "PTY workloads did not become ready"
        );
        thread::sleep(Duration::from_millis(5));
    }
    thread::sleep(Duration::from_millis(200));
    let before: Vec<_> = busy.iter().map(Session::output_bytes).collect();
    let start = Instant::now();
    let mut admission = Vec::with_capacity(128);
    let mut responses = Vec::with_capacity(128);
    let mut snapshot_batches = Vec::new();
    for sequence in 0..128 {
        let request = format!("{sequence:04}\n");
        let expected = format!("pong-{sequence:04}");
        let sent = Instant::now();
        responder.input(request.as_bytes())?;
        admission.push(sent.elapsed().as_nanos());
        loop {
            let snapshot_start = Instant::now();
            for session in &busy {
                black_box(session.frame()?);
            }
            let response = responder.frame()?;
            snapshot_batches.push(snapshot_start.elapsed().as_nanos());
            if response.text().lines().any(|line| line == expected) {
                responses.push(sent.elapsed().as_nanos());
                break;
            }
            ensure!(
                sent.elapsed() < Duration::from_secs(2),
                "response timeout at sample {sequence}"
            );
            // Polling is in this measurement harness, not the application.
            thread::sleep(Duration::from_micros(100));
        }
        thread::sleep(Duration::from_millis(5));
    }
    let elapsed = start.elapsed().as_secs_f64();
    let bytes: Vec<_> = busy
        .iter()
        .zip(before)
        .map(|(session, before)| session.output_bytes() - before)
        .collect();
    ensure!(
        bytes.iter().all(|count| *count > 0),
        "a flood pane stopped producing output"
    );
    let total_bytes: u64 = bytes.iter().sum();
    admission.sort_unstable();
    responses.sort_unstable();
    snapshot_batches.sort_unstable();
    println!(
        "{{\"panes\":{panes},\"flood_panes\":{},\"columns\":160,\"rows\":48,\"samples\":128,\"elapsed_seconds\":{elapsed:.6},\"parsed_flood_bytes\":{total_bytes},\"flood_mib_per_second\":{:.3},\"input_call_p50_ns\":{},\"input_call_p99_ns\":{},\"response_snapshot_p50_ns\":{},\"response_snapshot_p95_ns\":{},\"response_snapshot_p99_ns\":{},\"snapshot_batch_p99_ns\":{}}}",
        busy.len(),
        total_bytes as f64 / 1048576. / elapsed,
        percentile(&admission, 50),
        percentile(&admission, 99),
        percentile(&responses, 50),
        percentile(&responses, 95),
        percentile(&responses, 99),
        percentile(&snapshot_batches, 99)
    );
    Ok(())
}

fn main() -> Result<()> {
    let mode = std::env::args().nth(1).unwrap_or_else(|| "all".into());
    if mode == "--flood" || mode == "--echo" {
        return child(&mode);
    }
    if mode == "all" {
        for panes in [1, 4, 8, 16] {
            benchmark(panes)?;
        }
    } else {
        benchmark(mode.parse()?)?;
    }
    Ok(())
}
