//! The tray / menu bar icon and its menu (SPEC FR-2, FR-4, FR-5, FR-13).

use tauri::image::Image;
use tauri::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};
use unidle_core::Status;

use crate::strings;

const TRAY_ID: &str = "main";
const MENU_TOGGLE: &str = "toggle";
const MENU_OPEN: &str = "open";
const MENU_QUIT: &str = "quit";

/// Shape, not only color, tells ON from OFF (SPEC FR-4): OFF is an outlined
/// button, ON a filled one. Each app icon (app_icon.rs) has its own pair:
/// a closed and an open eye, or the power glyph. macOS gets monochrome
/// template images that the menu bar tints itself.
#[cfg(target_os = "macos")]
mod icons {
    pub const EYE: [&[u8]; 2] = [
        include_bytes!("../icons/tray/off-template.png"),
        include_bytes!("../icons/tray/on-template.png"),
    ];
    pub const POWER: [&[u8]; 2] = [
        include_bytes!("../icons/tray/power/off-template.png"),
        include_bytes!("../icons/tray/power/on-template.png"),
    ];
    pub const TEMPLATE: bool = true;
}
#[cfg(not(target_os = "macos"))]
mod icons {
    pub const EYE: [&[u8]; 2] = [
        include_bytes!("../icons/tray/off.png"),
        include_bytes!("../icons/tray/on.png"),
    ];
    pub const POWER: [&[u8]; 2] = [
        include_bytes!("../icons/tray/power/off.png"),
        include_bytes!("../icons/tray/power/on.png"),
    ];
    pub const TEMPLATE: bool = false;
}

fn icon(app: &AppHandle, on: bool) -> &'static [u8] {
    let set = if crate::app_icon::is_power(app) {
        icons::POWER
    } else {
        icons::EYE
    };
    set[usize::from(on)]
}

/// Menu items whose text follows the state.
pub struct TrayMenu {
    status: MenuItem<Wry>,
    toggle: MenuItem<Wry>,
}

pub fn create(app: &AppHandle) -> tauri::Result<TrayMenu> {
    let status = MenuItem::new(app, strings::STATUS_OFF, false, None::<&str>)?;
    let toggle = MenuItem::with_id(app, MENU_TOGGLE, strings::TURN_ON, true, None::<&str>)?;
    let open = MenuItem::with_id(app, MENU_OPEN, strings::OPEN_WINDOW, true, None::<&str>)?;
    let quit = MenuItem::with_id(app, MENU_QUIT, strings::QUIT, true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &status,
            &PredefinedMenuItem::separator(app)?,
            &toggle,
            &open,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(Image::from_bytes(icon(app, false))?)
        .icon_as_template(icons::TEMPLATE)
        .tooltip(strings::TOOLTIP_OFF)
        .menu(&menu)
        // Left-click toggles on Windows and Linux; macOS opens the menu, as
        // menu bar items do there (SPEC FR-2). Most Linux trays never report
        // clicks, so the menu is the way to toggle there.
        .show_menu_on_left_click(cfg!(target_os = "macos"))
        .on_menu_event(on_menu_event)
        .on_tray_icon_event(on_tray_event)
        .build(app)?;

    Ok(TrayMenu { status, toggle })
}

/// Brings the icon, tooltip and menu in line with the controller.
pub fn refresh(app: &AppHandle, status: &Status) {
    let on = status.is_on();
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let image = Image::from_bytes(icon(app, on)).ok();
        let _ = tray.set_icon_with_as_template(image, icons::TEMPLATE);
        let _ = tray.set_tooltip(Some(if on {
            strings::TOOLTIP_ON
        } else {
            strings::TOOLTIP_OFF
        }));
    }
    if let Some(menu) = app.try_state::<TrayMenu>() {
        let _ = menu.status.set_text(if on {
            strings::status_on(status.elapsed)
        } else {
            strings::STATUS_OFF.to_string()
        });
        let _ = menu.toggle.set_text(if on {
            strings::TURN_OFF
        } else {
            strings::TURN_ON
        });
    }
}

fn on_menu_event(app: &AppHandle, event: MenuEvent) {
    match event.id().as_ref() {
        MENU_TOGGLE => crate::toggle(app),
        MENU_OPEN => crate::window::show(app),
        MENU_QUIT => crate::quit(app),
        _ => {}
    }
}

fn on_tray_event(tray: &TrayIcon, event: TrayIconEvent) {
    if cfg!(target_os = "macos") {
        return;
    }
    if let TrayIconEvent::Click {
        button: MouseButton::Left,
        button_state: MouseButtonState::Up,
        ..
    } = event
    {
        crate::toggle(tray.app_handle());
    }
}
