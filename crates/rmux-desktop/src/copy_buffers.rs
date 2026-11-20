//! Explicit process-local copy buffers, independent of the OS clipboard.
use serde_json::{Value, json};
use std::collections::BTreeMap;
const MAX_COUNT: usize = 32;
const MAX_BUFFER: usize = 1024 * 1024;
const MAX_TOTAL: usize = 8 * 1024 * 1024;

pub enum Command { List, Get(String), Delete(String), Set(String, String) }
pub fn parse(method: &str, params: Option<&Value>) -> Result<Option<Command>, &'static str> {
    if !matches!(method, "buffer.list" | "buffer.get" | "buffer.delete" | "buffer.set") { return Ok(None); }
    let empty = json!({});
    let params = params.unwrap_or(&empty).as_object().ok_or("Expected object params")?;
    let allowed: &[&str] = match method { "buffer.list" => &[], "buffer.set" => &["name", "text"], _ => &["name"] };
    if params.keys().any(|key| !allowed.contains(&key.as_str())) { return Err("Unknown buffer parameter"); }
    if method == "buffer.list" { return Ok(Some(Command::List)); }
    let name = match params.get("name") { None => "default", Some(Value::String(name)) => name, _ => return Err("Expected buffer name string") };
    if name.trim().is_empty() || name.len() > 128 || name.chars().any(char::is_control) { return Err("Buffer name must be nonempty, at most 128 bytes and contain no controls"); }
    Ok(Some(match method {
        "buffer.get" => Command::Get(name.into()),
        "buffer.delete" => Command::Delete(name.into()),
        _ => {
            let text = params.get("text").and_then(Value::as_str).ok_or("Expected buffer text string")?;
            if text.len() > MAX_BUFFER { return Err("Buffer exceeds 1 MiB"); }
            Command::Set(name.into(), text.into())
        }
    }))
}
#[derive(Default)]
pub struct Store { entries: BTreeMap<String, String>, bytes: usize }
impl gpui::Global for Store {}
impl Store {
    pub fn execute(&mut self, command: &Command) -> Result<Value, &'static str> {
        Ok(match command {
            Command::List => json!({"buffers":self.entries.iter().map(|(name, text)| json!({"name":name,"bytes":text.len()})).collect::<Vec<_>>(), "bytes":self.bytes,"max_buffers":MAX_COUNT,"max_bytes":MAX_TOTAL,"max_buffer_bytes":MAX_BUFFER}),
            Command::Get(name) => json!({"name":name,"text":self.entries.get(name).ok_or("Buffer not found")?}),
            Command::Delete(name) => {
                let removed = self.entries.remove(name);
                if let Some(text) = &removed { self.bytes -= text.len(); }
                json!({"name":name,"deleted":removed.is_some()})
            }
            Command::Set(name, text) => {
                let previous = self.entries.get(name);
                if previous.is_none() && self.entries.len() >= MAX_COUNT { return Err("Buffer count limit reached; delete a buffer first"); }
                let next_bytes = self.bytes - previous.map_or(0, String::len) + text.len();
                if text.len() > MAX_BUFFER || next_bytes > MAX_TOTAL { return Err("Buffer memory limit exceeded; existing buffers unchanged"); }
                self.entries.insert(name.clone(), text.clone());
                self.bytes = next_bytes;
                json!({"name":name,"bytes":text.len()})
            }
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
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
