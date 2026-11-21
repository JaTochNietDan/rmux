//! Launch OpenSSH directly; authentication/configuration remain client-owned.
pub const INPUT_HINT: &str = "SSH alias, user@host or ssh://user@host:port · Authenticate in the terminal";
pub struct Aliases { pub names: Vec<String>, pub limited: bool }
pub fn read_aliases(path: &std::path::Path, canceled: &std::sync::atomic::AtomicBool) -> anyhow::Result<Aliases> {
    use std::io::Read;
    let check = || -> anyhow::Result<()> {
        anyhow::ensure!(!canceled.load(std::sync::atomic::Ordering::Acquire), "SSH host loading canceled");
        Ok(())
    };
    check()?;
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)] {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK);
    }
    let mut file = match options.open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Aliases { names: Vec::new(), limited: false }),
        Err(error) => return Err(error.into()),
    };
    anyhow::ensure!(file.metadata()?.is_file(), "SSH configuration is not a regular file");
    let mut bytes = Vec::new();
    let mut chunk = [0; 8192];
    while bytes.len() < 65537 {
        check()?;
        let limit = chunk.len().min(65537 - bytes.len());
        let count = match file.read(&mut chunk[..limit]) {
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            result => result?,
        };
        if count == 0 { break; }
        bytes.extend_from_slice(&chunk[..count]);
    }
    check()?;
    let limited = bytes.len() > 65536;
    if limited {
        bytes.truncate(65536);
        // Never offer a host name cut in half at the read limit.
        bytes.truncate(bytes.iter().rposition(|byte| *byte == b'\n').map_or(0, |index| index + 1));
    }
    let text = std::str::from_utf8(&bytes)?;
    let mut result = parse_aliases(text);
    result.limited |= limited;
    Ok(result)
}

fn config_words(line: &str) -> Option<Vec<String>> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quoted = false;
    let mut escaped = false;
    for ch in line.chars() {
        if escaped { word.push(ch); escaped = false; continue; }
        match ch {
            '\\' => escaped = true,
            '"' => quoted = !quoted,
            '#' if !quoted => break,
            ch if !quoted && (ch.is_whitespace() || ch == '=') => {
                if !word.is_empty() { words.push(std::mem::take(&mut word)); }
            }
            ch => word.push(ch),
        }
    }
    if quoted || escaped { return None; }
    if !word.is_empty() { words.push(word); }
    Some(words)
}

fn parse_aliases(text: &str) -> Aliases {
    let mut names = std::collections::BTreeMap::new();
    let mut limited = false;
    for line in text.lines() {
        let Some(words) = config_words(line) else { continue; };
        if !words.first().is_some_and(|word| word.eq_ignore_ascii_case("Host")) { continue; }
        for name in words.into_iter().skip(1) {
            if name.is_empty() || name.len() > 255 || name.starts_with('-')
                || !name.bytes().all(|ch| ch.is_ascii_alphanumeric() || b"._-:%".contains(&ch)) { continue; }
            let key = name.to_ascii_lowercase();
            if names.contains_key(&key) { continue; }
            if names.len() == 256 { limited = true; break; }
            names.insert(key, name);
        }
        if limited { break; }
    }
    Aliases { names: names.into_values().collect(), limited }
}

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
    fn host_aliases_are_literal_sorted_deduplicated_and_bounded() {
        let aliases = super::parse_aliases("# Hosts\nHost \"Production\" dev *.example !blocked\n HostName internal\nhOsT=DEV staging # comment\nInclude elsewhere\nMatch exec something\nHost \"unterminated\nHost user@host -option\n");
        assert_eq!(aliases.names, ["dev", "Production", "staging"]);
        assert!(!aliases.limited);
        let many: String = (0..300).map(|index| format!("Host alias-{index}\n")).collect();
        let aliases = super::parse_aliases(&many);
        assert_eq!(aliases.names.len(), 256);
        assert!(aliases.limited);
    }
    #[test]
    fn host_config_reads_drop_partial_lines_and_honor_cancellation() -> anyhow::Result<()> {
        use std::sync::atomic::AtomicBool;
        let root = tempfile::tempdir()?;
        let path = root.path().join("config");
        assert!(super::read_aliases(&path, &AtomicBool::new(false))?.names.is_empty());
        assert!(super::read_aliases(&path, &AtomicBool::new(true)).is_err());
        let mut text = "Host complete\n".to_owned();
        text.push_str(&format!("#{}\n", "x".repeat(65500)));
        text.push_str(&format!("Host {}", "partial".repeat(20)));
        std::fs::write(&path, text)?;
        let aliases = super::read_aliases(&path, &AtomicBool::new(false))?;
        assert_eq!(aliases.names, ["complete"]);
        assert!(aliases.limited);
        Ok(())
    }
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
