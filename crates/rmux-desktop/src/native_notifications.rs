//! Optional, bounded OS delivery. No backend is initialized until first use.
use super::*;

#[derive(Default)]
pub(super) struct Dispatcher {
    sender: Option<async_channel::Sender<Request>>,
    active_windows: std::collections::HashSet<AnyWindowHandle>,
    #[cfg(unix)]
    responses: Vec<(Arc<AtomicBool>, Task<()>)>,
}
impl Global for Dispatcher {}
struct Request {
    terminal: WeakEntity<TerminalView>,
    revision: u64,
}

// Use activation events: GPUI's macOS active_window() returns mainWindow, which
// may remain set when another app owns foreground focus.
pub(super) fn record_activation(window: &Window, cx: &mut App) {
    let live = cx.windows();
    let active = &mut cx.global_mut::<Dispatcher>().active_windows;
    active.retain(|handle| live.contains(handle));
    let handle = window.window_handle();
    if window.is_window_active() { active.insert(handle); } else { active.remove(&handle); }
}
fn allowed(terminal_focused: bool, cx: &App) -> bool {
    let settings = cx.global::<Preferences>();
    if !settings.native_notifications { return false; }
    let active = &cx.global::<Dispatcher>().active_windows;
    let app_active = cx.windows().iter().any(|handle| active.contains(handle));
    settings.notification_focus.allows(terminal_focused, app_active)
}

pub(super) fn enqueue(revision: u64, cx: &mut Context<TerminalView>) {
    if !allowed(false, cx) { return; }
    if cx.global::<Dispatcher>().sender.is_none() {
        let (sender, receiver) = async_channel::bounded::<Request>(8);
        cx.global_mut::<Dispatcher>().sender = Some(sender);
        cx.spawn(async move |_, cx| {
            let mut rate = RateLimit::default();
            while let Ok(request) = receiver.recv().await {
                // Recheck settings/focus/read state after queueing. Requests hold no
                // terminal or message allocation alive after its pane is closed.
                let message = request.terminal.update(cx, |terminal, cx| {
                    if !allowed(terminal.input_focused, cx) { return None; }
                    terminal.notifications.iter().find(|record| record.unread && record.message.revision == request.revision)
                        .map(|record| record.message.clone())
                }).ok().flatten();
                let Some(message) = message else { continue; };
                #[cfg(unix)]
                if !cx.update(|cx| {
                    let pending = &mut cx.global_mut::<Dispatcher>().responses;
                    pending.retain(|(done, _)| !done.load(Ordering::Acquire));
                    pending.len() < 16
                }).unwrap_or(false) { continue; }
                if !rate.admit(std::time::Instant::now()) { continue; }
                let result = cx.background_executor().spawn(async move {
                    deliver(&message.title, &message.body)
                }).await;
                match result {
                    Ok(delivery) => {
                        #[cfg(unix)]
                        let _ = cx.update(|cx| observe_response(delivery, request, cx));
                        #[cfg(not(unix))]
                        let _ = delivery;
                    }
                    Err(error) => {
                        let _ = request.terminal.update(cx, |terminal, cx| {
                            terminal.error = Some(format!("Desktop notification failed: {error}"));
                            cx.notify();
                        });
                    }
                }
            }
        }).detach();
    }
    // Floods retain their in-app history; OS delivery is best effort and bounded.
    let _ = cx.global::<Dispatcher>().sender.as_ref().unwrap().try_send(Request { terminal: cx.weak_entity(), revision });
}

#[cfg(target_os = "macos")]
type Delivery = mac_usernotifications::NotificationHandle;
#[cfg(all(unix, not(target_os = "macos")))]
type Delivery = notify_rust::NotificationHandle;
#[cfg(not(unix))]
type Delivery = ();

#[cfg(unix)]
fn observe_response(delivery: Delivery, request: Request, cx: &mut App) {
    let done = Arc::new(AtomicBool::new(false));
    let completed = done.clone();
    let executor = cx.background_executor().clone();
    let task = cx.spawn(async move |cx| {
        if wait_for_click(delivery, executor).await {
            let _ = cx.update(|cx| window_view::focus_native_notification(&request.terminal, request.revision, cx));
        }
        completed.store(true, Ordering::Release);
    });
    cx.global_mut::<Dispatcher>().responses.push((done, task));
}

#[cfg(target_os = "macos")]
async fn wait_for_click(delivery: Delivery, _: BackgroundExecutor) -> bool {
    delivery.response().await.is_ok_and(|response| response.is_default_action())
}

#[cfg(all(unix, not(target_os = "macos")))]
async fn wait_for_click(delivery: Delivery, executor: BackgroundExecutor) -> bool {
    let response = async {
        let clicked = std::cell::Cell::new(false);
        delivery.wait_for_action_async(|response| clicked.set(response.is_default_action())).await;
        Some(clicked.get())
    };
    let timeout = async {
        executor.timer(std::time::Duration::from_secs(120)).await;
        None
    };
    let result = futures_lite::future::or(response, timeout).await;
    if result.is_none() {
        // Desktop servers may ignore expiry hints. Bound CloseNotification too,
        // so an unavailable D-Bus service cannot retain this listener forever.
        futures_lite::future::or(delivery.close_async(), async {
            executor.timer(std::time::Duration::from_secs(2)).await;
        }).await;
    }
    result.unwrap_or(false)
}

fn deliver(title: &str, body: &str) -> Result<Delivery> {
    #[cfg(target_os = "macos")]
    {
        notify_rust::check_bundle()?;
        anyhow::ensure!(notify_rust::request_auth_blocking()?, "notification permission is disabled in System Settings");
    }
    let title = bounded_text(title, 256);
    let body = bounded_text(body, 4096);
    #[cfg(target_os = "macos")]
    return Ok(mac_usernotifications::Notification::new()
        .title(if title.is_empty() { "rmux" } else { &title })
        .message(&body)
        .timeout(std::time::Duration::from_secs(120))
        .send_blocking()?);
    #[cfg(not(target_os = "macos"))]
    {

        // XDG notification bodies permit markup; terminal output is always plain text.
        #[cfg(all(unix, not(target_os = "macos")))]
        let body = body.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
        let mut notification = notify_rust::Notification::new();
        #[cfg(target_os = "windows")]
        notification.app_id("dev.rmux.prototype");
        notification.appname("rmux")
            .summary(if title.is_empty() { "rmux" } else { &title })
            .body(&body);
        #[cfg(unix)]
        {
            notification.action("default", "Open Terminal").timeout(120_000);
            Ok(notification.show()?)
        }
        #[cfg(not(unix))]
        {
            notification.show()?;
            Ok(())
        }
    }
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
    fn focus_policy_distinguishes_terminal_and_application_focus() {
        use crate::preferences::NotificationFocus::{BackgroundApp, UnfocusedTerminal};
        for app_active in [false, true] {
            assert!(!UnfocusedTerminal.allows(true, app_active));
            assert!(!BackgroundApp.allows(true, app_active));
            assert!(UnfocusedTerminal.allows(false, app_active));
            assert_eq!(BackgroundApp.allows(false, app_active), !app_active);
        }
        assert_eq!(serde_json::to_string(&BackgroundApp).unwrap(), "\"background_app\"");
        assert_eq!(BackgroundApp.toggle(), UnfocusedTerminal);
    }
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
