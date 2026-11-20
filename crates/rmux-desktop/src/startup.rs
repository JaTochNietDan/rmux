//! Desktop launch and inspection CLI. External socket calls use rmuxctl.
use std::{ffi::OsString, path::PathBuf};

pub enum Command {
    Help,
    Version,
    RpcStdio,
    CheckConfig,
    DumpConfig,
    Layout {
        path: PathBuf,
        live_rpc_stdio: bool,
    },
    CheckLayout(PathBuf),
    Session { path: PathBuf, live_rpc_stdio: bool },
    CheckSession(PathBuf),
    Launch {
        live_rpc_stdio: bool,
        directory: Option<PathBuf>,
        argv: Vec<OsString>,
    },
}
pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Command, String> {
    let mut args = args.into_iter();
    let mut directory = None;
    let mut layout = None;
    let mut check_layout = false;
    let mut session = false;
    let mut argv = Vec::new();
    let mut check_config = false;
    let mut dump_config = false;
    let mut rpc_stdio = false;
    let mut live_rpc_stdio = false;
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--") => {
                argv.extend(args);
                if argv.first().is_none_or(|program| program.is_empty()) {
                    return Err("-- requires a nonempty program name".into());
                }
                break;
            }
            Some("--help" | "-h") => return Ok(Command::Help),
            Some("--version" | "-V") => return Ok(Command::Version),
            Some("--rpc-stdio" | "--rpc-stdio-live") => {
                if rpc_stdio {
                    return Err("--rpc-stdio specified more than once".into());
                }
                rpc_stdio = true;
                live_rpc_stdio = arg == "--rpc-stdio-live";
            }
            Some("--check-config") => {
                if check_config {
                    return Err("--check-config specified more than once".into());
                }
                check_config = true;
            }
            Some("--dump-config") => {
                if dump_config {
                    return Err("--dump-config specified more than once".into());
                }
                dump_config = true;
            }
            Some("--layout" | "--check-layout" | "--session" | "--check-session") => {
                check_layout = arg == "--check-layout" || arg == "--check-session";
                session = arg == "--session" || arg == "--check-session";
                if layout.is_some() {
                    return Err("choose one layout or session file".into());
                }
                let path = args.next().ok_or("layout/session option requires a path")?;
                if path.is_empty() {
                    return Err("layout/session path must not be empty".into());
                }
                layout = Some(PathBuf::from(path));
            }
            Some("--working-directory" | "-d") => {
                if directory.is_some() {
                    return Err("working directory specified more than once".into());
                }
                let path = args.next().ok_or("--working-directory requires a path")?;
                if path.is_empty() {
                    return Err("working directory must not be empty".into());
                }
                directory = Some(PathBuf::from(path));
            }
            _ => {
                return Err(format!(
                    "unknown argument: {} (use --help)",
                    arg.to_string_lossy()
                ));
            }
        }
    }
    if live_rpc_stdio && (check_config || dump_config || check_layout) {
        return Err("--rpc-stdio-live requires a GUI launch, not an inspection command".into());
    }
    if rpc_stdio && !live_rpc_stdio {
        if check_config
            || dump_config
            || layout.is_some()
            || directory.is_some()
            || !argv.is_empty()
        {
            return Err("--rpc-stdio cannot be combined with launch or inspection options".into());
        }
        return Ok(Command::RpcStdio);
    }
    if check_config && dump_config {
        return Err("choose --check-config or --dump-config".into());
    }
    if check_config || dump_config {
        if layout.is_some() || directory.is_some() || !argv.is_empty() {
            return Err("configuration inspection cannot be combined with launch options".into());
        }
        return Ok(if dump_config {
            Command::DumpConfig
        } else {
            Command::CheckConfig
        });
    }
    if let Some(layout) = layout {
        if !argv.is_empty() {
            return Err("layout/session restore cannot be combined with a command".into());
        }
        if directory.is_some() {
            return Err("layout/session restore cannot be combined with --working-directory; saved directories are explicit".into());
        }
        Ok(if session && check_layout {
            Command::CheckSession(layout)
        } else if session {
            Command::Session { path: layout, live_rpc_stdio }
        } else if check_layout {
            Command::CheckLayout(layout)
        } else {
            Command::Layout {
                path: layout,
                live_rpc_stdio,
            }
        })
    } else {
        Ok(Command::Launch {
            directory,
            argv,
            live_rpc_stdio,
        })
    }
}
pub const HELP: &str = "rmux — terminal workspace

Usage: rmux [OPTIONS] [-- PROGRAM [ARG...]]

  -d, --working-directory PATH  Set the initial shell directory
      --layout PATH             Restore a saved layout with fresh shells
      --session PATH            Restore a multi-window session with fresh shells
      --check-session PATH      Validate session structure without starting shells
      --rpc-stdio               Standalone JSON-RPC discovery over stdin/stdout
      --rpc-stdio-live          Launch UI with JSON-RPC over stdin/stdout
      --check-layout PATH       Validate layout structure without starting shells
      --check-config            Validate settings without opening a window
      --dump-config             Print effective JSON settings and exit
  -h, --help                    Show this help without opening a window
  -V, --version                 Show the version

Arguments after -- are passed directly to the first terminal command. New tabs use the default shell.
Live automation is opt-in. Set RMUX_SOCKET_DIR to a new absolute directory for Unix socket access via rmuxctl.
";

