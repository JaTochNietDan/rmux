//! Bounded OSC 99 text assembly. Ghostty owns framing and cancellation.
use base64::Engine;
use crate::NotificationVisibility;
use std::collections::VecDeque;
#[derive(Default)]
struct Message { id: String, title: String, body: String, visibility: NotificationVisibility }
#[derive(Default)]
pub(super) struct Assembler { pending: VecDeque<Message> }
fn append(target: &mut String, text: &str, limit: usize) {
    for ch in text.chars().filter(|ch| !ch.is_control()) {
        if target.len() + ch.len_utf8() > limit { break; }
        target.push(ch);
    }
}
impl Assembler {
    pub fn receive(&mut self, content: &[u8]) -> Option<(String, String, NotificationVisibility)> {
        let content = std::str::from_utf8(content).ok()?.strip_prefix("99;")?;
        let (metadata, payload) = content.split_once(';')?;
        let (mut id, mut kind, mut done, mut encoded) = ("", "title", true, false);
        let mut visibility = None;
        for field in metadata.split(':').filter(|field| !field.is_empty()) {
            let (key, value) = field.split_once('=')?;
            match key {
                "i" => id = value,
                "o" => visibility = Some(match value {
                    "always" => NotificationVisibility::Always,
                    "unfocused" => NotificationVisibility::Unfocused,
                    "invisible" => NotificationVisibility::Invisible,
                    _ => return None,
                }),
                "p" => kind = value,
                "d" => done = match value { "0" => false, "1" => true, _ => return None },
                "e" => encoded = match value { "0" => false, "1" => true, _ => return None },
                _ => {},
            }
        }
        if id.len() > 128 || !id.bytes().all(|b| b.is_ascii_graphic()) { return None; }
        if !matches!(kind, "title" | "body") { return None; }
        if payload.len() > if encoded { 4096 } else { 2048 } { return None; }
        let decoded;
        let text = if encoded {
            decoded = base64::engine::general_purpose::STANDARD.decode(payload).ok()?;
            if decoded.len() > 2048 { return None; }
            std::str::from_utf8(&decoded).ok()?
        } else { payload };
        let mut message = self.pending.iter().position(|message| message.id == id)
            .and_then(|index| self.pending.remove(index)).unwrap_or_else(|| Message { id: id.into(), ..Message::default() });
        if let Some(visibility) = visibility { message.visibility = visibility; }
        if kind == "title" { append(&mut message.title, text, 256); }
        else { append(&mut message.body, text, 1024); }
        if !done {
            if self.pending.len() == 16 { self.pending.pop_front(); }
            self.pending.push_back(message);
            return None;
        }
        if message.title.is_empty() && message.body.is_empty() { return None; }
        if message.title.is_empty() { append(&mut message.title, &message.body, 256); message.body.clear(); }
        Some((message.title, message.body, message.visibility))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn visibility_persists_across_chunks_and_rejects_unknown_values() {
        let mut assembler = Assembler::default();
        assert!(assembler.receive(b"99;i=job:o=invisible:d=0;Build").is_none());
        assert_eq!(assembler.receive(b"99;i=job:p=body;Done").unwrap().2, NotificationVisibility::Invisible);
        assert!(assembler.receive(b"99;o=sometimes;Ignored").is_none());
        for focused in [false, true] {
            for visible in [false, true] {
                assert!(NotificationVisibility::Always.allows(focused, visible));
                assert_eq!(NotificationVisibility::Unfocused.allows(focused, visible), !focused);
                assert_eq!(NotificationVisibility::Invisible.allows(focused, visible), !focused && !visible);
            }
        }
        assert_eq!(assembler.receive(b"99;o=unfocused;Done").unwrap().2, NotificationVisibility::Unfocused);
        assert_eq!(assembler.receive(b"99;;Default").unwrap().2, NotificationVisibility::Always);
    }
    #[test]
    fn chunks_encoding_and_limits() {
        let mut assembler = Assembler::default();
        assert!(assembler.receive(b"99;i=build:d=0;Build ").is_none());
        assert!(assembler.receive(b"99;i=other:d=0;Other").is_none());
        assert!(assembler.receive(b"99;i=build:d=0;complete").is_none());
        assert_eq!(assembler.receive(b"99;i=build:p=body:e=1;dGVzdHMgcGFzc2Vk"), Some(("Build complete".into(), "tests passed".into(), NotificationVisibility::Always)));
        assert!(assembler.receive(b"99;i=other:p=icon;ignored").is_none());
        assert_eq!(assembler.receive(b"99;i=other:p=body;details"), Some(("Other".into(), "details".into(), NotificationVisibility::Always)));
        assert!(assembler.receive(b"99;e=1;!invalid!").is_none());
        for i in 0..100 { assembler.receive(format!("99;i={i}:d=0;pending").as_bytes()); }
        assert_eq!(assembler.pending.len(), 16);
        let text = "世".repeat(600);
        let result = assembler.receive(format!("99;;{text}").as_bytes()).unwrap();
        assert!(result.0.len() <= 256);
    }
}
