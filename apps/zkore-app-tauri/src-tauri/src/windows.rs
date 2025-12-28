use tauri::{AppHandle, Manager as _, WebviewUrl, WebviewWindowBuilder};

pub const SIGNING_WINDOW_LABEL: &str = "signing";

pub fn open_signing_window(app: &AppHandle) -> Result<(), tauri::Error> {
    if let Some(window) = app.get_webview_window(SIGNING_WINDOW_LABEL) {
        window.show()?;
        window.set_focus()?;
        return Ok(());
    }

    let window = WebviewWindowBuilder::new(app, SIGNING_WINDOW_LABEL, WebviewUrl::App("index.html".into()))
        .title("Sign Transaction")
        .resizable(true)
        .fullscreen(true)
        .build()?;

    let _ = window.eval("window.location.hash = '#/signing'");
    Ok(())
}

pub fn close_signing_window(app: &AppHandle) -> Result<(), tauri::Error> {
    if let Some(window) = app.get_webview_window(SIGNING_WINDOW_LABEL) {
        window.close()?;
    }
    Ok(())
}
