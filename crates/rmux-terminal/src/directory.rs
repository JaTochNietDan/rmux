//! Interpret shell-reported OSC 7 file URIs for local shell inheritance.
use std::path::PathBuf;

pub(crate) fn local_hostname() -> Option<String> {
    #[cfg(unix)]
    {
        let mut bytes = [0u8; 256];
        // SAFETY: gethostname writes at most the provided buffer length.
        if unsafe { libc::gethostname(bytes.as_mut_ptr().cast(), bytes.len()) } != 0 {
            return None;
        }
        let end = bytes.iter().position(|&byte| byte == 0)?;
        String::from_utf8(bytes[..end].to_vec()).ok()
    }
    #[cfg(not(unix))]
    {
        std::env::var("COMPUTERNAME").ok()
    }
}

pub(crate) fn local_path(uri: &str, hostname: Option<&str>) -> Option<PathBuf> {
    let (host, path) = uri.strip_prefix("file://")?.split_once('/')?;
    let normalize = |host: &str| {
        host.to_ascii_lowercase()
            .trim_end_matches('.')
            .trim_end_matches(".local")
            .to_owned()
    };
    if !host.is_empty()
        && !host.eq_ignore_ascii_case("localhost")
        && !hostname.is_some_and(|local| normalize(host) == normalize(local))
    {
        return None;
    }
    if path.contains(['?', '#']) {
        return None;
    }
    let mut decoded = vec![b'/'];
    let mut bytes = path.bytes();
    while let Some(byte) = bytes.next() {
        let byte = if byte == b'%' {
            let high = (bytes.next()? as char).to_digit(16)?;
            let low = (bytes.next()? as char).to_digit(16)?;
            (high * 16 + low) as u8
        } else {
            byte
        };
        if byte == 0 {
            return None;
        }
        decoded.push(byte);
    }
    // Do not turn remote/UNC paths into implicit network directory accesses.
    if decoded.starts_with(b"//") {
        return None;
    }
    #[cfg(unix)]
    let path = {
        use std::os::unix::ffi::OsStringExt;
        PathBuf::from(std::ffi::OsString::from_vec(decoded))
    };
    #[cfg(not(unix))]
    let path = {
        let text = String::from_utf8(decoded).ok()?;
        let text = if text.as_bytes().get(2) == Some(&b':') {
            &text[1..]
        } else {
            &text
        };
        PathBuf::from(text)
    };
    path.is_absolute().then_some(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    #[test]
    fn local_uri_decodes_paths_and_rejects_remote_or_malformed_reports() {
        assert_eq!(
            local_path("file://dev.local/tmp/hello%20world", Some("dev")),
            Some(PathBuf::from("/tmp/hello world"))
        );
        assert_eq!(
            local_path("file:///tmp/%E4%B8%96%E7%95%8C", None),
            Some(PathBuf::from("/tmp/世界"))
        );
        assert_eq!(
            local_path("file://localhost/tmp", None),
            Some(PathBuf::from("/tmp"))
        );
        for uri in [
            "file://remote/tmp",
            "https://dev/tmp",
            "file:///tmp/%00",
            "file:///tmp/%zz",
            "file:relative",
            "file:////server/share",
            "file:///tmp?a=b",
        ] {
            assert!(local_path(uri, Some("dev")).is_none(), "{uri}");
        }
    }
}
