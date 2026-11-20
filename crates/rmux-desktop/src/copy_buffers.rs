//! Explicit process-local copy buffers, independent of the OS clipboard.
use serde_json::{Value, json};
use std::collections::BTreeMap;
const MAX_COUNT: usize = 32;
pub const MAX_BUFFER: usize = 1024 * 1024;
const MAX_TOTAL: usize = 8 * 1024 * 1024;

fn preview(text: &str) -> String {
    let mut characters = text.chars();
    let mut preview: String = characters.by_ref().take(96)
        .map(|ch| if ch.is_control() || ch.is_whitespace() { ' ' } else { ch }).collect();
    if characters.next().is_some() { preview.push('…'); }
    if preview.is_empty() { preview.push_str("Empty buffer"); }
    preview
}

pub enum Command { List, Get(String), Delete(String), Set(String, String), Rename(String, String), Paste { name: String, surface_id: String, bracketed: bool } }
pub fn parse(method: &str, params: Option<&Value>) -> Result<Option<Command>, &'static str> {
    if !matches!(method, "buffer.list" | "buffer.get" | "buffer.delete" | "buffer.set" | "buffer.paste" | "buffer.rename") { return Ok(None); }
    let empty = json!({});
    let params = params.unwrap_or(&empty).as_object().ok_or("Expected object params")?;
    let allowed: &[&str] = match method { "buffer.list" => &[], "buffer.rename" => &["name", "new_name"], "buffer.set" => &["name", "text"], "buffer.paste" => &["name", "surface_id", "bracketed"], _ => &["name"] };
    if params.keys().any(|key| !allowed.contains(&key.as_str())) { return Err("Unknown buffer parameter"); }
    if method == "buffer.list" { return Ok(Some(Command::List)); }
    let name = match params.get("name") { None => "default", Some(Value::String(name)) => name, _ => return Err("Expected buffer name string") };
    validate_name(name)?;
    Ok(Some(match method {
        "buffer.rename" => {
            let new_name = params.get("new_name").and_then(Value::as_str).ok_or("Expected new_name string")?;
            validate_name(new_name)?;
            Command::Rename(name.into(), new_name.into())
        }
        "buffer.paste" => {
            let surface_id = params.get("surface_id").and_then(Value::as_str).filter(|id| !id.is_empty() && id.len() <= 128).ok_or("Expected explicit surface_id")?;
            let bracketed = match params.get("bracketed") { None => false, Some(Value::Bool(value)) => *value, _ => return Err("Expected bracketed boolean") };
            Command::Paste { name: name.into(), surface_id: surface_id.into(), bracketed }
        }
        "buffer.get" => Command::Get(name.into()),
        "buffer.delete" => Command::Delete(name.into()),
        _ => {
            let text = params.get("text").and_then(Value::as_str).ok_or("Expected buffer text string")?;
            if text.len() > MAX_BUFFER { return Err("Buffer exceeds 1 MiB"); }
            Command::Set(name.into(), text.into())
        }
    }))
}
pub fn validate_name(name: &str) -> Result<(), &'static str> {
    if name.trim().is_empty() || name.len() > 128 || name.chars().any(char::is_control) { return Err("Buffer name must be nonempty, at most 128 bytes and contain no controls"); }
    Ok(())
}
#[derive(Default)]
pub struct Store { entries: BTreeMap<String, String>, bytes: usize, revision: u64 }
impl gpui::Global for Store {}
impl Store {
    pub fn rename_if_unchanged(&mut self, name: &str, next: String, revision: u64) -> Result<Value, &'static str> {
        if self.revision != revision { return Err("Buffers changed; reopen the picker before renaming"); }
        self.execute(&Command::Rename(name.into(), next))
    }
    pub fn metadata(&self) -> Vec<(String, usize, String)> {
        self.entries.iter().map(|(name, text)| (name.clone(), text.len(), preview(text))).collect()
    }
    pub fn text_at_revision(&self, name: &str, revision: u64) -> Result<&str, &'static str> {
        if self.revision != revision { return Err("Buffers changed; reopen the picker before pasting"); }
        self.entries.get(name).map(String::as_str).ok_or("Buffer not found")
    }
    pub fn delete_if_unchanged(&mut self, name: &str, revision: u64) -> Result<Value, &'static str> {
        if self.revision != revision { return Err("Buffers changed; reopen the picker before deleting"); }
        self.execute(&Command::Delete(name.into()))
    }
    pub fn revision(&self) -> u64 { self.revision }
    pub fn save_if_unchanged(&mut self, name: String, text: String, revision: u64) -> Result<Value, &'static str> {
        if self.revision != revision { return Err("Buffers changed while saving; retry to apply this selection"); }
        self.set(name, text)
    }
    pub fn paste_input(&self, command: &Command) -> Result<Option<super::automation::SurfaceText>, &'static str> {
        let Command::Paste { name, surface_id, bracketed } = command else { return Ok(None); };
        let text = self.entries.get(name).ok_or("Buffer not found")?;
        if !bracketed && text.len() > 64 * 1024 { return Err("Raw buffer delivery exceeds 64 KiB; use bracketed paste"); }
        Ok(Some(super::automation::SurfaceText { surface_id: surface_id.clone(), text: text.clone(), paste: *bracketed }))
    }
    fn set(&mut self, name: String, text: String) -> Result<Value, &'static str> {
        validate_name(&name)?;
        let previous = self.entries.get(&name);
        if previous.is_none() && self.entries.len() >= MAX_COUNT { return Err("Buffer count limit reached; delete a buffer first"); }
        let next_bytes = self.bytes - previous.map_or(0, String::len) + text.len();
        if text.len() > MAX_BUFFER || next_bytes > MAX_TOTAL { return Err("Buffer memory limit exceeded; existing buffers unchanged"); }
        let result = json!({"name":name,"bytes":text.len()});
        self.entries.insert(name, text);
        self.bytes = next_bytes;
        self.revision = self.revision.wrapping_add(1);
        Ok(result)
    }
    pub fn execute(&mut self, command: &Command) -> Result<Value, &'static str> {
        Ok(match command {
            Command::Rename(name, new_name) => {
                validate_name(new_name)?;
                let bytes = self.entries.get(name).ok_or("Buffer not found")?.len();
                let changed = name != new_name;
                if changed {
                    if self.entries.contains_key(new_name) { return Err("Destination buffer already exists"); }
                    let text = self.entries.remove(name).expect("source checked");
                    self.entries.insert(new_name.clone(), text);
                    self.revision = self.revision.wrapping_add(1);
                }
                json!({"name":new_name,"previous_name":name,"bytes":bytes,"changed":changed})
            }
            Command::Paste { .. } => return Err("Buffer paste requires a live terminal target"),
            Command::List => json!({"buffers":self.entries.iter().map(|(name, text)| json!({"name":name,"bytes":text.len()})).collect::<Vec<_>>(), "bytes":self.bytes,"max_buffers":MAX_COUNT,"max_bytes":MAX_TOTAL,"max_buffer_bytes":MAX_BUFFER}),
            Command::Get(name) => json!({"name":name,"text":self.entries.get(name).ok_or("Buffer not found")?}),
            Command::Delete(name) => {
                let removed = self.entries.remove(name);
                if let Some(text) = &removed { self.bytes -= text.len(); self.revision = self.revision.wrapping_add(1); }
                json!({"name":name,"deleted":removed.is_some()})
            }
            Command::Set(name, text) => return self.set(name.clone(), text.clone()),
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rename_editor_rejects_stale_content_and_allows_corrected_names() {
        let mut store = Store::default();
        store.execute(&Command::Set("notes".into(), "original".into())).unwrap();
        let revision = store.revision();
        assert!(store.rename_if_unchanged("notes", "\n".into(), revision).is_err());
        store.rename_if_unchanged("notes", "renamed".into(), revision).unwrap();
        let revision = store.revision();
        store.execute(&Command::Set("renamed".into(), "replacement".into())).unwrap();
        assert!(store.rename_if_unchanged("renamed", "stale".into(), revision).is_err());
        assert_eq!(store.entries["renamed"], "replacement");
        assert!(!store.entries.contains_key("stale"));
    }
    #[test]
    fn rename_preserves_storage_and_rejects_collisions_atomically() {
        let mut store = Store::default();
        store.execute(&Command::Set("default".into(), "世界".repeat(1000))).unwrap();
        store.execute(&Command::Set("taken".into(), "keep".into())).unwrap();
        let revision = store.revision();
        let bytes = store.bytes;
        let pointer = store.entries["default"].as_ptr();
        assert!(store.execute(&Command::Rename("default".into(), "taken".into())).is_err());
        assert!(store.execute(&Command::Rename("missing".into(), "new".into())).is_err());
        assert!(store.execute(&Command::Rename("default".into(), "\n".into())).is_err());
        assert_eq!(store.revision(), revision);
        assert_eq!(store.execute(&Command::Rename("default".into(), "default".into())).unwrap()["changed"], false);
        assert_eq!(store.revision(), revision);
        let command = parse("buffer.rename", Some(&json!({"new_name":"build 世界"}))).unwrap().unwrap();
        assert_eq!(store.execute(&command).unwrap()["changed"], true);
        assert_eq!(store.entries["build 世界"].as_ptr(), pointer);
        assert_eq!(store.bytes, bytes);
        assert!(!store.entries.contains_key("default"));
        assert!(store.text_at_revision("build 世界", revision).is_err());
        for params in [json!({}), json!({"new_name":42}), json!({"new_name":""}), json!({"new_name":"x", "overwrite":true})] {
            assert!(parse("buffer.rename", Some(&params)).is_err());
        }
    }
    #[test]
    fn picker_previews_are_bounded_unicode_text_without_mutating_buffers() {
        assert_eq!(preview("line\n世界\tend\u{1b}"), "line 世界 end ");
        assert_eq!(preview(""), "Empty buffer");
        assert_eq!(preview(&"界".repeat(96)), "界".repeat(96));
        assert_eq!(preview(&"界".repeat(97)), format!("{}…", "界".repeat(96)));
        let mut store = Store::default();
        let content = format!("{}tail", "x".repeat(MAX_BUFFER - 4));
        store.execute(&Command::Set("large".into(), content.clone())).unwrap();
        let revision = store.revision();
        let entries = store.metadata();
        assert_eq!(entries[0], ("large".into(), MAX_BUFFER, format!("{}…", "x".repeat(96))));
        assert_eq!(store.text_at_revision("large", revision).unwrap(), content);
    }
    #[test]
    fn stale_picker_cannot_delete_replaced_buffer() {
        let mut store = Store::default();
        store.execute(&Command::Set("notes".into(), "old".into())).unwrap();
        let picked = store.revision();
        store.execute(&Command::Set("notes".into(), "new".into())).unwrap();
        assert!(store.delete_if_unchanged("notes", picked).is_err());
        assert_eq!(store.execute(&Command::Get("notes".into())).unwrap()["text"], "new");
        store.delete_if_unchanged("notes", store.revision()).unwrap();
        assert_eq!(store.bytes, 0);
        assert!(store.metadata().is_empty());
    }
    #[test]
    fn delayed_save_cannot_overwrite_newer_changes_and_invalid_names() {
        let mut store = Store::default();
        let revision = store.revision();
        store.execute(&Command::Set("notes".into(), "new".into())).unwrap();
        assert!(store.save_if_unchanged("notes".into(), "old".into(), revision).is_err());
        assert_eq!(store.execute(&Command::Get("notes".into())).unwrap()["text"], "new");
        let revision = store.revision();
        assert!(store.save_if_unchanged("bad\n".into(), "bad".into(), revision).is_err());
        assert_eq!(store.revision(), revision);
        assert!(store.text_at_revision("notes", revision.wrapping_sub(1)).is_err());
        assert_eq!(store.text_at_revision("notes", revision).unwrap(), "new");
        store.save_if_unchanged("notes".into(), "chosen".into(), revision).unwrap();
        assert_eq!(store.execute(&Command::Get("notes".into())).unwrap()["text"], "chosen");
    }
    #[test]
    fn buffer_paste_requires_target_and_respects_input_limits_without_consuming_text() {
        assert!(parse("buffer.paste", Some(&json!({}))).is_err());
        assert!(parse("buffer.paste", Some(&json!({"surface_id":"42", "bracketed":"true"}))).is_err());
        assert!(parse("buffer.paste", Some(&json!({"surface_id":"42", "activate":true}))).is_err());
        let mut store = Store::default();
        let command = parse("buffer.paste", Some(&json!({"surface_id":"42"}))).unwrap().unwrap();
        assert!(store.paste_input(&command).is_err());
        store.execute(&Command::Set("default".into(), "世界\n".into())).unwrap();
        let input = store.paste_input(&command).unwrap().unwrap();
        assert_eq!((input.surface_id.as_str(), input.text.as_str(), input.paste), ("42", "世界\n", false));
        store.execute(&Command::Set("default".into(), "x".repeat(64 * 1024 + 1))).unwrap();
        assert!(store.paste_input(&command).is_err());
        let bracketed = parse("buffer.paste", Some(&json!({"surface_id":"42","bracketed":true}))).unwrap().unwrap();
        assert!(store.paste_input(&bracketed).unwrap().unwrap().paste);
        assert_eq!(store.bytes, 64 * 1024 + 1);
    }
    #[test]
    fn replacement_limits_and_deletion_preserve_existing_buffers() {
        let mut store = Store::default();
        for i in 0..8 { store.execute(&Command::Set(i.to_string(), "x".repeat(MAX_BUFFER))).unwrap(); }
        assert!(store.execute(&Command::Set("extra".into(), "x".into())).is_err());
        store.execute(&Command::Set("0".into(), "世界".into())).unwrap();
        assert_eq!(store.bytes, 7 * MAX_BUFFER + 6);
        assert!(store.execute(&Command::Set("0".into(), "x".repeat(MAX_BUFFER + 1))).is_err());
        assert_eq!(store.execute(&Command::Get("0".into())).unwrap()["text"], "世界");
        assert_eq!(store.execute(&Command::Delete("0".into())).unwrap()["deleted"], true);
        assert_eq!(store.execute(&Command::Delete("0".into())).unwrap()["deleted"], false);
        for i in 8..33 { store.execute(&Command::Set(i.to_string(), String::new())).unwrap(); }
        assert!(store.execute(&Command::Set("overflow".into(), String::new())).is_err());
    }
    #[test]
    fn protocol_rejects_unknown_keys_and_preserves_literal_text() {
        assert!(parse("buffer.list", Some(&json!({"text":"bad"}))).is_err());
        assert!(parse("buffer.set", Some(&json!({"name":"bad\n", "text":"x"}))).is_err());
        let command = parse("buffer.set", Some(&json!({"text":"$(literal)\n世界"}))).unwrap().unwrap();
        let mut store = Store::default();
        store.execute(&command).unwrap();
        assert_eq!(store.execute(&Command::Get("default".into())).unwrap()["text"], "$(literal)\n世界");
    }
}
