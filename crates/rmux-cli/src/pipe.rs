//! One bounded text snapshot sent to an explicitly supplied local shell command.
// Command output is inherited, so the CLI does not accumulate stdout/stderr.
pub fn run(command: &str, text: &str) -> Result<i32, String> {
    if text.len() > 1024 * 1024 { return Err("pipe-pane snapshot exceeds 1 MiB".into()); }
    if command.trim().is_empty() { return Err("pipe-pane requires a shell command".into()); }
    run_platform(command, text)
}
#[cfg(unix)]
fn run_platform(command: &str, text: &str) -> Result<i32, String> {
    use std::{io::{Write, ErrorKind}, process::{Command, Stdio}, os::unix::process::ExitStatusExt};
    let mut child = Command::new("/bin/sh").args(["-c", command])
        .stdin(Stdio::piped()).stdout(Stdio::inherit()).stderr(Stdio::inherit())
        .spawn().map_err(|error| format!("start pipe-pane command: {error}"))?;
    let written = child.stdin.take().expect("piped stdin").write_all(text.as_bytes());
    // Early-consuming commands (e.g. head) may legitimately close stdin.
    if let Err(error) = written {
        if error.kind() != ErrorKind::BrokenPipe {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("write pipe-pane input: {error}"));
        }
    }
    match child.wait() {
        Ok(status) => Ok(status.code().unwrap_or_else(|| 128 + status.signal().unwrap_or(1))),
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            Err(format!("wait for pipe-pane command: {error}"))
        }
    }
}
#[cfg(not(unix))]
fn run_platform(_: &str, _: &str) -> Result<i32, String> {
    Err("pipe-pane local shell execution is not available on this platform".into())
}
#[cfg(all(test, unix))]
mod tests {
    #[test]
    fn pipe_snapshot_stays_stdin_and_preserves_child_exit_status() {
        assert_eq!(super::run("IFS= read -r line; test \"$line\" = 'literal $(exit 7)'", "literal $(exit 7)\n").unwrap(), 0);
        assert_eq!(super::run("cat >/dev/null; exit 3", "世界\nnext\n").unwrap(), 3);
        assert_eq!(super::run("exit 0", &"x".repeat(1024 * 1024)).unwrap(), 0);
        assert!(super::run("exit 0", &"x".repeat(1024 * 1024 + 1)).is_err());
    }
}
