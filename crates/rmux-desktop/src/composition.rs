use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;

/// The input method's local document. Nothing reaches the PTY until committed.
#[derive(Default, Clone)]
pub struct Composition {
    pub text: String,
    pub selected: Range<usize>,
    pub marked: Option<Range<usize>>,
}

// Clamp platform offsets and expand partial surrogate pairs to scalar boundaries.
fn boundaries(text: &str, range: Range<usize>) -> (Range<usize>, Range<usize>) {
    let len = text.encode_utf16().count();
    let start = range.start.min(len);
    let end = range.end.min(len).max(start);
    let mut offset = 0;
    let mut byte_start = text.len();
    let mut byte_end = text.len();
    let mut actual_start = len;
    let mut actual_end = len;
    for (byte, ch) in text.char_indices() {
        let next = offset + ch.len_utf16();
        if start >= offset && start < next {
            byte_start = byte;
            actual_start = offset;
        }
        if end >= offset && end < next {
            // A caret inside a surrogate pair snaps to its start.
            let expand = end != start && end != offset;
            byte_end = byte + if expand { ch.len_utf8() } else { 0 };
            actual_end = if expand { next } else { offset };
        }
        offset = next;
    }
    (byte_start..byte_end, actual_start..actual_end)
}

impl Composition {
    /// Unicode word-boundary segment at a UTF-16 position, including whitespace
    /// or punctuation when that is what was clicked.
    pub fn word_range(&self, offset: usize) -> Range<usize> {
        let byte = self.byte_range(offset..offset).start;
        let segment = self
            .text
            .split_word_bound_indices()
            .find(|(start, text)| byte < start + text.len())
            .or_else(|| self.text.split_word_bound_indices().last());
        segment.map_or(0..0, |(start, text)| {
            self.utf16_offset(start)..self.utf16_offset(start + text.len())
        })
    }
    pub fn grapheme_bounds(&self, byte: usize) -> Range<usize> {
        let byte = byte.min(self.text.len());
        self.floor_grapheme(byte)..self.ceil_grapheme(byte)
    }
    fn previous_grapheme(&self, byte: usize) -> usize {
        self.text
            .grapheme_indices(true)
            .map(|(i, _)| i)
            .take_while(|i| *i < byte)
            .last()
            .unwrap_or(0)
    }
    fn next_grapheme(&self, byte: usize) -> usize {
        self.text
            .grapheme_indices(true)
            .map(|(i, _)| i)
            .find(|i| *i > byte)
            .unwrap_or(self.text.len())
    }
    fn floor_grapheme(&self, byte: usize) -> usize {
        if byte == self.text.len() {
            byte
        } else {
            self.text
                .grapheme_indices(true)
                .map(|(i, _)| i)
                .take_while(|i| *i <= byte)
                .last()
                .unwrap_or(0)
        }
    }
    fn ceil_grapheme(&self, byte: usize) -> usize {
        self.text
            .grapheme_indices(true)
            .map(|(i, _)| i)
            .find(|i| *i >= byte)
            .unwrap_or(self.text.len())
    }
    /// Keyboard deletion removes whole user-perceived characters. Platform IME
    /// replacement ranges continue to use their original UTF-16 semantics.
    pub fn deletion_range(&self, backwards: bool) -> Range<usize> {
        let range = self.byte_range(self.selected.clone());
        let bytes = if !range.is_empty() {
            self.floor_grapheme(range.start)..self.ceil_grapheme(range.end)
        } else if backwards {
            self.previous_grapheme(range.start)..self.ceil_grapheme(range.end)
        } else {
            self.floor_grapheme(range.start)..self.next_grapheme(range.end)
        };
        self.utf16_offset(bytes.start)..self.utf16_offset(bytes.end)
    }
    /// Move a persistent editor caret by a grapheme or to a document edge.
    /// `reversed` identifies which end is active while Shift retains the anchor.
    pub fn move_caret(&mut self, backwards: bool, edge: bool, extend: bool, reversed: &mut bool) {
        self.move_caret_by(backwards, edge, false, extend, reversed);
    }
    fn word_boundary(&self, byte: usize, backwards: bool) -> usize {
        let segments = self
            .text
            .split_word_bound_indices()
            .filter(|(_, text)| !text.chars().all(char::is_whitespace));
        if backwards {
            segments
                .map(|(start, _)| start)
                .take_while(|start| *start < byte)
                .last()
                .unwrap_or(0)
        } else {
            segments
                .map(|(start, text)| start + text.len())
                .find(|end| *end > byte)
                .unwrap_or(self.text.len())
        }
    }
    /// Single-line editor deletion to an edge. A selection takes precedence,
    /// and partial IME offsets are expanded to whole graphemes.
    pub fn edge_deletion_range(&self, backwards: bool) -> Range<usize> {
        if !self.selected.is_empty() {
            return self.deletion_range(backwards);
        }
        let byte = self.byte_range(self.selected.clone()).start;
        if backwards {
            0..self.utf16_offset(self.ceil_grapheme(byte))
        } else {
            self.utf16_offset(self.floor_grapheme(byte))..self.text.encode_utf16().count()
        }
    }
    pub fn word_deletion_range(&self, backwards: bool) -> Range<usize> {
        if !self.selected.is_empty() {
            return self.deletion_range(backwards);
        }
        let byte = self.byte_range(self.selected.clone()).start;
        let target = self.word_boundary(byte, backwards);
        self.utf16_offset(self.floor_grapheme(target.min(byte)))
            ..self.utf16_offset(self.ceil_grapheme(target.max(byte)))
    }
    pub fn move_caret_by(
        &mut self,
        backwards: bool,
        edge: bool,
        word: bool,
        extend: bool,
        reversed: &mut bool,
    ) {
        let range = self.byte_range(self.selected.clone());
        let active = if *reversed { range.start } else { range.end };
        let anchor = if *reversed {
            self.selected.end
        } else {
            self.selected.start
        };
        let byte = if edge {
            if backwards { 0 } else { self.text.len() }
        } else if !extend && !range.is_empty() {
            if backwards {
                self.floor_grapheme(range.start)
            } else {
                self.ceil_grapheme(range.end)
            }
        } else if word {
            self.word_boundary(active, backwards)
        } else if backwards {
            self.previous_grapheme(active)
        } else {
            self.next_grapheme(active)
        };
        let offset = self.utf16_offset(byte);
        if extend {
            self.selected = anchor.min(offset)..anchor.max(offset);
            *reversed = offset < anchor;
        } else {
            self.selected = offset..offset;
            *reversed = false;
        }
    }
    pub fn byte_range(&self, range: Range<usize>) -> Range<usize> {
        boundaries(&self.text, range).0
    }

