//! Optional, bounded OS delivery. No backend is initialized until first use.
use super::*;

#[derive(Default)]
pub(super) struct Dispatcher {
    sender: Option<async_channel::Sender<Request>>,
}
impl Global for Dispatcher {}
struct Request {
    terminal: WeakEntity<TerminalView>,
    revision: u64,
}

pub(super) fn enqueue(revision: u64, cx: &mut Context<TerminalView>) {
    if !cx.global::<Preferences>().native_notifications { return; }
    if cx.global::<Dispatcher>().sender.is_none() {
        let (sender, receiver) = async_channel::bounded::<Request>(8);
        cx.global_mut::<Dispatcher>().sender = Some(sender);
        cx.spawn(async move |_, cx| {
            let mut rate = RateLimit::default();
            while let Ok(request) = receiver.recv().await {
                // Recheck settings/focus/read state after queueing. Requests hold no
                // terminal or message allocation alive after its pane is closed.
                let message = request.terminal.update(cx, |terminal, cx| {
                    if !cx.global::<Preferences>().native_notifications || terminal.input_focused { return None; }
                    terminal.notifications.iter().find(|record| record.unread && record.message.revision == request.revision)
                        .map(|record| record.message.clone())
                }).ok().flatten();
                let Some(message) = message else { continue; };
                if !rate.admit(std::time::Instant::now()) { continue; }
                let result = cx.background_executor().spawn(async move {
                    deliver(&message.title, &message.body)
                }).await;
                if let Err(error) = result {
                    let _ = request.terminal.update(cx, |terminal, cx| {
                        terminal.error = Some(format!("Desktop notification failed: {error}"));
                        cx.notify();
                    });
                }
            }
        }).detach();
    }
    // Floods retain their in-app history; OS delivery is best effort and bounded.
    let _ = cx.global::<Dispatcher>().sender.as_ref().unwrap().try_send(Request { terminal: cx.weak_entity(), revision });
}

fn deliver(title: &str, body: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        notify_rust::check_bundle()?;
        anyhow::ensure!(notify_rust::request_auth_blocking()?, "notification permission is disabled in System Settings");
    }
    let title = bounded_text(title, 256);
    let body = bounded_text(body, 4096);
    // XDG notification bodies permit markup; terminal output is always plain text.
    #[cfg(all(unix, not(target_os = "macos")))]
    let body = body.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    let mut notification = notify_rust::Notification::new();
    #[cfg(target_os = "windows")]
    notification.app_id("dev.rmux.prototype");
    notification.appname("rmux")
        .summary(if title.is_empty() { "rmux" } else { &title })
        .body(&body)
        .show()?;
    Ok(())
}

#[derive(Default)]
struct RateLimit { last: Option<std::time::Instant> }
impl RateLimit {
    fn admit(&mut self, now: std::time::Instant) -> bool {
        if self.last.is_some_and(|last| now.duration_since(last) < std::time::Duration::from_secs(1)) { return false; }
        self.last = Some(now);
        true
    }
}

fn bounded_text(text: &str, limit: usize) -> String {
    let mut result = String::new();
    for character in text.chars().filter(|c| !c.is_control() || *c == '\n' || *c == '\t') {
        if result.len() + character.len_utf8() > limit { break; }
        result.push(character);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::{bounded_text, RateLimit};
    use crate::preferences::Preferences;
    #[test]
    fn floods_do_not_schedule_delayed_native_deliveries() {
        let mut rate = RateLimit::default();
        let now = std::time::Instant::now();
        assert!(rate.admit(now));
        for _ in 0..1000 { assert!(!rate.admit(now)); }
        assert!(rate.admit(now + std::time::Duration::from_secs(1)));
    }
    #[test]
    fn native_payload_is_bounded_unicode_plain_text() {
        assert_eq!(bounded_text("a\0\x1b世界\n", 7), "a世界");
        assert_eq!(bounded_text("世界", 5), "世");
        assert_eq!(bounded_text("title\nbody\ttext", 100), "title\nbody\ttext");
        assert!(!Preferences::default().native_notifications);
        let settings: Preferences = serde_json::from_str(r#"{"native_notifications":true}"#).unwrap();
        assert!(settings.native_notifications);
    }
}
