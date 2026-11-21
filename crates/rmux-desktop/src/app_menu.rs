//! Native application commands live outside terminal input routing.
use super::*;

#[derive(Clone, Debug, Default, PartialEq, Action)]
#[action(namespace = rmux, no_json)]
struct AboutRmux;
#[derive(Clone, Debug, Default, PartialEq, Action)]
#[action(namespace = rmux, no_json)]
struct HideRmux;
#[derive(Clone, Debug, Default, PartialEq, Action)]
#[action(namespace = rmux, no_json)]
struct HideOtherApps;
#[derive(Clone, Debug, Default, PartialEq, Action)]
#[action(namespace = rmux, no_json)]
struct ShowAllApps;

pub fn initialize(cx: &mut App) {
    if cfg!(target_os = "macos") {
        cx.bind_keys([
            KeyBinding::new("cmd-h", HideRmux, None),
            KeyBinding::new("cmd-alt-h", HideOtherApps, None),
        ]);
        cx.on_action(|_: &AboutRmux, _| about());
        cx.on_action(|_: &HideRmux, cx| cx.hide());
        cx.on_action(|_: &HideOtherApps, cx| cx.hide_other_apps());
        cx.on_action(|_: &ShowAllApps, cx| cx.unhide_other_apps());
    }
}

pub fn items(updater_available: bool) -> Vec<MenuItem> {
    let mut items = Vec::new();
    if cfg!(target_os = "macos") {
        items.push(MenuItem::action("About rmux", AboutRmux));
    }
    if updater_available {
        items.push(MenuItem::action("Check for Updates…", CheckForUpdates));
    }
    if !items.is_empty() { items.push(MenuItem::separator()); }
    items.push(MenuItem::action("Settings…", ToggleSettings));
    items.push(MenuItem::separator());
    if cfg!(target_os = "macos") {
        items.push(MenuItem::action("Hide rmux", HideRmux));
        items.push(MenuItem::action("Hide Others", HideOtherApps));
        items.push(MenuItem::action("Show All", ShowAllApps));
        items.push(MenuItem::separator());
    }
    items.push(MenuItem::action("Quit rmux", Quit));
    items
}

#[allow(unexpected_cfgs)]
fn about() {
    #[cfg(target_os = "macos")]
    {
        use objc::{class, msg_send, sel, sel_impl, runtime::Object};
        // GPUI dispatches application actions on the AppKit main thread. AppKit
        // owns this standard panel and reads product/version metadata from the bundle.
        unsafe {
            let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
            let _: () = msg_send![app, orderFrontStandardAboutPanel:std::ptr::null_mut::<Object>()];
        }
    }
}
