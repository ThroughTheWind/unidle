//! Linux: D-Bus idle inhibitors.
//!
//! Desktops disagree on which inhibitor they honor, so this takes every one
//! that is available and succeeds if at least one was granted:
//!
//! - `org.freedesktop.ScreenSaver.Inhibit` (KDE, Xfce, most others) and
//!   `org.gnome.SessionManager.Inhibit` (GNOME) on the session bus;
//! - a systemd-logind `idle` inhibitor lock on the system bus.
//!
//! It never takes a logind `sleep` lock, so closing the lid or choosing
//! Suspend still works (SPEC PB-4). Every inhibitor here is tied to this
//! process: the session-bus cookies to its D-Bus connection and the logind
//! lock to a file descriptor, so both go away when the process dies
//! (SPEC PB-3). Inspect them with `systemd-inhibit --list` or
//! `gnome-session-inhibit --list`.

use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::OwnedFd;

use super::{PowerOptions, REASON};
use crate::adapter::AdapterError;

const APP_ID: &str = "com.throughthewind.unidle";
const APP_NAME: &str = "Unidle";

// org.gnome.SessionManager inhibit flags.
const GNOME_INHIBIT_SUSPEND: u32 = 4;
const GNOME_INHIBIT_IDLE: u32 = 8;

enum Inhibitor {
    /// `org.freedesktop.ScreenSaver` cookie, released with `UnInhibit`.
    ScreenSaver { path: &'static str, cookie: u32 },
    /// `org.gnome.SessionManager` cookie, released with `Uninhibit`.
    Gnome { cookie: u32 },
    /// logind lock, released by closing the descriptor.
    Logind { _fd: OwnedFd },
}

pub struct PowerAssertion {
    session: Option<Connection>,
    inhibitors: Vec<Inhibitor>,
}

impl PowerAssertion {
    pub fn acquire(options: PowerOptions) -> Result<Self, AdapterError> {
        let mut errors = Vec::new();
        let mut inhibitors = Vec::new();

        let session = Connection::session()
            .map_err(|error| errors.push(format!("session bus: {error}")))
            .ok();
        if let Some(session) = &session {
            if options.keep_display_on {
                match screensaver_inhibit(session) {
                    Ok(inhibitor) => inhibitors.push(inhibitor),
                    Err(error) => errors.push(error),
                }
            }
            // GNOME ignores org.freedesktop.ScreenSaver on some versions,
            // and its own idle flag also keeps the screen on, so only ask
            // GNOME to stop automatic suspend when the display may sleep.
            let flags = if options.keep_display_on {
                GNOME_INHIBIT_IDLE | GNOME_INHIBIT_SUSPEND
            } else {
                GNOME_INHIBIT_SUSPEND
            };
            match gnome_inhibit(session, flags) {
                Ok(inhibitor) => inhibitors.push(inhibitor),
                Err(error) => errors.push(error),
            }
        }

        match logind_inhibit() {
            Ok(inhibitor) => inhibitors.push(inhibitor),
            Err(error) => errors.push(error),
        }

        if inhibitors.is_empty() {
            return Err(AdapterError(format!(
                "No idle inhibitor is available: {}",
                errors.join("; ")
            )));
        }
        Ok(Self {
            session,
            inhibitors,
        })
    }
}

impl Drop for PowerAssertion {
    fn drop(&mut self) {
        // Releasing is best effort and must never panic: the inhibitors go
        // away with the connection and descriptor anyway, both dropped here.
        for inhibitor in self.inhibitors.drain(..) {
            let Some(session) = &self.session else {
                continue;
            };
            match inhibitor {
                Inhibitor::ScreenSaver { path, cookie } => {
                    if let Ok(proxy) = Proxy::new(
                        session,
                        "org.freedesktop.ScreenSaver",
                        path,
                        "org.freedesktop.ScreenSaver",
                    ) {
                        let _: zbus::Result<()> = proxy.call("UnInhibit", &(cookie,));
                    }
                }
                Inhibitor::Gnome { cookie } => {
                    if let Ok(proxy) = gnome_proxy(session) {
                        let _: zbus::Result<()> = proxy.call("Uninhibit", &(cookie,));
                    }
                }
                Inhibitor::Logind { .. } => {}
            }
        }
    }
}

fn screensaver_inhibit(session: &Connection) -> Result<Inhibitor, String> {
    // KDE historically exported the interface at /ScreenSaver only.
    let mut errors = Vec::new();
    for path in ["/org/freedesktop/ScreenSaver", "/ScreenSaver"] {
        let result = Proxy::new(
            session,
            "org.freedesktop.ScreenSaver",
            path,
            "org.freedesktop.ScreenSaver",
        )
        .and_then(|proxy| proxy.call::<_, _, u32>("Inhibit", &(APP_ID, REASON)));
        match result {
            Ok(cookie) => return Ok(Inhibitor::ScreenSaver { path, cookie }),
            Err(error) => errors.push(format!("ScreenSaver at {path}: {error}")),
        }
    }
    Err(errors.join("; "))
}

fn gnome_proxy(session: &Connection) -> zbus::Result<Proxy<'_>> {
    Proxy::new(
        session,
        "org.gnome.SessionManager",
        "/org/gnome/SessionManager",
        "org.gnome.SessionManager",
    )
}

fn gnome_inhibit(session: &Connection, flags: u32) -> Result<Inhibitor, String> {
    gnome_proxy(session)
        .and_then(|proxy| proxy.call::<_, _, u32>("Inhibit", &(APP_ID, 0u32, REASON, flags)))
        .map(|cookie| Inhibitor::Gnome { cookie })
        .map_err(|error| format!("GNOME SessionManager: {error}"))
}

fn logind_inhibit() -> Result<Inhibitor, String> {
    Connection::system()
        .and_then(|system| {
            let proxy = Proxy::new(
                &system,
                "org.freedesktop.login1",
                "/org/freedesktop/login1",
                "org.freedesktop.login1.Manager",
            )?;
            proxy.call::<_, _, OwnedFd>("Inhibit", &("idle", APP_NAME, REASON, "block"))
        })
        .map(|fd| Inhibitor::Logind { _fd: fd })
        .map_err(|error| format!("logind: {error}"))
}