    pub fn utf16_offset(&self, byte: usize) -> usize {
        let mut byte = byte.min(self.text.len());
        while !self.text.is_char_boundary(byte) {
            byte -= 1;
        }
        self.text[..byte].encode_utf16().count()
    }

    pub fn substring(&self, range: Range<usize>) -> (String, Range<usize>) {
        let (bytes, actual) = boundaries(&self.text, range);
        (self.text[bytes].to_owned(), actual)
    }

    fn replace(&mut self, range: Option<Range<usize>>, text: &str) -> usize {
        let range =
            range.unwrap_or_else(|| self.marked.clone().unwrap_or_else(|| self.selected.clone()));
        let (bytes, actual) = boundaries(&self.text, range);
        self.text.replace_range(bytes, text);
        actual.start
    }

    pub fn update(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selection: Option<Range<usize>>,
    ) {
        let start = self.replace(range, text);
        let len = text.encode_utf16().count();
        let (_, selected) = boundaries(text, selection.unwrap_or(len..len));
        self.selected = start + selected.start..start + selected.end;
        self.marked = (len != 0).then_some(start..start + len);
    }

    pub fn commit(&mut self, range: Option<Range<usize>>, text: &str) -> String {
        self.replace(range, text);
        self.take()
    }

    /// Commit into a persistent editor document (for example the find field).
    pub fn insert(&mut self, range: Option<Range<usize>>, text: &str) {
        let start = self.replace(range, text);
        let end = start + text.encode_utf16().count();
        self.selected = end..end;
        self.marked = None;
    }

