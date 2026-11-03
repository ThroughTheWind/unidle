//! Checks the X11 nudge on a virtual display: the pointer ends exactly where
//! it started, no button or key is left pressed, and the X idle timer sees
//! the nudge (SPEC FR-7, FR-9, acceptance criterion 4). Skips when `Xvfb` is
//! not installed.
#![cfg(target_os = "linux")]

use std::process::{Child, Command, Stdio};
use std::time::Duration;

use unidle_core::{Input, NudgeKind};
use x11rb::connection::Connection as _;
use x11rb::protocol::xproto::ConnectionExt as _;

struct Xvfb(Child);

impl Drop for Xvfb {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

const DISPLAY: &str = ":197";

fn start_xvfb() -> Option<Xvfb> {
    let child = Command::new("Xvfb")
        .args([
            DISPLAY,
            "-screen",
            "0",
            "1280x800x24",
            "-nolisten",
            "tcp",
            "-noreset",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let server = Xvfb(child);
    for _ in 0..50 {
        if x11rb::connect(Some(DISPLAY)).is_ok() {
            return Some(server);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    None
}

// One test function: DISPLAY is a process-wide environment variable.
#[test]
fn nudges_leave_no_trace() {
    let Some(_server) = start_xvfb() else {
        eprintln!("Xvfb not installed; skipping");
        return;
    };
    // SAFETY: this test binary runs this one test, on one thread.
    unsafe {
        std::env::set_var("DISPLAY", DISPLAY);
        std::env::remove_var("WAYLAND_DISPLAY");
        std::env::remove_var("XDG_SESSION_TYPE");
    }
    let (conn, screen) = x11rb::connect(Some(DISPLAY)).unwrap();
    let root = conn.setup().roots[screen].root;
    let pointer = || conn.query_pointer(root).unwrap().reply().unwrap();

    let mut input = Input::new();
    assert_eq!(input.availability(), Ok(()));

    // Middle of the screen, and the right and bottom edges, where a +1 px
    // move is clamped and a naive "move back" would drift.
    for (x, y) in [(640, 400), (1279, 400), (0, 0), (1279, 799)] {
        conn.warp_pointer(x11rb::NONE, root, 0, 0, 0, 0, x, y)
            .unwrap();
        conn.flush().unwrap();
        assert_eq!((pointer().root_x, pointer().root_y), (x, y));

        input.nudge(NudgeKind::Pointer).unwrap();
        let after = pointer();
        assert_eq!(
            (after.root_x, after.root_y),
            (x, y),
            "pointer moved from {x},{y}"
        );
        // No button held: the nudge never clicks or drags.
        assert_eq!(u16::from(after.mask) & 0x1f00, 0, "a button is pressed");
    }

    input.nudge(NudgeKind::Keystroke).unwrap();
    let keys = conn.query_keymap().unwrap().reply().unwrap();
    assert!(
        keys.keys.iter().all(|&byte| byte == 0),
        "a key is still down"
    );
    // Neither nudge set a modifier or button state.
    assert_eq!(u16::from(pointer().mask), 0);

    std::thread::sleep(Duration::from_millis(1500));
    let before = input.idle_time().unwrap();
    assert!(before >= Duration::from_millis(1000), "{before:?}");
    input.nudge(NudgeKind::Pointer).unwrap();
    let after = input.idle_time().unwrap();
    assert!(
        after < Duration::from_millis(500),
        "idle timer not reset: {after:?}"
    );

    // A Wayland session reports itself and sends nothing (SPEC PB-2).
    unsafe { std::env::set_var("WAYLAND_DISPLAY", "wayland-0") };
    let wayland = Input::new();
    assert_eq!(
        wayland.availability(),
        Err(unidle_core::InputUnavailable::Wayland)
    );
}
