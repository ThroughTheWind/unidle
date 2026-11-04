//! The tray / menu bar icon and its menu (SPEC FR-2, FR-4, FR-5, FR-13), with
//! the quick timers and the source line (SPEC SC-3, SC-4).

use std::sync::Mutex;

use jiff::Zoned;
use tauri::image::Image;
use tauri::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};
use unidle_core::Status;

use crate::{App, strings};

const TRAY_ID: &str = "main";
const MENU_TOGGLE: &str = "toggle";
const MENU_OPEN: &str = "open";
const MENU_QUIT: &str = "quit";
/// Followed by the number of minutes.
const MENU_TIMER: &str = "timer-";
const MENU_TIMER_UNTIL: &str = "timer-until";

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
    menu: Menu<Wry>,
    status: MenuItem<Wry>,
    /// The source and next change, under the status. Only in the menu while
    /// there is something to say, so the menu never shows a blank line.
    detail: MenuItem<Wry>,
    detail_shown: Mutex<bool>,
    toggle: MenuItem<Wry>,
}

pub fn create(app: &AppHandle) -> tauri::Result<TrayMenu> {
    let status = MenuItem::new(app, strings::STATUS_OFF, false, None::<&str>)?;
    let detail = MenuItem::new(app, "", false, None::<&str>)?;
    let toggle = MenuItem::with_id(app, MENU_TOGGLE, strings::TURN_ON, true, None::<&str>)?;
    let timers = Submenu::new(app, strings::KEEP_AWAKE_FOR, true)?;
    for (minutes, label) in strings::TIMERS {
        let id = format!("{MENU_TIMER}{minutes}");
        timers.append(&MenuItem::with_id(app, id, label, true, None::<&str>)?)?;
    }
    timers.append(&PredefinedMenuItem::separator(app)?)?;
    timers.append(&MenuItem::with_id(
        app,
        MENU_TIMER_UNTIL,
        strings::TIMER_UNTIL,
        true,
        None::<&str>,
    )?)?;
    let open = MenuItem::with_id(app, MENU_OPEN, strings::OPEN_WINDOW, true, None::<&str>)?;
    let quit = MenuItem::with_id(app, MENU_QUIT, strings::QUIT, true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &status,
            &PredefinedMenuItem::separator(app)?,
            &toggle,
            &timers,
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
        // Left-click follows the setting (SPEC FR-2). Most Linux trays never
        // report clicks, so the menu is the way to toggle there.
        .show_menu_on_left_click(!click_toggles(app))
        .on_menu_event(on_menu_event)
        .on_tray_icon_event(on_tray_event)
        .build(app)?;

    Ok(TrayMenu {
        menu,
        status,
        detail,
        detail_shown: Mutex::new(false),
        toggle,
    })
}

fn click_toggles(app: &AppHandle) -> bool {
    app.state::<App>().settings().tray_click_toggles
}

/// Applies the tray click setting after it changes.
pub fn apply_click_setting(app: &AppHandle) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_show_menu_on_left_click(!click_toggles(app));
    }
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
        let detail = strings::detail(status, &Zoned::now());
        let mut shown = menu
            .detail_shown
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        match detail {
            Some(text) => {
                let _ = menu.detail.set_text(text);
                if !*shown && menu.menu.insert(&menu.detail, 1).is_ok() {
                    *shown = true;
                }
            }
            None => {
                if *shown && menu.menu.remove(&menu.detail).is_ok() {
                    *shown = false;
                }
            }
        }
    }
}

fn on_menu_event(app: &AppHandle, event: MenuEvent) {
    match event.id().as_ref() {
        MENU_TOGGLE => crate::toggle(app),
        MENU_OPEN => crate::window::show(app),
        MENU_QUIT => crate::quit(app),
        MENU_TIMER_UNTIL => crate::window::show_timer_picker(app),
        id => {
            if let Some(minutes) = id
                .strip_prefix(MENU_TIMER)
                .and_then(|minutes| minutes.parse().ok())
            {
                let _ = crate::start_timer(app, crate::TimerLength::Minutes(minutes));
            }
        }
    }
}

fn on_tray_event(tray: &TrayIcon, event: TrayIconEvent) {
    if !click_toggles(tray.app_handle()) {
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
