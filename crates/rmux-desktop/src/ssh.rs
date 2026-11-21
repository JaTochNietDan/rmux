//! Launch OpenSSH directly; authentication/configuration remain client-owned.
pub const INPUT_HINT: &str = "SSH alias, user@host or ssh://user@host:port · Authenticate in the terminal";
pub fn command(destination: &str) -> Result<crate::preferences::CustomCommand, &'static str> {
    let authority = destination.strip_prefix("ssh://").unwrap_or(destination);
    if destination.is_empty() || destination.len() > 255 || destination.starts_with('-')
        || !authority.bytes().all(|ch| ch.is_ascii_alphanumeric() || b"._-@:%[]".contains(&ch)) {
        return Err("Enter an SSH destination (up to 255 bytes), without spaces, paths or options");
    }
    let mut parts = authority.split('@');
    let first = parts.next().unwrap_or_default();
    let host = parts.next().unwrap_or(first);
    if first.is_empty() || host.is_empty() || host.starts_with('-') || parts.next().is_some() {
        return Err("Enter one host, optionally prefixed with user@");
    }
    let argv = if destination.starts_with("ssh://") {
        if authority.contains('@') && !first.bytes().all(|ch| ch.is_ascii_alphanumeric() || b"._-".contains(&ch)) {
            return Err("SSH URLs accept a username, not passwords or encoded user information");
        }
        let (host, port) = validate_url_host(host)?;
        let mut argv = vec!["ssh".into()];
        if authority.contains('@') { argv.extend(["-l".into(), first.into()]); }
        if let Some(port) = port { argv.extend(["-p".into(), port.into()]); }
        argv.extend(["--".into(), host.into()]);
        argv
    } else { vec!["ssh".into(), "--".into(), destination.into()] };
    Ok(crate::preferences::CustomCommand {
        name: format!("SSH · {destination}"),
        argv,
        env: Default::default(), cwd: None,
    })
}

fn validate_url_host(host: &str) -> Result<(&str, Option<&str>), &'static str> {
    let (hostname, port) = if let Some(bracketed) = host.strip_prefix('[') {
        let (address, suffix) = bracketed.split_once(']').ok_or("Close the IPv6 address with ]")?;
        let hostname = address;
        let (address, zone) = address.split_once('%').map_or((address, None), |(address, zone)| (address, Some(zone)));
        if address.parse::<std::net::Ipv6Addr>().is_err() || zone.is_some_and(|zone| zone.is_empty() || !zone.bytes().all(|ch| ch.is_ascii_alphanumeric() || b"._-".contains(&ch))) {
            return Err("Enter a valid bracketed IPv6 address");
        }
        (hostname, if suffix.is_empty() { None } else { Some(suffix.strip_prefix(':').ok_or("Use :port after the IPv6 address")?) })
    } else {
        let (name, port) = host.split_once(':').map_or((host, None), |(name, port)| (name, Some(port)));
        if name.is_empty() || !name.bytes().all(|ch| ch.is_ascii_alphanumeric() || b"._-".contains(&ch)) {
            return Err("Enter a host name, or put an IPv6 address in brackets");
        }
        (name, port)
    };
    if port.is_some_and(|port| port.is_empty() || !port.bytes().all(|ch| ch.is_ascii_digit()) || port.parse::<u16>().map_or(true, |port| port == 0)) {
        return Err("SSH port must be a number from 1 to 65535; bracket IPv6 addresses");
    }
    Ok((hostname, port))
}

#[cfg(test)]
mod tests {
    #[test]
    fn ssh_urls_validate_ports_ipv6_and_exclude_passwords_or_paths() {
        for (destination, expected) in [
            ("ssh://user@example.org:2222", vec!["ssh", "-l", "user", "-p", "2222", "--", "example.org"]),
            ("ssh://example.org", vec!["ssh", "--", "example.org"]),
            ("ssh://user@[2001:db8::1]:65535", vec!["ssh", "-l", "user", "-p", "65535", "--", "2001:db8::1"]),
            ("ssh://[::1]", vec!["ssh", "--", "::1"]),
            ("ssh://[fe80::1%en0]:22", vec!["ssh", "-p", "22", "--", "fe80::1%en0"]),
        ] {
            assert_eq!(super::command(destination).unwrap().argv, expected);
        }
        for destination in ["ssh://", "ssh://host:", "ssh://host:0", "ssh://host:65536", "ssh://host:+22", "ssh://host/path", "ssh://user:password@host", "ssh://[bad]:22", "ssh://[::1]extra", "ssh://::1", "ssh://[::1", "ssh://[::1%]:22", "ssh://-host"] {
            assert!(super::command(destination).is_err(), "{destination:?}");
        }
    }
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
