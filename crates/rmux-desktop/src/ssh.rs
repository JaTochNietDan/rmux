//! Launch OpenSSH directly; authentication/configuration remain client-owned.
pub const INPUT_HINT: &str = "Enter an SSH alias or user@host · Authentication opens in the terminal";
pub fn command(destination: &str) -> Result<crate::preferences::CustomCommand, &'static str> {
    if destination.is_empty() || destination.len() > 255 || destination.starts_with('-')
        || !destination.bytes().all(|ch| ch.is_ascii_alphanumeric() || b"._-@:%[]".contains(&ch)) {
        return Err("Enter an SSH alias or user@host (up to 255 bytes), without spaces or options");
    }
    let mut parts = destination.split('@');
    let first = parts.next().unwrap_or_default();
    let host = parts.next().unwrap_or(first);
    if first.is_empty() || host.is_empty() || host.starts_with('-') || parts.next().is_some() {
        return Err("Enter one host, optionally prefixed with user@");
    }
    Ok(crate::preferences::CustomCommand {
        name: format!("SSH · {destination}"),
        argv: vec!["ssh".into(), "--".into(), destination.into()],
        env: Default::default(), cwd: None,
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn destinations_are_one_literal_argument_without_shell_options() {
        for destination in ["production", "user@example.org", "user@2001:db8::1", "host-name", "user@fe80::1%en0"] {
            let command = super::command(destination).unwrap();
            assert_eq!(command.argv, ["ssh", "--", destination]);
            assert!(command.env.is_empty());
        }
        for destination in ["", "-oProxyCommand=bad", "host command", "$(command)", "a;b", "a\nb", "@host", "user@", "a@b@c", "user@-option"] {
            assert!(super::command(destination).is_err(), "{destination:?}");
        }
        assert!(super::command(&"x".repeat(256)).is_err());
    }
}
