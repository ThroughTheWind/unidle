//! Linux: D-Bus idle inhibitors, and X11 input for the nudge.
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

// ── Input ────────────────────────────────────────────────────────────────

mod input {
    use std::time::Duration;

    use x11rb::connection::{Connection as _, RequestConnection as _};
    use x11rb::protocol::screensaver::ConnectionExt as _;
    use x11rb::protocol::xproto::{
        ConnectionExt as _, KEY_PRESS_EVENT, KEY_RELEASE_EVENT, Keycode, MOTION_NOTIFY_EVENT,
        Window,
    };
    use x11rb::protocol::xtest::{self, ConnectionExt as _};
    use x11rb::rust_connection::RustConnection;

    use crate::adapter::{AdapterError, InputUnavailable};

    const XK_F15: u32 = 0xFFCC;
    const XK_SHIFT_L: u32 = 0xFFE1;

    struct X11 {
        conn: RustConnection,
        root: Window,
        key: Keycode,
        has_idle_timer: bool,
    }

    /// Input through the XTEST extension, idle time through the
    /// MIT-SCREEN-SAVER extension. A Wayland session gets neither: there is
    /// no portable way to inject input there, so the app falls back to the
    /// power assertion alone (SPEC PB-2). XWayland is not used, because input
    /// injected into it does not reach the compositor's idle timer.
    pub struct Input {
        x11: Result<X11, InputUnavailable>,
    }

    impl Input {
        pub fn new() -> Self {
            Self { x11: connect() }
        }

        pub fn availability(&self) -> Result<(), InputUnavailable> {
            self.x11.as_ref().map(drop).map_err(|reason| *reason)
        }

        /// An absolute move to 1 px right and back, then a warp to the start
        /// if anything moved it in between.
        pub fn nudge_pointer(&mut self) -> Result<(), AdapterError> {
            let x11 = self.x11()?;
            let start = x11
                .conn
                .query_pointer(x11.root)
                .map_err(error)?
                .reply()
                .map_err(error)?;
            let (x, y) = (start.root_x, start.root_y);
            fake(x11, MOTION_NOTIFY_EVENT, 0, x.saturating_add(1), y)?;
            fake(x11, MOTION_NOTIFY_EVENT, 0, x, y)?;
            let end = x11
                .conn
                .query_pointer(x11.root)
                .map_err(error)?
                .reply()
                .map_err(error)?;
            if (end.root_x, end.root_y) != (x, y) {
                x11.conn
                    .warp_pointer(x11::NONE, x11.root, 0, 0, 0, 0, x, y)
                    .map_err(error)?;
                x11.conn.flush().map_err(error)?;
            }
            Ok(())
        }

        /// F15 down and up, or Shift if the keyboard map has no F15.
        pub fn nudge_key(&mut self) -> Result<(), AdapterError> {
            let x11 = self.x11()?;
            fake(x11, KEY_PRESS_EVENT, x11.key, 0, 0)?;
            fake(x11, KEY_RELEASE_EVENT, x11.key, 0, 0)
        }

        pub fn idle_time(&mut self) -> Option<Duration> {
            let x11 = self.x11.as_ref().ok()?;
            if !x11.has_idle_timer {
                return None;
            }
            let info = x11
                .conn
                .screensaver_query_info(x11.root)
                .ok()?
                .reply()
                .ok()?;
            Some(Duration::from_millis(u64::from(info.ms_since_user_input)))
        }

        fn x11(&self) -> Result<&X11, AdapterError> {
            self.x11
                .as_ref()
                .map_err(|reason| AdapterError(format!("Input nudge unavailable: {reason:?}")))
        }
    }

    mod x11 {
        pub const NONE: u32 = 0;
    }

    fn connect() -> Result<X11, InputUnavailable> {
        if is_wayland() {
            return Err(InputUnavailable::Wayland);
        }
        if std::env::var_os("DISPLAY").is_none_or(|display| display.is_empty()) {
            return Err(InputUnavailable::NoDisplay);
        }
        let (conn, screen) = x11rb::connect(None).map_err(|_| InputUnavailable::NoDisplay)?;
        let has = |name| conn.extension_information(name).ok().flatten().is_some();
        if !has(xtest::X11_EXTENSION_NAME) {
            return Err(InputUnavailable::Unsupported);
        }
        let has_idle_timer = has(x11rb::protocol::screensaver::X11_EXTENSION_NAME);
        let root = conn.setup().roots[screen].root;
        let key = keycode_for(&conn, XK_F15)
            .or_else(|| keycode_for(&conn, XK_SHIFT_L))
            .ok_or(InputUnavailable::Unsupported)?;
        Ok(X11 {
            conn,
            root,
            key,
            has_idle_timer,
        })
    }

    fn is_wayland() -> bool {
        std::env::var("XDG_SESSION_TYPE").is_ok_and(|kind| kind.eq_ignore_ascii_case("wayland"))
            || std::env::var_os("WAYLAND_DISPLAY").is_some_and(|display| !display.is_empty())
    }

    fn keycode_for(conn: &RustConnection, keysym: u32) -> Option<Keycode> {
        let setup = conn.setup();
        let (min, max) = (setup.min_keycode, setup.max_keycode);
        let mapping = conn
            .get_keyboard_mapping(min, max - min + 1)
            .ok()?
            .reply()
            .ok()?;
        let per = usize::from(mapping.keysyms_per_keycode).max(1);
        mapping
            .keysyms
            .chunks(per)
            .position(|syms| syms.contains(&keysym))
            .and_then(|index| u8::try_from(index).ok())
            .map(|index| min + index)
    }

    fn fake(x11: &X11, kind: u8, detail: u8, x: i16, y: i16) -> Result<(), AdapterError> {
        x11.conn
            .xtest_fake_input(kind, detail, 0, x11.root, x, y, 0)
            .map_err(error)?;
        x11.conn.flush().map_err(error)
    }

    fn error(error: impl std::fmt::Display) -> AdapterError {
        AdapterError(format!("X11: {error}"))
    }
}

pub use input::Input;
