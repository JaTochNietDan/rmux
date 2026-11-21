//! Opt-in native browser window. Split-tree ownership remains a separate gate.
use super::*;
use crate::text_field::TextField;
use std::{cell::RefCell, rc::Rc};

actions!(browser, [FocusAddress, Back, Forward, Reload]);
#[derive(Clone, Copy)]
enum Navigation { Back, Forward, Reload }

fn address(text: &str) -> anyhow::Result<String> {
    let text = text.trim();
    anyhow::ensure!(!text.is_empty() && text.len() <= 4096 && !text.chars().any(char::is_control), "Enter an HTTP or HTTPS address");
    // A URL embedded in a query/path must not make a bare hostname look like
    // an explicitly schemed address. Preserve explicit HTTP/HTTPS choices.
    let explicit = text.find("://").is_some_and(|index| !text[..index].contains(['/', '?', '#']))
        || text.get(..6).is_some_and(|prefix| prefix.eq_ignore_ascii_case("about:"));
    let value = if explicit { text.to_owned() } else { format!("https://{text}") };
    let mut url = url::Url::parse(&value)?;
    if !explicit {
        let loopback = match url.host() {
            Some(url::Host::Domain(host)) => {
                let host = host.trim_end_matches('.');
                host == "localhost" || host.ends_with(".localhost")
            }
            Some(url::Host::Ipv4(host)) => host.is_loopback(),
            Some(url::Host::Ipv6(host)) => host.is_loopback(),
            None => false,
        };
        if loopback {
            // Reparse rather than set_scheme: HTTPS parsing can discard an
            // explicit :443, which must stay a port when choosing HTTP.
            url = url::Url::parse(&format!("http://{text}"))?;
        }
    }
    anyhow::ensure!(matches!(url.scheme(), "http" | "https") && url.host_str().is_some() || value == "about:blank", "Only HTTP, HTTPS and about:blank are supported");
    anyhow::ensure!(url.username().is_empty() && url.password().is_none(), "Use a URL without embedded credentials");
    Ok(url.into())
}
fn allowed(url: &str) -> bool {
    url == "about:blank" || url::Url::parse(url).is_ok_and(|url| matches!(url.scheme(), "http" | "https") && url.username().is_empty() && url.password().is_none())
}
fn rect(bounds: Bounds<Pixels>) -> wry::Rect {
    wry::Rect {
        position: wry::dpi::LogicalPosition::new(f64::from(f32::from(bounds.origin.x)), f64::from(f32::from(bounds.origin.y))).into(),
        size: wry::dpi::LogicalSize::new(f64::from(f32::from(bounds.size.width)), f64::from(f32::from(bounds.size.height))).into(),
    }
}
#[derive(Default)]
struct Host { view: Option<wry::WebView>, bounds: Option<Bounds<Pixels>>, pending_url: Option<String> }
struct BrowserWindow {
    host: Rc<RefCell<Host>>,
    input: Entity<TextField>,
    status: String,
    committed: String,
    latest: Arc<std::sync::Mutex<Option<(bool, String)>>>,
    wake: async_channel::Sender<()>,
    _updates: Task<()>,
}
impl BrowserWindow {
    fn new(initial: Option<String>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| TextField::new(initial.as_deref().unwrap_or("about:blank"), cx));
        cx.subscribe_in(&input, window, |this, _, event: &text_field::EditEvent, window, cx| {
            if let text_field::EditEvent::Submitted { text, .. } = event {
                match address(text).and_then(|url| this.navigate(&url, window)) {
                    Ok(()) => this.status = "Navigation requested".into(),
                    Err(error) => this.status = error.to_string(),
                }
                cx.notify();
            } else if matches!(event, text_field::EditEvent::Cancelled) {
                if !this.committed.is_empty() {
                    this.input.update(cx, |input, cx| input.replace_draft(&this.committed, cx));
                }
                if let Some(view) = &this.host.borrow().view { let _ = view.focus(); }
            }
        }).detach();
        input.read(cx).focus.focus(window);
        let latest = Arc::new(std::sync::Mutex::new(None));
        let (wake, receive) = async_channel::bounded(1);
        let updates = latest.clone();
        let task = cx.spawn(async move |entity, cx| {
            while receive.recv().await.is_ok() {
                let update = updates.lock().ok().and_then(|mut value| value.take());
                if let Some((finished, url)) = update {
                    if entity.update(cx, |this: &mut Self, cx| {
                        this.committed = url;
                        this.status = if finished { "Loaded" } else { "Loading…" }.into();
                        cx.notify();
                    }).is_err() { break; }
                }
            }
        });
        Self { host: Rc::new(RefCell::new(Host { pending_url: initial, ..Default::default() })), input, status: "Enter an address to create an ephemeral browser.".into(), committed: String::new(), latest, wake, _updates: task }
    }
    fn navigate(&mut self, url: &str, window: &Window) -> anyhow::Result<()> {
        let mut host = self.host.borrow_mut();
        if let Some(view) = &host.view { view.load_url(url)?; view.focus()?; return Ok(()); }
        let bounds = host.bounds.ok_or_else(|| anyhow::anyhow!("Browser layout is not ready"))?;
        let latest = self.latest.clone();
        let wake = self.wake.clone();
        host.view = Some(wry::WebViewBuilder::new().with_incognito(true).with_url(url)
            .with_bounds(rect(bounds)).with_navigation_handler(|url| allowed(&url))
            .with_new_window_req_handler(|_, _| wry::NewWindowResponse::Deny)
            .with_download_started_handler(|_, _| false)
            .with_on_page_load_handler(move |event, url| {
                // A single coalesced wakeup and bounded URL avoid unbounded page events.
                if url.len() <= 4096 {
                    if let Ok(mut value) = latest.lock() { *value = Some((matches!(event, wry::PageLoadEvent::Finished), url)); }
                    let _ = wake.try_send(());
                }
            }).build_as_child(window)?);
        Ok(())
    }
    fn focus_address(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.input.read(cx).is_composing() { return; }
        if let Some(view) = &self.host.borrow().view { let _ = view.focus_parent(); }
        self.input.update(cx, |input, cx| {
            if !self.committed.is_empty() { input.replace_draft(&self.committed, cx); }
            input.select_all_text(cx);
        });
        self.input.read(cx).focus.focus(window);
    }
    fn navigation(&mut self, operation: Navigation) {
        let host = self.host.borrow();
        if let Some(view) = &host.view {
            let result = match operation {
                Navigation::Back if view.can_go_back().unwrap_or(false) => view.go_back(),
                Navigation::Forward if view.can_go_forward().unwrap_or(false) => view.go_forward(),
                Navigation::Reload => view.reload(),
                _ => return,
            };
            if let Err(error) = result { self.status = error.to_string(); }
        }
    }
}
impl Render for BrowserWindow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let host = self.host.clone();
        let entity = cx.weak_entity();
        let (back, forward, loaded) = self.host.borrow().view.as_ref().map(|view|
            (view.can_go_back().unwrap_or(false), view.can_go_forward().unwrap_or(false), true)
        ).unwrap_or((false, false, false));
        let button = |id, label| div().id(id).px_3().py_2().rounded_md().bg(rgb(0x354969)).cursor_pointer().child(label);
        div().size_full().flex().flex_col().key_context("Browser")
            .on_action(cx.listener(|_, _: &CloseWindow, window, _| window.remove_window()))
            .bg(rgb(0x202733)).text_color(rgb(0xe8edf5)).text_size(px(14.))
            .child(div().h(px(52.)).flex_shrink_0().px_3().flex().items_center().gap_2()
                .child(button("back", "Back").when(!back, |button| button.opacity(0.4).cursor_default()).when(back, |button| button.on_click(cx.listener(|this, _, _, cx| { this.navigation(Navigation::Back); cx.notify(); }))))
                .child(button("forward", "Forward").when(!forward, |button| button.opacity(0.4).cursor_default()).when(forward, |button| button.on_click(cx.listener(|this, _, _, cx| { this.navigation(Navigation::Forward); cx.notify(); }))))
                .child(button("reload", "Reload").when(!loaded, |button| button.opacity(0.4).cursor_default()).when(loaded, |button| button.on_click(cx.listener(|this, _, _, cx| { this.navigation(Navigation::Reload); cx.notify(); }))))
                .child(div().flex_1().min_w_0().child(self.input.clone()))
                .child(button("go", "Go").on_click(cx.listener(|this, _, window, cx| {
                    if this.input.read(cx).is_composing() { return; }
                    let result = address(this.input.read(cx).value()).and_then(|url| this.navigate(&url, window));
                    if let Err(error) = result { this.status = error.to_string(); }
                    cx.notify();
                }))))
            .child(div().h(px(28.)).flex_shrink_0().px_3().text_xs().truncate().child(format!("{} · {}", self.status, self.committed)))
            .child(div().flex_1().min_h_0().child(canvas(move |bounds, window, cx| {
                let mut host = host.borrow_mut();
                if host.bounds != Some(bounds) {
                    let result = host.view.as_ref().map(|view| view.set_bounds(rect(bounds))).transpose();
                    match result { Ok(_) => host.bounds = Some(bounds), Err(error) => eprintln!("browser bounds: {error}") }
                }
                let pending = host.pending_url.take();
                drop(host);
                if let Some(url) = pending {
                    // Native child creation needs measured bounds. Defer the
                    // entity update until prepaint has released its host borrow.
                    window.defer(cx, move |window, cx| {
                        let _ = entity.update(cx, |this, cx| {
                            match this.navigate(&url, window) {
                                Ok(()) => this.status = "Navigation requested".into(),
                                Err(error) => this.status = error.to_string(),
                            }
                            cx.notify();
                        });
                    });
                }
            }, |_, _, _, _| {}).size_full()))
    }
}
fn with_active(cx: &mut App, update: impl FnOnce(&mut BrowserWindow, &mut Window, &mut Context<BrowserWindow>)) {
    if let Some(handle) = cx.active_window().and_then(|window| window.downcast::<BrowserWindow>()) {
        let _ = handle.update(cx, |this, window, cx| {
            if !this.input.read(cx).is_composing() { update(this, window, cx); }
        });
    }
}
pub fn initialize(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("cmd-l", FocusAddress, Some("Browser")),
        KeyBinding::new("cmd-r", Reload, Some("Browser")),
        KeyBinding::new("alt-left", Back, Some("Browser")),
        KeyBinding::new("alt-right", Forward, Some("Browser")),
    ]);
    cx.on_action(|_: &FocusAddress, cx| with_active(cx, |this, window, cx| this.focus_address(window, cx)));
    cx.on_action(|_: &Back, cx| with_active(cx, |this, _, cx| { this.navigation(Navigation::Back); cx.notify(); }));
    cx.on_action(|_: &Forward, cx| with_active(cx, |this, _, cx| { this.navigation(Navigation::Forward); cx.notify(); }));
    cx.on_action(|_: &Reload, cx| with_active(cx, |this, _, cx| { this.navigation(Navigation::Reload); cx.notify(); }));
}
pub fn menu() -> Menu {
    Menu { name: "Browser".into(), items: vec![
        MenuItem::action("Focus Address", FocusAddress),
        MenuItem::action("Back", Back),
        MenuItem::action("Forward", Forward),
        MenuItem::action("Reload", Reload),
    ] }
}