    pub fn take(&mut self) -> String {
        self.selected = 0..0;
        self.marked = None;
        std::mem::take(&mut self.text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edge_deletion_preserves_unicode_and_selected_text_semantics() {
        let mut c = Composition::default();
        c.insert(None, "a👩🏽‍💻z");
        let end = c.text.encode_utf16().count();
        c.selected = 1..1;
        assert_eq!(c.edge_deletion_range(true), 0..1);
        assert_eq!(c.edge_deletion_range(false), 1..end);
        c.selected = 3..3; // Inside the emoji sequence.
        assert_eq!(c.edge_deletion_range(true), 0..end - 1);
        assert_eq!(c.edge_deletion_range(false), 1..end);
        c.selected = 1..end - 1;
        assert_eq!(c.edge_deletion_range(true), 1..end - 1);
        assert_eq!(c.edge_deletion_range(false), 1..end - 1);
        c.selected = 0..0;
        assert_eq!(c.edge_deletion_range(true), 0..0);
        c.selected = end..end;
        assert_eq!(c.edge_deletion_range(false), end..end);
    }

    #[test]
    fn word_movement_and_deletion_keep_unicode_and_selection_anchors() {
        let mut c = Composition::default();
        c.insert(None, "one  cafe\u{301} 👩🏽‍💻");
        let mut reversed = false;
        let emoji_start = "one  cafe\u{301} ".encode_utf16().count();
        let end = c.selected.end;
        assert_eq!(c.word_deletion_range(true), emoji_start..end);
        c.move_caret_by(true, false, true, true, &mut reversed);
        assert_eq!(c.selected, emoji_start..end);
        c.move_caret_by(true, false, true, true, &mut reversed);
        assert_eq!(c.selected, 5..end);
        c.selected = 3..3;
        let deletion = c.word_deletion_range(false);
        assert_eq!(c.substring(deletion.clone()).0, "  cafe\u{301}");
        c.insert(Some(deletion), "");
        assert_eq!(c.text, "one 👩🏽‍💻");
    }
    #[test]
    fn word_selection_uses_unicode_segments_and_utf16_offsets() {
        let mut c = Composition::default();
        assert_eq!(c.word_range(0), 0..0);
        c.insert(None, "cafe\u{301} 👩🏽‍💻 end");
        for (offset, expected) in [(2, "cafe\u{301}"), (5, " "), (7, "👩🏽‍💻"), (100, "end")]
        {
            assert_eq!(c.substring(c.word_range(offset)).0, expected);
        }
    }
    #[test]
    fn keyboard_edits_keep_combining_emoji_and_flag_clusters_whole() {
        for cluster in ["e\u{301}", "👩🏽‍💻", "🇳🇱", "👨‍👩‍👧‍👦"] {
            let mut c = Composition::default();
            c.insert(None, &format!("a{cluster}z"));
            let end = 1 + cluster.encode_utf16().count();
            c.selected = end..end;
            let mut reversed = false;
            c.move_caret(true, false, false, &mut reversed);
            assert_eq!(c.selected, 1..1);
            c.move_caret(false, false, true, &mut reversed);
            assert_eq!(c.selected, 1..end);
            c.selected = end..end;
            assert_eq!(c.deletion_range(true), 1..end);
            c.selected = 1..1;
            assert_eq!(c.deletion_range(false), 1..end);
            // Hit testing or IME may put a caret between scalars in a cluster.
            c.selected = 2..2;
            let range = c.deletion_range(false);
            c.insert(Some(range), "");
            assert_eq!(c.text, "az");
        }
    }
    #[test]
    fn extended_selection_keeps_anchor_across_unicode_and_direction_changes() {
        let mut c = Composition::default();
        c.insert(None, "a😀z");
        let mut reversed = false;
        c.move_caret(true, false, true, &mut reversed);
        assert_eq!(c.selected, 3..4);
        c.move_caret(true, false, true, &mut reversed);
        assert_eq!(c.selected, 1..4);
        assert!(reversed);
        c.move_caret(false, false, true, &mut reversed);
        assert_eq!(c.selected, 3..4);
        c.move_caret(true, false, false, &mut reversed);
        assert_eq!(c.selected, 3..3);
        c.move_caret(true, true, true, &mut reversed);
        assert_eq!(c.selected, 0..3);
        c.move_caret(false, true, true, &mut reversed);
        assert_eq!(c.selected, 3..4);
        assert!(!reversed);
    }

    #[test]
    fn persistent_editor_commit_keeps_document_and_utf16_caret() {
        let mut c = Composition::default();
        c.insert(None, "a😀z");
        assert_eq!(c.selected, 4..4);
        c.update(Some(1..3), "に", Some(1..1));
        c.insert(None, "日本");
        assert_eq!(c.text, "a日本z");
        assert_eq!(c.selected, 3..3);
        assert!(c.marked.is_none());
    }

    #[test]
    fn composition_replaces_mark_and_preserves_selection() {
        let mut c = Composition::default();
        c.update(None, "にほん", Some(1..2));
        assert_eq!(c.selected, 1..2);
        c.update(None, "日本", Some(0..2));
        assert_eq!(c.text, "日本");
        assert_eq!(c.commit(None, "日本"), "日本");
        assert!(c.text.is_empty());
        assert_eq!(c.selected, 0..0);
        assert_eq!(c.marked, None);
    }

    #[test]
    fn explicit_replacement_keeps_surrounding_pending_text() {
        let mut c = Composition::default();
        c.update(None, "a😀z", None);
        c.update(Some(1..3), "文", Some(0..1));
        assert_eq!(c.text, "a文z");
        assert_eq!(c.selected, 1..2);
        assert_eq!(c.marked, Some(1..2));
        assert_eq!(c.commit(None, "字"), "a字z");
    }

    #[test]
    fn utf16_ranges_never_split_surrogates() {
        let mut c = Composition::default();
        c.update(None, "a😀z", Some(2..2));
        assert_eq!(c.selected, 1..1);
        assert_eq!(c.substring(2..3), ("😀".into(), 1..3));
        assert_eq!(c.substring(90..100), (String::new(), 4..4));
        assert_eq!(c.substring(3..1), (String::new(), 3..3));
        assert_eq!(c.byte_range(1..3), 1..5);
        assert_eq!(c.utf16_offset(5), 3);
        assert_eq!(c.utf16_offset(3), 1);
    }

    #[test]
    fn unmark_commits_once_and_empty_update_cancels() {
        let mut c = Composition::default();
        c.update(None, "é", None);
        assert_eq!(c.take(), "é");
        assert_eq!(c.take(), "");
        c.update(None, "draft", None);
        c.update(None, "", None);
        assert_eq!(c.take(), "");
        assert_eq!(c.commit(None, "x"), "x");
    }
}
