//! Stable, bounded history inspection; no per-frame history formatting.
use super::*;
const MAX_BYTES: usize = 8 * 1024 * 1024;
const MAX_LINES: usize = 65_536;

struct Buffer {
    text: String,
    lines: Vec<Range<usize>>,
    truncated: bool,
}
impl Buffer {
    fn new(mut text: String, mut truncated: bool) -> Self {
        let mut lines = Vec::new();
        let mut start = 0;
        for (index, byte) in text.bytes().enumerate() {
            if byte == b'\n' {
                lines.push(start..index);
                start = index + 1;
                if lines.len() == MAX_LINES { break; }
            }
        }
        if lines.len() == MAX_LINES {
            truncated |= start < text.len();
            text.truncate(start);
        } else { lines.push(start..text.len()); }
        Self { text, lines, truncated }
    }
    fn selected(&self, cursor: usize, anchor: Option<usize>) -> &str {
        let anchor = anchor.unwrap_or(cursor);
        let first = cursor.min(anchor).min(self.lines.len() - 1);
        let last = cursor.max(anchor).min(self.lines.len() - 1);
        &self.text[self.lines[first].start..self.lines[last].end]
    }
}

pub(super) struct CopyMode {
    pub focus: FocusHandle,
    buffer: Option<Buffer>,
    message: String,
    cursor: usize,
    anchor: Option<usize>,
    scroll: ListState,
    _load: Task<()>,
}
pub(super) struct Close;
impl EventEmitter<Close> for CopyMode {}
impl CopyMode {
    pub fn new(session: &Session, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let export = session.history_export(MAX_BYTES);
        let executor = cx.background_executor().clone();
        let load = cx.spawn(async move |entity, cx| {
            let result = executor.spawn(async move {
                export().map(|value| value.map(|(text, truncated)| Buffer::new(text, truncated)))
            }).await;
            let _ = entity.update(cx, |this, cx| {
                match result {
                    Ok(Some(buffer)) => {
                        this.cursor = buffer.lines.len().saturating_sub(1);
                        this.scroll.reset(buffer.lines.len());
                        this.scroll.scroll_to_reveal_item(this.cursor);
                        this.message = if buffer.truncated { "Snapshot truncated · First 8 MiB / 65,536 lines" } else { "Snapshot · Output continues in the terminal" }.into();
                        this.buffer = Some(buffer);
                    }
                    Ok(None) => this.message = "Terminal busy. Close and reopen copy mode to retry.".into(),
                    Err(error) => this.message = error.to_string(),
                }
                cx.notify();
            });
        });
        let focus = cx.focus_handle();
        focus.focus(window);
        Self { focus, buffer: None, message: "Reading terminal history…".into(), cursor: 0,
            anchor: None, scroll: ListState::new(0, ListAlignment::Bottom, px(128.)), _load: load }
    }
    fn copy(&self, cx: &mut Context<Self>) {
        if let Some(buffer) = &self.buffer {
            clipboard::write(ClipboardItem::new_string(buffer.selected(self.cursor, self.anchor).to_owned()), cx);
        }
    }
    fn key(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let key = &event.keystroke;
        if key.key == "escape" { cx.emit(Close); }
        else if !key.modifiers.platform && !key.modifiers.control && !key.modifiers.alt {
            let count = self.buffer.as_ref().map_or(0, |buffer| buffer.lines.len());
            if count > 0 {
                let next = match key.key.as_str() {
                    "up" | "k" => Some(self.cursor.saturating_sub(1)),
                    "down" | "j" => Some((self.cursor + 1).min(count - 1)),
                    "pageup" => Some(self.cursor.saturating_sub(20)),
                    "pagedown" => Some((self.cursor + 20).min(count - 1)),
                    "home" => Some(0), "end" => Some(count - 1), _ => None,
                };
                if let Some(next) = next {
                    if key.modifiers.shift && self.anchor.is_none() { self.anchor = Some(self.cursor); }
                    self.cursor = next;
                    self.scroll.scroll_to_reveal_item(next);
                } else { match key.key.as_str() {
                    "space" | "v" => self.anchor = if self.anchor.is_some() { None } else { Some(self.cursor) },
                    "enter" | "y" => self.copy(cx),
                    _ => {},
                } }
            }
        }
        cx.stop_propagation();
        cx.notify();
    }
    fn row(&self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let Some(buffer) = &self.buffer else { return div().into_any_element(); };
        let range = &buffer.lines[index];
        let text = &buffer.text[range.clone()];
        // A pathological single line must not cause unbounded shaping work.
        let mut end = text.len().min(4096);
        while !text.is_char_boundary(end) { end -= 1; }
        let display = if end < text.len() { format!("{}… [display clipped; copy includes full line]", &text[..end]) } else { text.to_owned() };
        let anchor = self.anchor.unwrap_or(self.cursor);
        let selected = (self.cursor.min(anchor)..=self.cursor.max(anchor)).contains(&index);
        div().id(("copy-line", index)).w_full().min_h(px(22.)).px_2().py_1().text_size(px(12.)).font_family(FONT_FAMILY)
            .when(selected, |row| row.bg(rgb(0x354969)))
            .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                if event.standard_click() {
                    if event.modifiers().shift && this.anchor.is_none() { this.anchor = Some(this.cursor); }
                    else if !event.modifiers().shift { this.anchor = None; }
                    this.cursor = index;
                    this.focus.focus(window);
                    cx.notify();
                }
            }))
            .flex().gap_2()
            .child(div().w(px(54.)).flex_shrink_0().text_color(rgb(0x909090)).child((index + 1).to_string()))
            .child(div().min_w_0().flex_1().child(display)).into_any_element()
    }
}
impl Render for CopyMode {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        div().absolute().inset_0().size_full().occlude().bg(rgb(0x242424)).text_color(rgb(0xd6d6d6))
            .flex().flex_col().track_focus(&self.focus).key_context("CopyMode")
            .on_key_down(cx.listener(Self::key))
            .on_action(|_: &Paste, _, cx| cx.stop_propagation())
            .on_action(cx.listener(|this, _: &EditorSelectAll, _, cx| {
                if let Some(buffer) = &this.buffer {
                    this.anchor = Some(0);
                    this.cursor = buffer.lines.len() - 1;
                    this.scroll.scroll_to_reveal_item(this.cursor);
                    cx.notify();
                }
                cx.stop_propagation();
            }))
            .on_action(cx.listener(|this, _: &CopySelectionWithLineBreaks, _, cx| { this.copy(cx); cx.stop_propagation(); }))
            .on_action(cx.listener(|this, _: &CopySelection, _, cx| { this.copy(cx); cx.stop_propagation(); }))
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| { this.focus.focus(window); cx.stop_propagation(); }))
            .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
            .on_mouse_down(MouseButton::Middle, |_, _, cx| cx.stop_propagation())
            .on_mouse_move(|_, _, cx| cx.stop_propagation())
            .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
            .child(div().px_3().py_2().flex().items_center().gap_2().border_b_1().border_color(rgb(0x383838))
                .child(div().flex_1().min_w_0().text_sm().child("Copy mode"))
                .child(pane_view::pane_control("close-copy-mode", "Close Copy Mode")
                    .on_click(cx.listener(|_, event: &ClickEvent, _, cx| { if event.standard_click() { cx.emit(Close); } }))
                    .child(icons::icon("close"))))
            .child(div().px_3().py_1().text_xs().child(self.message.clone()))
            .child(list(self.scroll.clone(), move |index, _, cx| entity.update(cx, |this, cx| this.row(index, cx)))
                .flex_1().min_h_0().w_full())
            .child(div().px_3().py_2().text_xs().border_t_1().border_color(rgb(0x383838))
                .child("↑↓ / j k move · Shift or Space select lines · Enter / y copy · Esc close"))
    }
}

#[cfg(test)]
mod tests {
    use super::{Buffer, MAX_LINES};
    #[test]
    fn snapshot_selection_keeps_unicode_empty_lines_and_reverse_ranges() {
        let buffer = Buffer::new("first 世界\n\nlast 👩‍💻".into(), false);
        assert_eq!(buffer.selected(2, Some(0)), "first 世界\n\nlast 👩‍💻");
        assert_eq!(buffer.selected(0, Some(2)), buffer.selected(2, Some(0)));
        assert_eq!(buffer.selected(1, None), "");
        assert_eq!(Buffer::new(String::new(), false).selected(0, None), "");
    }
    #[test]
    fn snapshot_line_limit_discards_uncopyable_tail() {
        let buffer = Buffer::new("x\n".repeat(MAX_LINES + 1), false);
        assert!(buffer.truncated);
        assert_eq!(buffer.lines.len(), MAX_LINES);
        assert_eq!(buffer.text.len(), MAX_LINES * 2);
        assert_eq!(buffer.selected(MAX_LINES - 1, None), "x");
    }
}