pub fn open(cx: &mut App) -> anyhow::Result<()> { open_initial(None, cx) }
pub fn can_open_link(uri: &str) -> bool {
    uri.len() <= 4096 && !uri.chars().any(char::is_control) && allowed(uri)
}
pub fn open_link(uri: &str, cx: &mut App) -> anyhow::Result<()> {
    anyhow::ensure!(can_open_link(uri), "This link cannot be opened in the rmux browser");
    open_initial(Some(address(uri)?), cx)
}
fn open_initial(initial: Option<String>, cx: &mut App) -> anyhow::Result<()> {
    anyhow::ensure!(!cx.global::<Shutdowns>().quitting, "application is quitting");
    cx.open_window(WindowOptions { window_bounds: Some(WindowBounds::centered(size(px(1000.), px(680.)), cx)), ..Default::default() }, move |window, cx| {
        window.set_window_title("rmux · Browser (Experimental)");
        cx.new(|cx| BrowserWindow::new(initial, window, cx))
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{address, allowed, can_open_link};
    #[test]
    fn terminal_browser_link_policy_is_bounded_and_explicit() {
        assert!(can_open_link("http://localhost:3000/path?q=hello"));
        assert!(!can_open_link("localhost:3000"));
        assert!(!can_open_link("https://user:secret@example.com"));
        assert!(!can_open_link("https://example.com/a\nb"));
        assert!(!can_open_link(&format!("https://example.com/{}", "a".repeat(4096))));
        assert!(!can_open_link("file:///tmp/test"));
    }
    #[test]
    fn browser_addresses_choose_http_only_for_implicit_loopback() {
        for (input, expected) in [
            ("localhost:3000", "http://localhost:3000/"),
            ("LOCALHOST.:443/path", "http://localhost.:443/path"),
            ("app.localhost:8080", "http://app.localhost:8080/"),
            ("127.0.0.1:8000/?next=https://example.com", "http://127.0.0.1:8000/?next=https://example.com"),
            ("127.1:443", "http://127.0.0.1:443/"),
            ("[::1]:3000", "http://[::1]:3000/"),
            ("https://localhost:3000", "https://localhost:3000/"),
            ("HTTPS://localhost:3000", "https://localhost:3000/"),
            ("example.com:3000", "https://example.com:3000/"),
            ("192.168.1.2:3000", "https://192.168.1.2:3000/"),
            ("localhost.example.com", "https://localhost.example.com/"),
            ("example.com/path?next=http://localhost", "https://example.com/path?next=http://localhost"),
        ] { assert_eq!(address(input).unwrap(), expected, "{input}"); }
        for input in ["localhost:65536", "[::1", "https://example.com/a\nb", "localhost:3000@evil.example"] {
            assert!(address(input).is_err(), "{input}");
        }
    }
    #[test]
    fn browser_addresses_normalize_and_restrict_navigation() {
        assert_eq!(address(" example.com ").unwrap(), "https://example.com/");
        assert_eq!(address("http://localhost:3000/path?q=a").unwrap(), "http://localhost:3000/path?q=a");
        assert_eq!(address("about:blank").unwrap(), "about:blank");
        for url in ["", "file:///tmp/private", "javascript:alert(1)", "data:text/html,test", "https://user:password@example.com"] {
            assert!(address(url).is_err(), "{url}");
        }
        assert!(!allowed("mailto:test@example.com"));
        assert!(!allowed("https://user@example.com"));
        assert!(allowed("https://example.com/path"));
    }
}
