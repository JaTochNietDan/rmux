//! Finder file URLs. Reads metadata only and never modifies the pasteboard.
#![allow(unexpected_cfgs)]
use anyhow::{Result, ensure};
use objc::{class, msg_send, sel, sel_impl, runtime::{Object, BOOL, YES}};
use std::path::PathBuf;

// All objects are borrowed within an autorelease pool. NSString supplies the
// exact UTF-8 byte length; no unbounded C-string scan is needed.
unsafe fn text(value: *mut Object) -> Result<String> {
    ensure!(!value.is_null(), "Clipboard file URL is missing text");
    unsafe {
        let length: usize = msg_send![value, lengthOfBytesUsingEncoding:4usize];
        ensure!(length <= 64 * 1024, "Clipboard file URL exceeds 64 KiB");
        if length == 0 { return Ok(String::new()); }
        let bytes: *const u8 = msg_send![value, UTF8String];
        ensure!(!bytes.is_null(), "Clipboard file URL is not UTF-8");
        Ok(std::str::from_utf8(std::slice::from_raw_parts(bytes, length))?.to_owned())
    }
}
unsafe fn path_from_url(value: *mut Object) -> Result<PathBuf> {
    unsafe {
        // Validate the encoded size before NSURL creates a parsed representation.
        let _ = text(value)?;
        let url: *mut Object = msg_send![class!(NSURL), URLWithString:value];
        ensure!(!url.is_null(), "Invalid clipboard file URL");
        let file: BOOL = msg_send![url, isFileURL];
        ensure!(file == YES, "Clipboard URL is not a local file");
        let host: *mut Object = msg_send![url, host];
        if !host.is_null() {
            let host = text(host)?;
            ensure!(host.is_empty() || host.eq_ignore_ascii_case("localhost"), "Remote clipboard file URLs are not supported");
        }
        let query: *mut Object = msg_send![url, query];
        let fragment: *mut Object = msg_send![url, fragment];
        ensure!(query.is_null() && fragment.is_null(), "Clipboard file URL has a query or fragment");
        let path: *mut Object = msg_send![url, path];
        let path = PathBuf::from(text(path)?);
        ensure!(path.is_absolute(), "Clipboard file path must be absolute");
        Ok(path)
    }
}

pub fn read_paths() -> Result<Option<Vec<PathBuf>>> {
    objc::rc::autoreleasepool(|| unsafe {
        let main: BOOL = msg_send![class!(NSThread), isMainThread];
        ensure!(main == YES, "File clipboard reads require the UI thread");
        let board: *mut Object = msg_send![class!(NSPasteboard), generalPasteboard];
        ensure!(!board.is_null(), "Clipboard unavailable");
        let before: isize = msg_send![board, changeCount];
        let kind: *mut Object = msg_send![class!(NSString), stringWithUTF8String:c"public.file-url".as_ptr()];
        let types: *mut Object = msg_send![board, types];
        if types.is_null() { return Ok(None); }
        let contains: BOOL = msg_send![types, containsObject:kind];
        if contains != YES { return Ok(None); }
        let items: *mut Object = msg_send![board, pasteboardItems];
        ensure!(!items.is_null(), "Clipboard file items unavailable");
        let count: usize = msg_send![items, count];
        ensure!(count > 0 && count <= 256, "Paste between 1 and 256 files");
        let mut paths = Vec::new();
        let mut bytes = 0usize;
        for index in 0..count {
            let item: *mut Object = msg_send![items, objectAtIndex:index];
            let value: *mut Object = msg_send![item, stringForType:kind];
            if value.is_null() { continue; }
            let path = path_from_url(value)?;
            bytes += path.as_os_str().len();
            ensure!(bytes <= 64 * 1024, "Clipboard paths exceed 64 KiB");
            paths.push(path);
        }
        let after: isize = msg_send![board, changeCount];
        ensure!(before == after, "Clipboard changed while reading files; paste again");
        ensure!(!paths.is_empty(), "Clipboard file URLs could not be read");
        Ok(Some(paths))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn file_urls_decode_without_opening_files_or_mutating_clipboard() -> Result<()> {
        objc::rc::autoreleasepool(|| unsafe {
            let convert = |value: &str| -> Result<PathBuf> {
                let value = std::ffi::CString::new(value)?;
                let value: *mut Object = msg_send![class!(NSString), stringWithUTF8String:value.as_ptr()];
                path_from_url(value)
            };
            for (url, path) in [("file:///tmp/a%20b", "/tmp/a b"), ("file://localhost/tmp/it%27s%23x", "/tmp/it's#x"), ("file:///tmp/%E4%B8%96%E7%95%8C", "/tmp/世界")] {
                assert_eq!(convert(url)?, PathBuf::from(path));
            }
            for url in ["https://example.com/a", "file://remote.example/a", "file:///tmp/a?x", "file:///tmp/a#x", "relative"] { assert!(convert(url).is_err(), "{url}"); }
            Ok(())
        })
    }
}