#[cfg(test)]
mod tests {
    #[test]
    fn session_restore_and_inspection_reject_conflicting_launch_modes() {
        assert!(matches!(parse(["--session", "session.json", "--rpc-stdio-live"].map(OsString::from)), Ok(Command::Session { live_rpc_stdio: true, .. })));
        assert!(matches!(parse(["--check-session", "session.json"].map(OsString::from)), Ok(Command::CheckSession(_))));
        for args in [
            vec!["--session", "a", "--layout", "b"],
            vec!["--session", "a", "-d", "/tmp"],
            vec!["--session", "a", "--", "echo"],
            vec!["--session", ""],
            vec!["--check-session", "a", "--rpc-stdio-live"],
            vec!["--session", "a", "--rpc-stdio"],
        ] { assert!(parse(args.into_iter().map(OsString::from)).is_err()); }
    }
    use super::*;
    #[test]
    fn live_rpc_combines_with_layout_directory_and_literal_command() {
        assert!(matches!(
            parse(["--rpc-stdio-live", "--layout", "layout.json"].map(OsString::from)),
            Ok(Command::Layout {
                live_rpc_stdio: true,
                ..
            })
        ));
        let Command::Launch {
            live_rpc_stdio,
            directory,
            argv,
        } = parse(
            [
                "--rpc-stdio-live",
                "-d",
                "/tmp",
                "--",
                "printf",
                "--rpc-stdio",
            ]
            .map(OsString::from),
        )
        .unwrap()
        else {
            panic!("expected launch");
        };
        assert!(live_rpc_stdio);
        assert_eq!(directory, Some(PathBuf::from("/tmp")));
        assert_eq!(argv, ["printf", "--rpc-stdio"].map(OsString::from));
    }
    #[test]
    fn rpc_stdio_rejects_launch_and_inspection_options() {
        assert!(matches!(
            parse(["--rpc-stdio".into()]),
            Ok(Command::RpcStdio)
        ));
        assert!(matches!(
            parse(["--rpc-stdio-live".into()]),
            Ok(Command::Launch {
                live_rpc_stdio: true,
                ..
            })
        ));
        for args in [
            vec!["--rpc-stdio", "--rpc-stdio"],
            vec!["--rpc-stdio-live", "--rpc-stdio"],
            vec!["--rpc-stdio-live", "--check-layout", "a"],
            vec!["--rpc-stdio-live", "--check-config"],
            vec!["--rpc-stdio", "--check-config"],
            vec!["--rpc-stdio", "--dump-config"],
            vec!["--rpc-stdio", "--check-layout", "a"],
            vec!["--rpc-stdio", "--layout", "a"],
            vec!["--rpc-stdio", "-d", "/tmp"],
            vec!["--rpc-stdio", "--", "echo"],
        ] {
            assert!(parse(args.into_iter().map(OsString::from)).is_err());
        }
    }
    #[test]
    fn layout_check_is_headless_and_rejects_launch_combinations() {
        assert!(matches!(
            parse(["--check-layout", "layout.json"].map(OsString::from)),
            Ok(Command::CheckLayout(_))
        ));
        for args in [
            vec!["--check-layout"],
            vec!["--check-layout", "a", "--layout", "b"],
            vec!["--check-layout", "a", "--", "echo"],
            vec!["--check-layout", "a", "--dump-config"],
        ] {
            assert!(parse(args.into_iter().map(OsString::from)).is_err());
        }
    }
    #[test]
    fn configuration_dump_rejects_launch_and_duplicate_options() {
        assert!(matches!(
            parse(["--dump-config".into()]),
            Ok(Command::DumpConfig)
        ));
        for args in [
            vec!["--dump-config", "--check-config"],
            vec!["--dump-config", "--dump-config"],
            vec!["--dump-config", "--", "echo"],
            vec!["--dump-config", "-d", "/tmp"],
        ] {
            assert!(parse(args.into_iter().map(OsString::from)).is_err());
        }
    }
    #[test]
    fn command_arguments_remain_literal_after_separator() {
        let Command::Launch {
            argv, directory, ..
        } = parse(
            [
                "-d",
                "/tmp",
                "--",
                "printf",
                "--help",
                "a b",
                "$(touch nope)",
                "",
            ]
            .map(OsString::from),
        )
        .unwrap()
        else {
            panic!("launch expected")
        };
        assert_eq!(directory, Some(PathBuf::from("/tmp")));
        assert_eq!(
            argv,
            ["printf", "--help", "a b", "$(touch nope)", ""].map(OsString::from)
        );
        for args in [
            vec!["--"],
            vec!["--", ""],
            vec!["--layout", "file", "--", "echo"],
        ] {
            assert!(parse(args.into_iter().map(OsString::from)).is_err());
        }
    }
    #[test]
    fn startup_arguments_preserve_paths_and_reject_mistakes() {
        let args = ["-d", "project 世界 space"].map(OsString::from);
        let Command::Launch { directory, .. } = parse(args).unwrap() else {
            panic!("launch expected");
        };
        assert_eq!(directory, Some(PathBuf::from("project 世界 space")));
        for args in [
            vec!["--unknown"],
            vec!["-d"],
            vec!["-d", ""],
            vec!["-d", "a", "-d", "b"],
            vec!["--layout"],
            vec!["--layout", "a", "--layout", "b"],
            vec!["--layout", "a", "-d", "b"],
        ] {
            assert!(parse(args.into_iter().map(OsString::from)).is_err());
        }
        assert!(matches!(
            parse([OsString::from("--help")]),
            Ok(Command::Help)
        ));
        assert!(matches!(
            parse([OsString::from("--version")]),
            Ok(Command::Version)
        ));
        assert!(
            matches!(parse(["--layout", "saved layout.json"].map(OsString::from)), Ok(Command::Layout { path, live_rpc_stdio: false }) if path == PathBuf::from("saved layout.json"))
        );
    }
}
