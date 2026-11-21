//! Experimental native-view composition. Not a shipped browser surface.
#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("The browser composition probe currently requires macOS.");
    std::process::exit(1);
}

#[cfg(target_os = "macos")]
mod native {
    use gpui::{prelude::*, *};
    use std::{cell::RefCell, rc::Rc};

    const PAGE: &str = r#"<!doctype html><meta charset="utf-8">
<style>body{font:16px system-ui;background:#18202c;color:#e8edf5;padding:24px}
input,button{font:inherit;padding:10px;margin:8px 0}input{width:80%}</style>
<h1>Native browser composition</h1><p>This page is local and does not load network assets.</p>
<label>Test keyboard focus, Unicode and IME<br><input placeholder="Type here: 世界"></label>
<p><button onclick="this.textContent='Clicked '+(++window.count)">Click target</button></p>
<script>window.count=0</script><p>Resize the window and change the sidebar width.
Release and recreate the webview to inspect helper-process lifetime.</p>"#;

    #[derive(Default)]
    struct Host {
        webview: Option<wry::WebView>,
        bounds: Option<Bounds<Pixels>>,
    }
    struct Probe {
        host: Rc<RefCell<Host>>,
        sidebar: f32,
        status: String,
    }
    fn rect(bounds: Bounds<Pixels>) -> wry::Rect {
        wry::Rect {
            position: wry::dpi::LogicalPosition::new(f64::from(f32::from(bounds.origin.x)), f64::from(f32::from(bounds.origin.y))).into(),
            size: wry::dpi::LogicalSize::new(f64::from(f32::from(bounds.size.width)), f64::from(f32::from(bounds.size.height))).into(),
        }
    }
    impl Probe {
        fn create(&mut self, window: &Window) {
            let mut host = self.host.borrow_mut();
            if host.webview.is_some() { return; }
            let Some(bounds) = host.bounds else { self.status = "Wait for the first layout, then create.".into(); return; };
            match wry::WebViewBuilder::new()
                .with_incognito(true)
                .with_html(PAGE)
                .with_navigation_handler(|url| url == "about:blank")
                .with_bounds(rect(bounds))
                .build_as_child(window)
            {
                Ok(view) => { host.webview = Some(view); self.status = "Webview alive · ephemeral profile".into(); }
                Err(error) => self.status = format!("Creation failed: {error}"),
            }
        }
    }
    impl Render for Probe {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let host = self.host.clone();
            let button = |id, label| div().id(id).px_3().py_2().rounded_md().bg(rgb(0x354969)).cursor_pointer().child(label);
            div().size_full().flex().bg(rgb(0x202733)).text_color(rgb(0xe8edf5)).text_size(px(14.))
                .child(div().w(px(self.sidebar)).h_full().flex_shrink_0().p_4().flex().flex_col().gap_3()
                    .child("rmux browser probe")
                    .child(button("create", "Create webview").on_click(cx.listener(|this, _, window, cx| { this.create(window); cx.notify(); })))
                    .child(button("release", "Release webview").on_click(cx.listener(|this, _, _, cx| {
                        this.host.borrow_mut().webview.take();
                        this.status = "Webview dropped; inspect OS helper reclamation.".into(); cx.notify();
                    })))
                    .child(button("width", "Change sidebar width").on_click(cx.listener(|this, _, _, cx| {
                        this.sidebar = if this.sidebar == 260. { 360. } else { 260. }; cx.notify();
                    })))
                    .child(self.status.clone()))
                .child(div().flex_1().h_full().min_w_0().child(canvas(move |bounds, _, _| {
                    let mut host = host.borrow_mut();
                    if host.bounds != Some(bounds) {
                        if let Some(view) = &host.webview {
                            if let Err(error) = view.set_bounds(rect(bounds)) { eprintln!("browser bounds: {error}"); }
                        }
                        host.bounds = Some(bounds);
                    }
                }, |_, _, _, _| {}).size_full()))
        }
    }
    pub fn run() {
        Application::new().run(|cx| {
            cx.on_window_closed(|cx| { if cx.windows().is_empty() { cx.quit(); } }).detach();
            cx.open_window(WindowOptions {
                window_bounds: Some(WindowBounds::centered(size(px(1000.), px(680.)), cx)),
                ..Default::default()
            }, |window, cx| {
                window.set_window_title("rmux · browser composition probe");
                cx.new(|_| Probe { host: Rc::default(), sidebar: 260., status: "No webview created yet.".into() })
            }).expect("open browser composition probe");
            cx.activate(true);
        });
    }
}
#[cfg(target_os = "macos")]
fn main() { native::run(); }
