//! Checks the Linux power assertion against fake desktop services on a
//! private D-Bus daemon: which inhibitors it takes, and that it releases them.
//! Skips when `dbus-daemon` is not installed.
#![cfg(target_os = "linux")]

use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

use unidle_core::{PowerAssertion, PowerOptions};
use zbus::blocking::Connection;
use zbus::blocking::connection::Builder;
use zbus::interface;
use zbus::zvariant::OwnedFd;

#[derive(Default, Debug)]
struct Log {
    calls: Vec<String>,
}

type Shared = Arc<Mutex<Log>>;

struct ScreenSaver(Shared);

#[interface(name = "org.freedesktop.ScreenSaver")]
impl ScreenSaver {
    fn inhibit(&self, app: &str, _reason: &str) -> u32 {
        self.0.lock().unwrap().calls.push(format!("ss.inhibit {app}"));
        7
    }

    fn un_inhibit(&self, cookie: u32) {
        self.0.lock().unwrap().calls.push(format!("ss.uninhibit {cookie}"));
    }
}

struct Gnome(Shared);

#[interface(name = "org.gnome.SessionManager")]
impl Gnome {
    fn inhibit(&self, _app: &str, _xid: u32, _reason: &str, flags: u32) -> u32 {
        self.0.lock().unwrap().calls.push(format!("gnome.inhibit {flags}"));
        9
    }

    fn uninhibit(&self, cookie: u32) {
        self.0.lock().unwrap().calls.push(format!("gnome.uninhibit {cookie}"));
    }
}

struct Logind(Shared);

#[interface(name = "org.freedesktop.login1.Manager")]
impl Logind {
    fn inhibit(&self, what: &str, _who: &str, _why: &str, mode: &str) -> OwnedFd {
        self.0.lock().unwrap().calls.push(format!("logind.inhibit {what} {mode}"));
        let file = std::fs::File::open("/dev/null").unwrap();
        OwnedFd::from(std::os::fd::OwnedFd::from(file))
    }
}

struct Daemon(Child);

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn start_daemon() -> Option<(Daemon, String)> {
    let mut child = Command::new("dbus-daemon")
        .args(["--session", "--nofork", "--print-address"])
        .stdout(Stdio::piped())
        .spawn()
        .ok()?;
    let mut address = String::new();
    BufReader::new(child.stdout.take()?)
        .read_line(&mut address)
        .ok()?;
    Some((Daemon(child), address.trim().to_string()))
}

fn serve(address: &str, log: &Shared, services: &[&str]) -> Connection {
    let mut builder = Builder::address(address).unwrap();
    for service in services {
        builder = match *service {
            "screensaver" => builder
                .name("org.freedesktop.ScreenSaver")
                .unwrap()
                .serve_at("/org/freedesktop/ScreenSaver", ScreenSaver(log.clone()))
                .unwrap(),
            "gnome" => builder
                .name("org.gnome.SessionManager")
                .unwrap()
                .serve_at("/org/gnome/SessionManager", Gnome(log.clone()))
                .unwrap(),
            "logind" => builder
                .name("org.freedesktop.login1")
                .unwrap()
                .serve_at("/org/freedesktop/login1", Logind(log.clone()))
                .unwrap(),
            other => panic!("unknown service {other}"),
        };
    }
    builder.build().unwrap()
}

fn calls(log: &Shared) -> Vec<String> {
    std::mem::take(&mut log.lock().unwrap().calls)
}

// One test function: the bus addresses are process-wide environment variables.
#[test]
fn takes_every_available_inhibitor_and_releases_them() {
    let Some((_session_daemon, session)) = start_daemon() else {
        eprintln!("dbus-daemon not installed; skipping");
        return;
    };
    let (_system_daemon, system) = start_daemon().unwrap();
    // SAFETY: this test binary runs this one test, on one thread.
    unsafe {
        std::env::set_var("DBUS_SESSION_BUS_ADDRESS", &session);
        std::env::set_var("DBUS_SYSTEM_BUS_ADDRESS", &system);
    }

    let log = Shared::default();
    let _session_services = serve(&session, &log, &["screensaver", "gnome"]);
    let _system_services = serve(&system, &log, &["logind"]);

    // Display on: everything, and never a logind "sleep" lock (SPEC PB-4).
    let assertion = PowerAssertion::acquire(PowerOptions {
        keep_display_on: true,
    })
    .unwrap();
    let mut taken = calls(&log);
    taken.sort();
    assert_eq!(
        taken,
        [
            "gnome.inhibit 12",
            "logind.inhibit idle block",
            "ss.inhibit com.throughthewind.unidle",
        ]
    );
    drop(assertion);
    let mut released = calls(&log);
    released.sort();
    assert_eq!(released, ["gnome.uninhibit 9", "ss.uninhibit 7"]);

    // Display may sleep: no screensaver inhibit, GNOME suspend flag only.
    let assertion = PowerAssertion::acquire(PowerOptions {
        keep_display_on: false,
    })
    .unwrap();
    let mut taken = calls(&log);
    taken.sort();
    assert_eq!(taken, ["gnome.inhibit 4", "logind.inhibit idle block"]);
    drop(assertion);
    calls(&log);

    // No desktop services at all: logind alone is enough.
    drop(_session_services);
    PowerAssertion::acquire(PowerOptions::default()).unwrap();
    assert_eq!(calls(&log), ["logind.inhibit idle block"]);

    // Nothing at all: an error, not a silent "ON".
    drop(_system_services);
    let error = PowerAssertion::acquire(PowerOptions::default())
        .err()
        .expect("acquire must fail with no inhibitor");
    assert!(error.0.contains("No idle inhibitor"), "{error}");
}
