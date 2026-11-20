//! Sparkle's standard native UI, loaded only from a configured macOS app bundle.
//! All entry points run on GPUI's main thread; no updater work enters the render loop.
#[cfg(target_os = "macos")]
#[allow(unexpected_cfgs)]
mod macos {
    use objc::{class, msg_send, sel, sel_impl, runtime::{Class, Object, BOOL, YES, NO}};
    use std::{cell::RefCell, ffi::CString, ptr};
    struct Controller(*mut Object);
    impl Drop for Controller {
        fn drop(&mut self) { unsafe { let _: () = msg_send![self.0, release]; } }
    }
    thread_local! { static CONTROLLER: RefCell<Option<Controller>> = const { RefCell::new(None) }; }
    unsafe fn string(value: &str) -> *mut Object {
        let value = CString::new(value).expect("static updater string");
        unsafe { msg_send![class!(NSString), stringWithUTF8String:value.as_ptr()] }
    }
    pub fn initialize() -> bool {
        CONTROLLER.with(|slot| {
            if slot.borrow().is_some() { return true; }
            unsafe {
                let main: BOOL = msg_send![class!(NSThread), isMainThread];
                assert_eq!(main, YES);
                let bundle: *mut Object = msg_send![class!(NSBundle), mainBundle];
                let feed: *mut Object = msg_send![bundle, objectForInfoDictionaryKey:string("SUFeedURL")];
                if feed.is_null() { return false; } // Plain cargo runs/local bundles do not update.
                let frameworks: *mut Object = msg_send![bundle, privateFrameworksPath];
                let path: *mut Object = msg_send![frameworks, stringByAppendingPathComponent:string("Sparkle.framework")];
                let framework: *mut Object = msg_send![class!(NSBundle), bundleWithPath:path];
                let mut error: *mut Object = ptr::null_mut();
                let loaded: BOOL = msg_send![framework, loadAndReturnError:&mut error];
                if loaded != YES { eprintln!("rmux: could not load bundled Sparkle framework"); return false; }
                let Some(class) = Class::get("SPUStandardUpdaterController") else { return false; };
                let controller: *mut Object = msg_send![class, alloc];
                let controller: *mut Object = msg_send![controller, initWithStartingUpdater:NO updaterDelegate:ptr::null_mut::<Object>() userDriverDelegate:ptr::null_mut::<Object>()];
                if controller.is_null() { return false; }
                let controller = Controller(controller);
                let updater: *mut Object = msg_send![controller.0, updater];
                let started: BOOL = msg_send![updater, startUpdater:&mut error];
                if started != YES { eprintln!("rmux: Sparkle configuration was rejected"); return false; }
                *slot.borrow_mut() = Some(controller);
                true
            }
        })
    }
    pub fn check() {
        CONTROLLER.with(|slot| {
            if let Some(controller) = slot.borrow().as_ref() {
                unsafe {
                    let updater: *mut Object = msg_send![controller.0, updater];
                    let allowed: BOOL = msg_send![updater, canCheckForUpdates];
                    if allowed == YES { let _: () = msg_send![controller.0, checkForUpdates:ptr::null_mut::<Object>()]; }
                }
            }
        });
    }
}
#[cfg(target_os = "macos")]
pub use macos::{initialize, check};
#[cfg(not(target_os = "macos"))]
pub fn initialize() -> bool { false }
#[cfg(not(target_os = "macos"))]
pub fn check() {}
