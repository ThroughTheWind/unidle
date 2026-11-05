//! Headless check for the input nudge (WI-002, SPEC acceptance criteria 3 to 5).
//!
//! ```text
//! cargo run -p unidle-core --example nudge -- once [pointer|key]
//! cargo run -p unidle-core --example nudge -- run [pointer|key|both] [interval-seconds] [minutes]
//! ```
//!
//! `once` sends one nudge and prints the idle time before and after, so you
//! can see the OS counted it. `run` turns ON as the app does, with the power
//! assertion and nudges on a schedule, and prints each nudge or skip. Move
//! the mouse or type during `run` to see nudges skipped.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use unidle_core::{
    Controller, EventKind, Input, NudgeConfig, NudgeKind, NudgeMethod, PlatformAdapter,
    SystemAdapter, Ticker, request_input_permission,
};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let arg = |index: usize| args.get(index).map(String::as_str);

    if !request_input_permission() {
        eprintln!("macOS: grant Accessibility to this terminal in System Settings, then rerun.");
    }
    let mut input = Input::new();
    if let Err(reason) = input.availability() {
        eprintln!(
            "Input nudge unavailable here: {reason:?}. The app would run on the power assertion alone."
        );
        std::process::exit(1);
    }

    match arg(0) {
        Some("once") => {
            let kind = match arg(1) {
                Some("key") => NudgeKind::Keystroke,
                _ => NudgeKind::Pointer,
            };
            std::thread::sleep(Duration::from_secs(2));
            println!("idle before: {:?}", input.idle_time());
            match input.nudge(kind) {
                Ok(()) => println!("sent {kind:?}"),
                Err(error) => println!("failed: {error}"),
            }
            println!("idle after:  {:?}", input.idle_time());
        }
        Some("run") => {
            let method = match arg(1) {
                Some("key") => NudgeMethod::Keystroke,
                Some("both") => NudgeMethod::Both,
                _ => NudgeMethod::Pointer,
            };
            let interval = arg(2).and_then(|s| s.parse().ok()).unwrap_or(10);
            let minutes = arg(3).and_then(|s| s.parse().ok()).unwrap_or(2);
            run(method, interval, minutes);
        }
        _ => {
            eprintln!(
                "usage: nudge once [pointer|key] | run [pointer|key|both] [interval-s] [minutes]"
            );
            std::process::exit(2);
        }
    }
}

fn run(method: NudgeMethod, interval: u64, minutes: u64) {
    let config = NudgeConfig {
        enabled: true,
        method,
        interval: Duration::from_secs(interval),
    };
    let controller = Arc::new(Mutex::new(Controller::with_nudge_config(
        SystemAdapter::default(),
        config,
    )));
    controller.lock().unwrap().turn_on().unwrap();
    let ticker = Ticker::spawn(controller.clone(), || {});
    ticker.wake();
    println!(
        "ON: {method:?} every {:?} ±20% for {minutes} min",
        config.clamped().interval
    );

    let start = Instant::now();
    let mut seen = 0;
    while start.elapsed() < Duration::from_secs(minutes * 60) {
        std::thread::sleep(Duration::from_millis(500));
        let controller = controller.lock().unwrap();
        for event in controller.events().skip(seen) {
            let at = event.at.saturating_duration_since(start).as_secs();
            match &event.kind {
                EventKind::Nudged(kind) => println!("{at:>4}s nudged {kind:?}"),
                EventKind::SkippedUserActive => println!("{at:>4}s skipped: you were active"),
                EventKind::NudgeFailed(error) => println!("{at:>4}s failed: {error}"),
                other => println!("{at:>4}s {other:?}"),
            }
        }
        seen = controller.events().count();
        let _ = controller.adapter().capabilities();
    }
    controller.lock().unwrap().turn_off().unwrap();
    println!("OFF");
}
