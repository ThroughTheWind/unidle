//! The app icon setting: the Wake eye (default) or the Halo power button.
//! The tray, the window (taskbar on Windows and Linux X11) and the macOS Dock
//! follow it while the app runs. The installers, the Start menu, Finder and
//! the Linux `.desktop` entry keep the bundled Wake eye, since those come
//! from files the OS reads before the app starts.

use tauri::image::Image;
use tauri::{AppHandle, Manager};

use crate::App;

/// 512 px renders of `icons/icon.svg` and `icons/power/icon.svg`.
const EYE: &[u8] = include_bytes!("../icons/icon.png");
const POWER: &[u8] = include_bytes!("../icons/power/icon.png");

/// Whether the power button icon is chosen. Anything else is the Wake eye.
pub fn is_power(app: &AppHandle) -> bool {
    app.state::<App>().settings().app_icon == "power"
}

pub fn image(app: &AppHandle) -> Option<Image<'static>> {
    Image::from_bytes(if is_power(app) { POWER } else { EYE }).ok()
}

/// Sets the open window's icon and the Dock icon. The tray follows on its
/// next refresh.
pub fn apply(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(crate::window::MAIN)
        && let Some(image) = image(app)
    {
        let _ = window.set_icon(image);
    }
    #[cfg(target_os = "macos")]
    set_dock_icon(app, if is_power(app) { POWER } else { EYE });
}

#[cfg(target_os = "macos")]
fn set_dock_icon(app: &AppHandle, png: &'static [u8]) {
    let _ = app.run_on_main_thread(move || {
        use objc2::{AllocAnyThread, MainThreadMarker};
        use objc2_app_kit::{NSApplication, NSImage};
        use objc2_foundation::NSData;

        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        let data = NSData::with_bytes(png);
        if let Some(image) = NSImage::initWithData(NSImage::alloc(), &data) {
            // SAFETY: on the main thread (checked above), with a valid image.
            unsafe { NSApplication::sharedApplication(mtm).setApplicationIconImage(Some(&image)) };
        }
    });
}
