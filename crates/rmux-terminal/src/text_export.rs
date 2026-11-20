//! Bounded plain-text export of the active screen and its retained history.
use super::*;

struct Output {
    bytes: Vec<u8>,
    limit: usize,
    truncated: bool,
    allocation_failed: bool,
}

unsafe extern "C" fn write_output(userdata: *mut c_void, data: *const u8, len: usize) -> bool {
    // Ghostty calls synchronously with a valid context and borrowed byte slice.
    let output = unsafe { &mut *userdata.cast::<Output>() };
    let bytes = unsafe { std::slice::from_raw_parts(data, len) };
    let accepted = len.min(output.limit.saturating_sub(output.bytes.len()));
    let required = output.bytes.len() + accepted;
    if required > output.bytes.capacity() {
        // Formatter callbacks can be as small as one character. Grow
        // geometrically rather than reallocating for each callback, while
        // keeping the requested capacity within the caller's export limit.
        let capacity = required
            .max(output.bytes.capacity().saturating_mul(2))
            .max(4096)
            .min(output.limit);
        if output
            .bytes
            .try_reserve_exact(capacity - output.bytes.len())
            .is_err()
        {
            output.allocation_failed = true;
            return false;
        }
    }
    output.bytes.extend_from_slice(&bytes[..accepted]);
    output.truncated |= accepted != len;
    !output.truncated
}

impl Terminal {
    /// Exports the active screen, including retained history, without selecting
    /// text or moving the viewport. Soft-wrapped rows are unwrapped.
    ///
    /// Returns the oldest text first, stopping at `max_bytes` (at most 16 MiB)
    /// and dropping any incomplete final UTF-8 code point. The boolean reports
    /// whether output was truncated. This is synchronous: callers must arrange
    /// background execution rather than hold up a UI thread for large exports.
    pub fn history_text_limited(&self, max_bytes: usize) -> Result<(String, bool)> {
        ensure!(max_bytes <= 16 * 1024 * 1024, "history export limit exceeds 16 MiB");
        self.export_text_limited(max_bytes, false).map(|text| text.expect("history always has a screen"))
    }

    /// Stream only the current selection, preserving UTF-8 at the byte limit.
    pub fn selection_text_prefix(&self, max_bytes: usize) -> Result<Option<(String, bool)>> {
        ensure!(max_bytes <= 16 * 1024 * 1024, "history export limit exceeds 16 MiB");
        self.export_text_limited(max_bytes, true)
    }

