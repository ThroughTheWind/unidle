//! Headless check for the power assertion (WI-001, SPEC acceptance criteria 2 and 6).
//!
//! ```text
//! cargo run -p unidle-core --example hold -- [seconds] [--display-may-sleep]
//! ```
//!
//! Holds the assertion for `seconds` (default 60), then releases it. While it
//! runs, inspect it with `powercfg /requests` (Windows), `pmset -g assertions`
//! (macOS) or `systemd-inhibit --list` (Linux), and kill the process to check
//! that the OS drops it.

use std::time::Duration;

use unidle_core::{Controller, PowerOptions, Source, SystemAdapter};

fn main() {
    let mut seconds = 60;
    let mut options = PowerOptions::default();
    for arg in std::env::args().skip(1) {
        if arg == "--display-may-sleep" {
            options.keep_display_on = false;
        } else if let Ok(value) = arg.parse() {
            seconds = value;
        } else {
            eprintln!("usage: hold [seconds] [--display-may-sleep]");
            std::process::exit(2);
        }
    }

    let mut controller = Controller::new(SystemAdapter::new(options));
    if let Err(error) = controller.turn_on(Source::Manual) {
        eprintln!("could not acquire the power assertion: {error}");
        std::process::exit(1);
    }
    println!(
        "ON (pid {}, keep display on: {}). Holding for {seconds} s.",
        std::process::id(),
        options.keep_display_on
    );

    std::thread::sleep(Duration::from_secs(seconds));

    match controller.turn_off() {
        Ok(()) => println!("OFF. The assertion is released."),
        Err(error) => eprintln!("release reported an error: {error}"),
    }
}
