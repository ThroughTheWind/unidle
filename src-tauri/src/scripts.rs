//! Run-on-event scripts (SPEC §17): a command the user wrote runs with
//! their shell when Unidle turns ON or OFF. Local, as the user, and never
//! waited on, so a slow script can't hold the toggle.

use std::process::{Command, Stdio};

use tauri::{AppHandle, Manager};
use unidle_core::{Change, ChangeSink, Source};

use crate::App;

/// Installs the commands from the settings, or nothing when both are unset.
pub fn apply(app: &AppHandle, on: Option<String>, off: Option<String>) {
    let sink: Option<ChangeSink> = if on.is_none() && off.is_none() {
        None
    } else {
        Some(Box::new(move |change| {
            let (command, event, source) = match change {
                Change::On(source) => (&on, "on", Some(source)),
                Change::Off => (&off, "off", None),
            };
            if let Some(command) = command {
                run(command, event, source);
            }
        }))
    };
    app.state::<App>().controller().set_change_sink(sink);
}

fn run(line: &str, event: &str, source: Option<Source>) {
    let mut command = shell(line);
    command
        .env("UNIDLE_EVENT", event)
        .env(
            "UNIDLE_SOURCE",
            source.map_or("", unidle_core::activity::source_name),
        )
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    match command.spawn() {
        // Reaped on its own thread, so it never lingers as a zombie.
        Ok(mut child) => {
            let line = line.to_string();
            std::thread::spawn(move || match child.wait() {
                Ok(status) if !status.success() => {
                    eprintln!("scripts: `{line}` exited with {status}");
                }
                Err(error) => eprintln!("scripts: `{line}`: {error}"),
                Ok(_) => {}
            });
        }
        Err(error) => eprintln!("scripts: couldn't run `{line}`: {error}"),
    }
}

#[cfg(windows)]
fn shell(line: &str) -> Command {
    use std::os::windows::process::CommandExt as _;
    /// No console window flashes up for the script.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let mut command = Command::new("cmd.exe");
    command
        .arg("/C")
        .raw_arg(line)
        .creation_flags(CREATE_NO_WINDOW);
    command
}

#[cfg(not(windows))]
fn shell(line: &str) -> Command {
    let mut command = Command::new("/bin/sh");
    command.arg("-c").arg(line);
    command
}

// Windows' `cmd /C` path is only compiled by CI, not run.
#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn the_shell_gets_the_line_and_the_event() {
        let dir = std::env::temp_dir().join(format!("unidle-scripts-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let out = dir.join("out");
        let line = format!(
            "echo \"$UNIDLE_EVENT $UNIDLE_SOURCE\" > '{}'",
            out.display()
        );
        let status = shell(&line)
            .env("UNIDLE_EVENT", "on")
            .env("UNIDLE_SOURCE", "timer")
            .status()
            .unwrap();
        assert!(status.success());
        assert_eq!(std::fs::read_to_string(&out).unwrap(), "on timer\n");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
