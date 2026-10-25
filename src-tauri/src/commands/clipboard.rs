//! Clipboard read for terminal paste.
//!
//! WebKitGTK refuses `navigator.clipboard.readText()`, so pasting reads the
//! system clipboard natively. Text only, bounded, and only on request; there
//! is no clipboard *write* command (writes use the WebView) and terminal
//! programs can never read the clipboard (OSC 52 queries are ignored).

/// Pasting more than this is refused rather than truncated.
pub const MAX_PASTE_BYTES: usize = 1024 * 1024;

fn bounded(text: Option<String>) -> Result<Option<String>, String> {
    match text {
        Some(t) if t.len() > MAX_PASTE_BYTES => {
            Err("clipboard text is larger than 1 MiB; paste refused".into())
        }
        other => Ok(other),
    }
}

#[cfg(target_os = "linux")]
fn read_text(app: &tauri::AppHandle) -> Result<Option<String>, String> {
    let (tx, rx) = std::sync::mpsc::channel();
    // GTK clipboard access must happen on the main (GTK) thread.
    app.run_on_main_thread(move || {
        let clipboard = gtk::Clipboard::get(&gtk::gdk::SELECTION_CLIPBOARD);
        let _ = tx.send(clipboard.wait_for_text().map(|t| t.to_string()));
    })
    .map_err(|e| e.to_string())?;
    rx.recv_timeout(std::time::Duration::from_secs(3))
        .map_err(|_| "clipboard did not respond".to_string())
}

#[cfg(windows)]
fn read_text(_app: &tauri::AppHandle) -> Result<Option<String>, String> {
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
    };
    use windows_sys::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};
    const CF_UNICODETEXT: u32 = 13;
    // SAFETY: standard clipboard protocol; the handle is only used between
    // OpenClipboard and CloseClipboard, the lock is released before closing,
    // and reads stay within GlobalSize.
    unsafe {
        if IsClipboardFormatAvailable(CF_UNICODETEXT) == 0 {
            return Ok(None);
        }
        let mut opened = false;
        for _ in 0..10 {
            if OpenClipboard(std::ptr::null_mut()) != 0 {
                opened = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        if !opened {
            return Err("clipboard is busy".into());
        }
        let handle = GetClipboardData(CF_UNICODETEXT);
        let mut result = Ok(None);
        if !handle.is_null() {
            let ptr = GlobalLock(handle as _) as *const u16;
            if !ptr.is_null() {
                let max = GlobalSize(handle as _) / 2;
                let len = (0..max).take_while(|&i| *ptr.add(i) != 0).count();
                let units = std::slice::from_raw_parts(ptr, len);
                result = Ok(Some(String::from_utf16_lossy(units)));
                GlobalUnlock(handle as _);
            }
        }
        CloseClipboard();
        result
    }
}

#[cfg(not(any(target_os = "linux", windows)))]
fn read_text(_app: &tauri::AppHandle) -> Result<Option<String>, String> {
    Ok(None)
}

#[tauri::command]
pub async fn clipboard_read_text(app: tauri::AppHandle) -> Result<Option<String>, String> {
    let text = tauri::async_runtime::spawn_blocking(move || read_text(&app))
        .await
        .map_err(|e| e.to_string())??;
    bounded(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oversized_paste_is_refused_not_truncated() {
        assert!(bounded(Some("x".repeat(MAX_PASTE_BYTES + 1))).is_err());
        assert_eq!(bounded(Some("ok".into())).unwrap().as_deref(), Some("ok"));
        assert_eq!(bounded(None).unwrap(), None);
    }
}
