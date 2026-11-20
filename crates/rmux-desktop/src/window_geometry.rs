use gpui::{App, Bounds, Pixels, Window, WindowBounds, point, px, size};
use serde::{Deserialize, Serialize};

/// Called synchronously from a mouse-down handler on the UI thread.
#[allow(unexpected_cfgs)] // objc 0.2's macros probe their legacy cargo-clippy cfg.
pub fn drag_titlebar(window: &Window) {
    #[cfg(target_os = "macos")]
    {
        use objc::{class, msg_send, sel, sel_impl};
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        let Ok(handle) = HasWindowHandle::window_handle(window) else { return; };
        let RawWindowHandle::AppKit(handle) = handle.as_raw() else { return; };
        // The borrowed view belongs to this live GPUI window. AppKit performs
        // its native drag with the current mouse event; no pointer is retained.
        unsafe {
            let view = handle.ns_view.as_ptr().cast::<objc::runtime::Object>();
            let native: *mut objc::runtime::Object = msg_send![view, window];
            let app: *mut objc::runtime::Object = msg_send![class!(NSApplication), sharedApplication];
            let event: *mut objc::runtime::Object = msg_send![app, currentEvent];
            if !native.is_null() && !event.is_null() {
                let _: () = msg_send![native, performWindowDragWithEvent: event];
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    window.start_window_move();
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Mode { #[default] Windowed, Maximized, Fullscreen }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Geometry {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    #[serde(default)]
    pub mode: Mode,
}
impl Geometry {
    pub fn valid(&self) -> bool {
        [self.x, self.y].into_iter().all(|value| value.is_finite() && value.abs() <= 100_000.)
            && [self.width, self.height].into_iter().all(|value| value.is_finite() && (1. ..=32768.).contains(&value))
    }
    pub fn capture(window: &Window) -> Self {
        let bounds = window.window_bounds();
        let mode = match bounds { WindowBounds::Windowed(_) => Mode::Windowed, WindowBounds::Maximized(_) => Mode::Maximized, WindowBounds::Fullscreen(_) => Mode::Fullscreen };
        let bounds = bounds.get_bounds();
        Self { x: bounds.origin.x.into(), y: bounds.origin.y.into(), width: bounds.size.width.into(), height: bounds.size.height.into(), mode }
    }
    fn fit(mut self, display: [f32; 4]) -> Self {
        self.width = self.width.min(display[2]);
        self.height = self.height.min(display[3]);
        self.x = self.x.clamp(display[0], display[0] + display[2] - self.width);
        self.y = self.y.clamp(display[1], display[1] + display[3] - self.height);
        self
    }
    pub fn restore(self, cx: &App) -> WindowBounds {
        let displays = cx.displays();
        // Prefer the display with the largest intersection. A disconnected
        // display falls back to the first available one, keeping the window reachable.
        let mut best = None;
        let mut area = -1.;
        for display in displays {
            let b = display.bounds();
            let d = [f32::from(b.origin.x), f32::from(b.origin.y), f32::from(b.size.width), f32::from(b.size.height)];
            if d[2] <= 0. || d[3] <= 0. { continue; }
            let overlap = (self.x + self.width).min(d[0] + d[2]) - self.x.max(d[0]);
            let vertical = (self.y + self.height).min(d[1] + d[3]) - self.y.max(d[1]);
            let candidate = overlap.max(0.) * vertical.max(0.);
            if candidate > area { area = candidate; best = Some(d); }
        }
        let fitted = best.map(|display| self.fit(display)).unwrap_or(self);
        let bounds: Bounds<Pixels> = Bounds::new(point(px(fitted.x), px(fitted.y)), size(px(fitted.width), px(fitted.height)));
        match self.mode { Mode::Windowed => WindowBounds::Windowed(bounds), Mode::Maximized => WindowBounds::Maximized(bounds), Mode::Fullscreen => WindowBounds::Fullscreen(bounds) }
    }
}

#[cfg(test)]
mod tests {
    use super::{Geometry, Mode};
    #[test]
    fn fitting_preserves_reachable_negative_coordinates_and_clamps_missing_display() {
        let geometry = Geometry { x: -1500., y: 20., width: 1000., height: 700., mode: Mode::Windowed };
        assert_eq!(geometry.fit([-1920., 0., 1920., 1080.]), geometry);
        let fitted = geometry.fit([0., 0., 800., 600.]);
        assert_eq!([fitted.x, fitted.y, fitted.width, fitted.height], [0., 0., 800., 600.]);
        assert!(!Geometry { width: f32::NAN, ..geometry }.valid());
        assert!(!Geometry { height: 0., ..geometry }.valid());
    }
}
