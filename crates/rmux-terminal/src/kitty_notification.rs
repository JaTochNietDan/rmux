//! Bounded OSC 99 text assembly. Ghostty owns framing and cancellation.
use base64::Engine;
use crate::NotificationVisibility;
use std::collections::VecDeque;

fn valid_id(id: &str) -> bool {
    id.len() <= 128 && id.bytes().all(|byte| byte.is_ascii_alphanumeric() || b"_-+.".contains(&byte))
}

/// Empty query payloads only: never answer a capability response with another
/// response. Restrict the echoed identifier to the protocol's safe alphabet.
pub(super) fn query_response(content: &[u8]) -> Option<String> {
    let content = std::str::from_utf8(content).ok()?.strip_prefix("99;")?;
    let (metadata, payload) = content.split_once(';')?;
    if !payload.is_empty() { return None; }
    let (mut id, mut kind) = ("", "title");
    for field in metadata.split(':').filter(|field| !field.is_empty()) {
        let (key, value) = field.split_once('=')?;
        match key { "i" => id = value, "p" => kind = value, _ => {} }
    }
    if kind != "?" || !valid_id(id) { return None; }
    let identity = if id.is_empty() { String::new() } else { format!("i={id}:") };
    Some(format!("\x1b]99;{identity}p=?;p=title,body,?:o=always,unfocused,invisible"))
}
#[derive(Default)]
struct Message { id: String, title: Text, body: Text, visibility: NotificationVisibility }
/// At most three Base64 characters and three UTF-8 bytes cross chunk boundaries.
#[derive(Default)]
struct Text {
    value: String,
    encoded_tail: Vec<u8>,
    utf8_tail: Vec<u8>,
    full: bool,
}
impl Text {
    fn decoded(&mut self, bytes: &[u8], limit: usize) -> Option<()> {
        self.utf8_tail.extend_from_slice(bytes);
        let valid = match std::str::from_utf8(&self.utf8_tail) {
            Ok(_) => self.utf8_tail.len(),
            Err(error) if error.error_len().is_none() => error.valid_up_to(),
            Err(_) => return None,
        };
        if !self.full {
            for ch in std::str::from_utf8(&self.utf8_tail[..valid]).ok()?.chars().filter(|ch| !ch.is_control()) {
                if self.value.len() + ch.len_utf8() > limit { self.full = true; break; }
                self.value.push(ch);
            }
        }
        self.utf8_tail.drain(..valid);
        Some(())
    }
    fn flush_encoding(&mut self, limit: usize) -> Option<()> {
        if !self.encoded_tail.is_empty() {
            let mut decoded = [0; 3];
            let count = base64::engine::general_purpose::STANDARD_NO_PAD.decode_slice(&self.encoded_tail, &mut decoded).ok()?;
            self.encoded_tail.clear();
            self.decoded(&decoded[..count], limit)?;
        }
        Some(())
    }
    fn feed(&mut self, payload: &str, encoded: bool, limit: usize) -> Option<()> {
        if !encoded {
            self.flush_encoding(limit)?;
            // Plain payloads are already valid UTF-8. Avoid retaining a full
            // payload-sized scratch allocation just to append bounded text.
            if !payload.is_empty() && !self.utf8_tail.is_empty() { return None; }
            if !self.full {
                for ch in payload.chars().filter(|ch| !ch.is_control()) {
                    if self.value.len() + ch.len_utf8() > limit { self.full = true; break; }
                    self.value.push(ch);
                }
            }
            return Some(());
        }
        for byte in payload.bytes() {
            if !(byte.is_ascii_alphanumeric() || b"+/=".contains(&byte)) { return None; }
            self.encoded_tail.push(byte);
            if self.encoded_tail.len() == 4 {
                let mut decoded = [0; 3];
                let count = base64::engine::general_purpose::STANDARD.decode_slice(&self.encoded_tail, &mut decoded).ok()?;
                self.encoded_tail.clear();
                self.decoded(&decoded[..count], limit)?;
            }
        }
        Some(())
    }
    fn finish(&mut self, limit: usize) -> Option<()> {
        self.flush_encoding(limit)?;
        self.utf8_tail.is_empty().then_some(())
    }
}
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
        if !valid_id(id) { return None; }
        if !matches!(kind, "title" | "body") { return None; }
        if payload.len() > if encoded { 4096 } else { 2048 } { return None; }
        let mut message = self.pending.iter().position(|message| message.id == id)
            .and_then(|index| self.pending.remove(index)).unwrap_or_else(|| Message { id: id.into(), ..Message::default() });
        if let Some(visibility) = visibility { message.visibility = visibility; }
        if kind == "title" { message.title.feed(payload, encoded, 256)?; }
        else { message.body.feed(payload, encoded, 1024)?; }
        if !done {
            if self.pending.len() == 16 { self.pending.pop_front(); }
            self.pending.push_back(message);
            return None;
        }
        message.title.finish(256)?;
        message.body.finish(1024)?;
        let (mut title, mut body) = (message.title.value, message.body.value);
        if title.is_empty() && body.is_empty() { return None; }
        if title.is_empty() { append(&mut title, &body, 256); body.clear(); }
        Some((title, body, message.visibility))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn base64_quartets_and_unicode_can_cross_chunks() {
        let text = "Build 世界 🚀 complete";
        let encoded = base64::engine::general_purpose::STANDARD.encode(text);
        for split in 0..=encoded.len() {
            let mut assembler = Assembler::default();
            assert!(assembler.receive(format!("99;i=job:e=1:d=0;{}", &encoded[..split]).as_bytes()).is_none());
            let message = assembler.receive(format!("99;i=job:e=1;{}", &encoded[split..]).as_bytes()).unwrap();
            assert_eq!(message.0, text, "split {split}");
        }
        let mut assembler = Assembler::default();
        for byte in encoded.trim_end_matches('=').bytes() {
            assert!(assembler.receive(format!("99;i=job:e=1:d=0;{}", byte as char).as_bytes()).is_none());
        }
        assert_eq!(assembler.receive(b"99;i=job:p=body;Done").unwrap().0, text);
        // Independently encoded padded chunks are also valid.
        assert!(assembler.receive(b"99;i=job:e=1:d=0;QQ==").is_none());
        assert_eq!(assembler.receive(b"99;i=job:e=1;Qg==").unwrap().0, "AB");
        assert!(assembler.receive(b"99;i=bad:e=1;4g==").is_none(), "incomplete UTF-8");
        assert!(assembler.receive(b"99;i=bad:e=1;A").is_none(), "incomplete Base64");
        let mut value = Text::default();
        value.feed(&base64::engine::general_purpose::STANDARD.encode("世".repeat(1000)), true, 256).unwrap();
        assert!(value.value.len() <= 256);
        assert!(value.encoded_tail.len() <= 3 && value.utf8_tail.len() <= 3);
    }
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
        assert_eq!(assembler.receive(b"99;e=1;SGk").unwrap().0, "Hi");
        for i in 0..100 { assembler.receive(format!("99;i={i}:d=0;pending").as_bytes()); }
        assert_eq!(assembler.pending.len(), 16);
        let text = "世".repeat(600);
        let result = assembler.receive(format!("99;;{text}").as_bytes()).unwrap();
        assert!(result.0.len() <= 256);
    }
}
