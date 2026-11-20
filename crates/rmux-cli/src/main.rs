//! Small external client: no GPUI, Ghostty, shell or terminal dependencies.
mod commands;
mod completions;
use serde_json::{Value, json};
use std::{ffi::OsString, path::PathBuf};

const HELP: &str = "Usage: rmuxctl [--socket PATH] METHOD [JSON_OBJECT]\n\nConnect to an existing rmux Unix socket. Without --socket, use\n$RMUX_SOCKET_DIR/rpc.sock. Print the JSON-RPC response to stdout.\nExit 0 on success, 1 on RPC error, 2 on usage/transport failure.\nRequests are never retried: a timeout may have an unknown outcome.\n";
struct Request {
    socket: PathBuf,
    body: Vec<u8>,
}
fn parse(
    args: impl IntoIterator<Item = OsString>,
    directory: Option<OsString>,
) -> Result<Option<Request>, String> {
    let mut args = args.into_iter().peekable();
    if args.peek().is_some_and(|v| v == "--help" || v == "-h") {
        return Ok(None);
    }
    let socket = if args.peek().is_some_and(|v| v == "--socket") {
        args.next();
        let path = args.next().ok_or("--socket requires a path")?;
        if path.is_empty() {
            return Err("socket path must not be empty".into());
        }
        PathBuf::from(path)
    } else {
        let directory = directory
            .filter(|v| !v.is_empty())
            .ok_or("specify --socket PATH or RMUX_SOCKET_DIR")?;
        PathBuf::from(directory).join("rpc.sock")
    };
    let method = args
        .next()
        .ok_or("a method is required")?
        .into_string()
        .map_err(|_| "method must be UTF-8")?;
    if method.is_empty() || method.starts_with('-') {
        return Err("a nonempty RPC method is required".into());
    }
    let (method, params) = commands::expand(&method, args.collect())?;
    let mut body =
        serde_json::to_vec(&json!({"jsonrpc":"2.0", "id":1, "method":method, "params":params}))
            .unwrap();
    if body.len() > 64 * 1024 {
        return Err("request exceeds 64 KiB".into());
    }
    body.push(b'\n');
    Ok(Some(Request { socket, body }))
}
#[cfg(unix)]
fn exchange(request: Request) -> Result<Value, String> {
    use std::{
        io::{BufRead, BufReader, Read, Write},
        os::unix::net::UnixStream,
        time::Duration,
    };
    let mut stream = UnixStream::connect(request.socket).map_err(|e| format!("connect: {e}"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(7)))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| e.to_string())?;
    stream
        .write_all(&request.body)
        .map_err(|e| format!("write: {e}; request outcome may be unknown"))?;
    // Bound allocation even if the endpoint is not an rmux server.
    const LIMIT: u64 = 8 * 1024 * 1024;
    let mut line = Vec::new();
    BufReader::new(stream)
        .take(LIMIT + 1)
        .read_until(b'\n', &mut line)
        .map_err(|e| format!("read: {e}; request outcome may be unknown"))?;
    if line.len() as u64 > LIMIT || !line.ends_with(b"\n") {
        return Err("incomplete or oversized reply; request outcome may be unknown".into());
    }
    let reply: Value = serde_json::from_slice(&line).map_err(|e| format!("invalid reply: {e}"))?;
    if reply.get("jsonrpc") != Some(&json!("2.0"))
        || reply.get("id") != Some(&json!(1))
        || (reply.get("result").is_some() == reply.get("error").is_some())
    {
        return Err("invalid JSON-RPC response envelope".into());
    }
    Ok(reply)
}
#[cfg(not(unix))]
fn exchange(_: Request) -> Result<Value, String> {
    Err("local socket transport is not yet available on this platform".into())
}
fn run() -> Result<i32, String> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.first().is_some_and(|arg| arg == "completions") {
        if args.len() != 2 { return Err("usage: rmuxctl completions bash|zsh|fish".into()); }
        print!("{}", completions::script(args[1].to_str().ok_or("shell must be UTF-8")?)?);
        return Ok(0);
    }
    if args.first().is_some_and(|arg| arg == "__complete") {
        for candidate in completions::candidates(&args[1..]) { println!("{candidate}"); }
        return Ok(0);
    }
    let Some(request) = parse(
        args,
        std::env::var_os("RMUX_SOCKET_DIR"),
    )?
    else {
        print!("{HELP}\n{}\nOffline shell setup: rmuxctl completions bash|zsh|fish\n", commands::HELP);
        return Ok(0);
    };
    let reply = exchange(request)?;
    println!("{reply}");
    Ok(if reply.get("error").is_some() { 1 } else { 0 })
}
fn main() {
    std::process::exit(match run() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("rmuxctl: {error}");
            2
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_ambiguous_or_oversized_requests() {
        for args in [
            vec!["method", "[]"],
            vec!["method", "{}", "extra"],
            vec!["--socket", ""],
            vec!["--bad"],
        ] {
            assert!(
                parse(
                    args.into_iter().map(OsString::from),
                    Some("/tmp/test".into())
                )
                .is_err()
            );
        }
        assert!(
            parse(
                [
                    "method".into(),
                    json!({"text":"x".repeat(65536)}).to_string().into()
                ],
                Some("/tmp/test".into())
            )
            .is_err()
        );
    }
    #[cfg(unix)]
    #[test]
    fn exchanges_unicode_and_preserves_rpc_errors() {
        use std::{
            io::{BufRead, BufReader, Write},
            os::unix::net::UnixListener,
        };
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rpc.sock");
        let listener = UnixListener::bind(&path).unwrap();
        let worker = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = String::new();
            BufReader::new(&stream).read_line(&mut request).unwrap();
            let request: Value = serde_json::from_str(&request).unwrap();
            assert_eq!(request["params"]["name"], "世界");
            stream.write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"error\":{\"code\":-32601,\"message\":\"unknown\"}}\n").unwrap();
        });
        let request = parse(
            [
                "--socket".into(),
                path.into_os_string(),
                "test".into(),
                r#"{"name":"世界"}"#.into(),
            ],
            None,
        )
        .unwrap()
        .unwrap();
        assert_eq!(exchange(request).unwrap()["error"]["code"], -32601);
        worker.join().unwrap();
    }
}
