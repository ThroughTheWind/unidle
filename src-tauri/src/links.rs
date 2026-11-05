//! `unidle://` links inside the running app (SPEC §13). macOS hands a link
//! to the app it belongs to, running or just started; Windows and Linux
//! start the binary with the link instead, and `cli::run` answers it there.

use tauri::AppHandle;
use tauri_plugin_deep_link::DeepLinkExt as _;

use crate::cli;

pub fn listen(app: &AppHandle) {
    let handle = app.clone();
    app.deep_link().on_open_url(move |event| {
        for url in event.urls() {
            open(&handle, url.as_str());
        }
    });
    // The link that started the app, on macOS.
    if let Ok(Some(urls)) = app.deep_link().get_current() {
        for url in urls {
            open(app, url.as_str());
        }
    }
    // An AppImage has no installer to register the scheme; the `.deb`,
    // MSI, NSIS and `.app` bundles do it from tauri.conf.json.
    #[cfg(target_os = "linux")]
    if std::env::var_os("APPIMAGE").is_some()
        && let Err(error) = app.deep_link().register_all()
    {
        eprintln!("links: {error}");
    }
}

fn open(app: &AppHandle, link: &str) {
    match cli::link_request(link) {
        Ok(request) => {
            if let Some(error) = cli::handle(app, request).error {
                eprintln!("links: {error}");
            }
        }
        Err(error) => eprintln!("links: {error}"),
    }
}