    // Shared with the complete-selection API, which rejects truncated output.
    pub(super) fn export_text_limited(&self, max_bytes: usize, selected: bool) -> Result<Option<(String, bool)>> {
        struct Formatter(GhosttyFormatter);
        impl Drop for Formatter {
            fn drop(&mut self) {
                unsafe { ghostty_formatter_free(self.0) };
            }
        }
        let mut output = Output {
            bytes: Vec::new(),
            limit: max_bytes,
            truncated: false,
            allocation_failed: false,
        };
        // The immutable engine borrow excludes writes throughout formatting.
        unsafe {
            let mut selection = GhosttySelection::default();
            if selected {
                let result = ghostty_terminal_get(self.terminal, GHOSTTY_TERMINAL_DATA_SELECTION, out(&mut selection));
                if result == GHOSTTY_NO_VALUE { return Ok(None); }
                check(result)?;
            }
            let mut formatter = Formatter(ptr::null_mut());
            check(ghostty_formatter_terminal_new(
                ptr::null(),
                &mut formatter.0,
                self.terminal,
            ))?;
            if selected {
                check(ghostty_formatter_set(formatter.0, GHOSTTY_FORMATTER_OPT_SELECTION, input(&selection)))?;
            }
            check(ghostty_formatter_set(
                formatter.0,
                GHOSTTY_FORMATTER_OPT_UNWRAP,
                input(&true),
            ))?;
            let result = ghostty_formatter_format(
                formatter.0,
                GhosttyWriter {
                    write: Some(write_output),
                    userdata: (&mut output as *mut Output).cast(),
                },
            );
            ensure!(
                !output.allocation_failed,
                "could not allocate history export"
            );
            if !(output.truncated && result == GHOSTTY_IO_ERROR) {
                check(result)?;
            }
        }
        if let Err(error) = std::str::from_utf8(&output.bytes) {
            ensure!(
                output.truncated && error.error_len().is_none(),
                "formatter returned invalid UTF-8"
            );
            output.bytes.truncate(error.valid_up_to());
        }
        Ok(Some((String::from_utf8(output.bytes)?, output.truncated)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selection_prefix_is_bounded_and_preserves_selection_and_viewport() -> Result<()> {
        let mut terminal = Terminal::new(20, 3)?;
        terminal.write("世界 end\r\nnext".as_bytes());
        assert!(terminal.selection_text_prefix(5)?.is_none());
        terminal.select_all()?;
        let selected = terminal.selected_text()?.unwrap();
        let before = terminal.frame()?;
        assert_eq!(terminal.selection_text_prefix(5)?, Some(("世".into(), true)));
        assert_eq!(terminal.selection_text_prefix(1024)?, Some((selected.clone(), false)));
        assert_eq!(terminal.selected_text()?, Some(selected));
        let after = terminal.frame()?;
        assert_eq!(after.selections, before.selections);
        assert_eq!(after.scrollback_offset, before.scrollback_offset);
        Ok(())
    }

    #[test]
    fn small_writes_grow_geometrically_and_stop_at_limit() {
        let mut output = Output {
            bytes: Vec::new(),
            limit: 10_000,
            truncated: false,
            allocation_failed: false,
        };
        let mut growths = 0;
        for _ in 0..output.limit {
            let capacity = output.bytes.capacity();
            assert!(unsafe { write_output((&mut output as *mut Output).cast(), b"x".as_ptr(), 1) });
            growths += usize::from(output.bytes.capacity() != capacity);
        }
        assert!(growths <= 3, "{growths} allocations for one-byte writes");
        assert!(!unsafe { write_output((&mut output as *mut Output).cast(), b"x".as_ptr(), 1) });
        assert_eq!(output.bytes.len(), output.limit);
        assert!(output.truncated && !output.allocation_failed);
    }
    #[test]
    fn history_export_preserves_viewport_and_selection_and_is_screen_local() -> Result<()> {
        let mut terminal = Terminal::new(12, 2)?;
        terminal.write("first 世界\r\nsecond\r\nthird\r\nfourth".as_bytes());
        terminal.scroll(Scroll::Top)?;
        terminal.selection_event(selection::Event {
            action: selection::Action::Press,
            x: 10.,
            y: 0.,
            cell_width: 10.,
            cell_height: 20.,
            rectangle: false,
            clicks: 2,
        })?;
        let selected = terminal.selected_text()?;
        let before = terminal.frame()?;
        let (text, truncated) = terminal.history_text_limited(4096)?;
        assert!(
            text.starts_with("first 世界\nsecond\nthird\nfourth"),
            "{text:?}"
        );
        assert!(!truncated);
        assert_eq!(terminal.selected_text()?, selected);
        let after = terminal.frame()?;
        assert_eq!(before.scrollback_offset, after.scrollback_offset);
        assert_eq!(before.text(), after.text());
        terminal.write(b"\x1b[?1049hALT");
        let (alternate, _) = terminal.history_text_limited(4096)?;
        assert!(alternate.contains("ALT") && !alternate.contains("first"));
        Ok(())
    }
    #[test]
    fn history_export_stops_at_utf8_safe_byte_limit() -> Result<()> {
        let mut terminal = Terminal::new(8, 2)?;
        terminal.write("世界abcdefghijk".as_bytes());
        let (full, truncated) = terminal.history_text_limited(4096)?;
        assert!(
            !truncated && full.starts_with("世界abcdefghijk"),
            "{full:?}"
        );
        for limit in 0..=full.len() {
            let (prefix, truncated) = terminal.history_text_limited(limit)?;
            assert!(prefix.len() <= limit && full.starts_with(&prefix));
            assert_eq!(truncated, limit < full.len());
        }
        Ok(())
    }
}
