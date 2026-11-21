//! Opt-in native browser window. Split-tree ownership remains a separate gate.
use super::*;
use crate::text_field::TextField;
use std::{cell::RefCell, rc::Rc};

fn address(text: &str) -> anyhow::Result<String> {
    let text = text.trim();
    anyhow::ensure!(!text.is_empty() && text.len() <= 4096 && !text.chars().any(char::is_control), "Enter an HTTP or HTTPS address");
    let value = if text.contains("://") || text.starts_with("about:") { text.to_owned() }
        else { format!("https://{text}") };
    let url = url::Url::parse(&value)?;
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
struct Host { view: Option<wry::WebView>, bounds: Option<Bounds<Pixels>> }
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
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| TextField::new("about:blank", cx));
        cx.subscribe_in(&input, window, |this, _, event: &text_field::EditEvent, window, cx| {
            if let text_field::EditEvent::Submitted { text, .. } = event {
                match address(text).and_then(|url| this.navigate(&url, window)) {
                    Ok(()) => this.status = "Navigation requested".into(),
                    Err(error) => this.status = error.to_string(),
                }
                cx.notify();
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
        Self { host: Rc::default(), input, status: "Enter an address to create an ephemeral browser.".into(), committed: String::new(), latest, wake, _updates: task }
    }
    fn navigate(&mut self, url: &str, window: &Window) -> anyhow::Result<()> {
        let mut host = self.host.borrow_mut();
        if let Some(view) = &host.view { view.load_url(url)?; return Ok(()); }
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
    fn navigation(&mut self, operation: u8) {
        let host = self.host.borrow();
        if let Some(view) = &host.view {
            let result = match operation { 0 => view.go_back(), 1 => view.go_forward(), _ => view.reload() };
            if let Err(error) = result { self.status = error.to_string(); }
        }
    }
}
impl Render for BrowserWindow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let host = self.host.clone();
        let button = |id, label| div().id(id).px_3().py_2().rounded_md().bg(rgb(0x354969)).cursor_pointer().child(label);
        div().size_full().flex().flex_col()
            .on_action(cx.listener(|_, _: &CloseWindow, window, _| window.remove_window()))
            .bg(rgb(0x202733)).text_color(rgb(0xe8edf5)).text_size(px(14.))
            .child(div().h(px(52.)).flex_shrink_0().px_3().flex().items_center().gap_2()
                .child(button("back", "Back").on_click(cx.listener(|this, _, _, cx| { this.navigation(0); cx.notify(); })))
                .child(button("forward", "Forward").on_click(cx.listener(|this, _, _, cx| { this.navigation(1); cx.notify(); })))
                .child(button("reload", "Reload").on_click(cx.listener(|this, _, _, cx| { this.navigation(2); cx.notify(); })))
                .child(div().flex_1().min_w_0().child(self.input.clone()))
                .child(button("go", "Go").on_click(cx.listener(|this, _, window, cx| {
                    if this.input.read(cx).is_composing() { return; }
                    let result = address(this.input.read(cx).value()).and_then(|url| this.navigate(&url, window));
                    if let Err(error) = result { this.status = error.to_string(); }
                    cx.notify();
                }))))
            .child(div().h(px(28.)).flex_shrink_0().px_3().text_xs().truncate().child(format!("{} · {}", self.status, self.committed)))
            .child(div().flex_1().min_h_0().child(canvas(move |bounds, _, _| {
                let mut host = host.borrow_mut();
                if host.bounds != Some(bounds) {
                    let result = host.view.as_ref().map(|view| view.set_bounds(rect(bounds))).transpose();
                    match result { Ok(_) => host.bounds = Some(bounds), Err(error) => eprintln!("browser bounds: {error}") }
                }
            }, |_, _, _, _| {}).size_full()))
    }
}
pub fn open(cx: &mut App) -> anyhow::Result<()> {
    anyhow::ensure!(!cx.global::<Shutdowns>().quitting, "application is quitting");
    cx.open_window(WindowOptions { window_bounds: Some(WindowBounds::centered(size(px(1000.), px(680.)), cx)), ..Default::default() }, |window, cx| {
        window.set_window_title("rmux · Browser (Experimental)");
        cx.new(|cx| BrowserWindow::new(window, cx))
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{address, allowed};
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
