//! `unidle on | off | status | timer` (SPEC §13): the same binary, driving
//! the tray app over a local channel. It never holds the power assertion
//! itself, so whatever it turns ON shows in the tray, and quitting the app
//! turns it OFF as usual.

use std::process::ExitCode;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager};
use unidle_core::remote::{self, Request, Response};

use crate::{App, StatusDto, TimerLength, strings};

const USAGE: &str = "\
Usage: unidle <command> [options]

Commands:
  on               Keep this computer awake until `unidle off`
  off              Let it sleep again
  status           Show whether it's ON, why, and what happens next
  timer <length>   Keep it awake for 30m, 1h, 1h30m (up to 24h), or until 18:00
  help             Show this help

Options:
  --nudge          With on and timer: also send the input nudges from Settings.
                   Without it, Unidle only asks the OS not to sleep (power only).
  --power-only     The default, said out loud
  --json           Print the status as JSON

Unidle starts in the tray if it isn't running, and shows there while it's ON.";

/// How long `unidle on` waits for an app it just started.
const START_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, PartialEq, Eq)]
enum Command {
    Send { request: Request, json: bool },
    Help,
    Version,
}

/// Runs the command line when the first argument is one of its commands, and
/// returns how to exit. `None` means start the app as usual, so `--hidden`
/// from the login item and anything the OS adds still reach it.
pub fn run() -> Option<ExitCode> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = parse(&args)?;
    remote::attach_parent_console();
    Some(match command {
        Ok(Command::Help) => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        Ok(Command::Version) => {
            println!("unidle {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Ok(Command::Send { request, json }) => match execute(&request) {
            Ok(response) => print(&request, &response, json),
            Err(error) => {
                eprintln!("unidle: {error}");
                ExitCode::FAILURE
            }
        },
        Err(error) => {
            eprintln!("unidle: {error}\n\n{USAGE}");
            ExitCode::from(2)
        }
    })
}

fn parse(args: &[String]) -> Option<Result<Command, String>> {
    let (first, rest) = args.split_first()?;
    let word = match first.as_str() {
        "help" | "--help" | "-h" => return Some(Ok(Command::Help)),
        "--version" | "-V" => return Some(Ok(Command::Version)),
        word @ ("on" | "off" | "status" | "timer") => word,
        // The app's own flags, and whatever the OS passes it.
        other if other.starts_with('-') => return None,
        other => return Some(Err(format!("unknown command `{other}`"))),
    };
    Some(parse_command(word, rest))
}

fn parse_command(word: &str, rest: &[String]) -> Result<Command, String> {
    let (mut nudge, mut power_only, mut json) = (false, false, false);
    let mut operands = Vec::new();
    for arg in rest {
        match arg.as_str() {
            "--nudge" => nudge = true,
            "--power-only" => power_only = true,
            "--json" => json = true,
            other if other.starts_with('-') => return Err(format!("unknown option `{other}`")),
            other => operands.push(other),
        }
    }
    if nudge && power_only {
        return Err("--nudge and --power-only don't go together".into());
    }
    let takes_nudge = matches!(word, "on" | "timer");
    if (nudge || power_only) && !takes_nudge {
        return Err(format!("`{word}` takes no --nudge or --power-only"));
    }
    let request = match (word, operands.as_slice()) {
        ("on", []) => Request::On { nudge },
        ("off", []) => Request::Off,
        ("status", []) => Request::Status,
        ("timer", [length]) => timer_request(length, nudge)?,
        ("timer", []) => return Err("`timer` needs a length, such as 30m or 18:00".into()),
        (word, _) => return Err(format!("`{word}` takes no arguments")),
    };
    Ok(Command::Send { request, json })
}

/// `30m`, `2h`, `1h30m`, `45` (minutes), or a time like `18:00`.
fn timer_request(length: &str, nudge: bool) -> Result<Request, String> {
    if length.contains(':') {
        return Ok(Request::Timer {
            minutes: None,
            until: Some(length.to_string()),
            nudge,
        });
    }
    let minutes = parse_minutes(length).ok_or_else(|| format!("not a length: `{length}`"))?;
    if !(1..=24 * 60).contains(&minutes) {
        return Err("a timer runs for 1 minute to 24 hours".into());
    }
    Ok(Request::Timer {
        minutes: Some(minutes),
        until: None,
        nudge,
    })
}

fn parse_minutes(text: &str) -> Option<u32> {
    if let Ok(minutes) = text.parse::<u32>() {
        return Some(minutes);
    }
    let (hours, rest) = match text.split_once('h') {
        Some((hours, rest)) => (hours.parse::<u32>().ok()?, rest),
        None => (0, text),
    };
    let minutes = match rest {
        "" => 0,
        rest => rest.strip_suffix('m')?.parse::<u32>().ok()?,
    };
    hours.checked_mul(60)?.checked_add(minutes)
}

/// Sends the request, starting the app first when ON is asked for and it
/// isn't running. OFF and status don't start it: not running is OFF.
fn execute(request: &Request) -> Result<Response, String> {
    let unreachable = |error: std::io::Error| format!("couldn't reach Unidle: {error}");
    if let Some(response) = remote::send(request).map_err(unreachable)? {
        return Ok(response);
    }
    if matches!(request, Request::Off | Request::Status) {
        return Ok(Response {
            text: Some(strings::CLI_NOT_RUNNING.into()),
            status: Some(serde_json::json!({ "running": false, "on": false })),
            ..Response::default()
        });
    }
    start_app()?;
    let deadline = Instant::now() + START_TIMEOUT;
    loop {
        if let Some(response) = remote::send(request).map_err(unreachable)? {
            return Ok(response);
        }
        if Instant::now() >= deadline {
            return Err(strings::CLI_START_FAILED.into());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// The app, in the tray. Its own process, so it outlives this terminal.
fn start_app() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|error| error.to_string())?;
    let mut command = std::process::Command::new(exe);
    command
        .arg(crate::HIDDEN_ARG)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        // Not in the terminal's job, so closing the terminal leaves it running.
        command.process_group(0);
    }
    command
        .spawn()
        .map(drop)
        .map_err(|error| format!("couldn't start Unidle: {error}"))
}

fn print(request: &Request, response: &Response, json: bool) -> ExitCode {
    if let Some(error) = &response.error {
        eprintln!("unidle: {error}");
        return ExitCode::FAILURE;
    }
    if json {
        let status = response.status.clone().unwrap_or_default();
        println!("{status}");
        return ExitCode::SUCCESS;
    }
    if let Some(text) = &response.text {
        println!("{text}");
    }
    let status = response.status.as_ref();
    let on = status
        .and_then(|status| status.get("on"))
        .and_then(serde_json::Value::as_bool)
        == Some(true);
    // Held by a condition is still ON as far as stopping it goes.
    let held = status
        .and_then(|status| status.get("held"))
        .is_some_and(|held| !held.is_null());
    let on = on || held;
    if on && matches!(request, Request::On { .. } | Request::Timer { .. }) {
        println!("{}", strings::CLI_HOW_TO_STOP);
    }
    ExitCode::SUCCESS
}

/// Answers a request inside the running app, the way the tray would.
pub fn handle(app: &AppHandle, request: Request) -> Response {
    let result = match request {
        Request::On { nudge } => crate::set_active_with(app, true, !nudge).map(drop),
        Request::Off => crate::set_active_with(app, false, false).map(drop),
        Request::Status => Ok(()),
        Request::Timer {
            minutes,
            until,
            nudge,
        } => timer_length(minutes, until)
            .and_then(|length| crate::start_timer_with(app, length, !nudge))
            .map(drop),
    };
    let status = app.state::<App>().controller().status();
    Response {
        status: serde_json::to_value(StatusDto::from(status)).ok(),
        text: Some(strings::cli_status(&status, &jiff::Zoned::now())),
        error: result.err(),
    }
}

fn timer_length(minutes: Option<u32>, until: Option<String>) -> Result<TimerLength, String> {
    match (minutes, until) {
        (Some(minutes), None) => Ok(TimerLength::Minutes(minutes)),
        (None, Some(until)) => unidle_core::parse_time(&until)
            .filter(|minutes| *minutes < unidle_core::MINUTES_PER_DAY)
            .map(TimerLength::Until)
            .ok_or_else(|| format!("Not a time: {until}")),
        _ => Err("Give minutes or a time".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(args: &[&str]) -> Option<Result<Command, String>> {
        let args: Vec<String> = args.iter().map(ToString::to_string).collect();
        parse(&args)
    }

    fn sends(args: &[&str]) -> Request {
        match parsed(args) {
            Some(Ok(Command::Send { request, .. })) => request,
            other => panic!("{args:?} gave {other:?}"),
        }
    }

    #[test]
    fn no_command_starts_the_app() {
        assert_eq!(parsed(&[]), None);
        assert_eq!(parsed(&["--hidden"]), None);
        assert_eq!(parsed(&["-psn_0_12345"]), None);
    }

    #[test]
    fn on_is_power_only_unless_asked() {
        assert_eq!(sends(&["on"]), Request::On { nudge: false });
        assert_eq!(sends(&["on", "--power-only"]), Request::On { nudge: false });
        assert_eq!(sends(&["on", "--nudge"]), Request::On { nudge: true });
        assert!(matches!(
            parsed(&["on", "--nudge", "--power-only"]),
            Some(Err(_))
        ));
        assert!(matches!(parsed(&["off", "--nudge"]), Some(Err(_))));
    }

    #[test]
    fn off_status_and_json() {
        assert_eq!(sends(&["off"]), Request::Off);
        assert_eq!(
            parsed(&["status", "--json"]),
            Some(Ok(Command::Send {
                request: Request::Status,
                json: true
            }))
        );
        assert!(matches!(parsed(&["status", "now"]), Some(Err(_))));
        assert!(matches!(parsed(&["stauts"]), Some(Err(_))));
        assert!(matches!(parsed(&["on", "--quiet"]), Some(Err(_))));
        assert_eq!(parsed(&["--version"]), Some(Ok(Command::Version)));
        assert_eq!(parsed(&["help"]), Some(Ok(Command::Help)));
    }

    #[test]
    fn timer_lengths() {
        let minutes = |text: &str| match sends(&["timer", text]) {
            Request::Timer { minutes, .. } => minutes,
            other => panic!("{other:?}"),
        };
        assert_eq!(minutes("30m"), Some(30));
        assert_eq!(minutes("45"), Some(45));
        assert_eq!(minutes("2h"), Some(120));
        assert_eq!(minutes("1h30m"), Some(90));
        assert_eq!(
            sends(&["timer", "18:00", "--nudge"]),
            Request::Timer {
                minutes: None,
                until: Some("18:00".into()),
                nudge: true
            }
        );
        assert!(matches!(parsed(&["timer"]), Some(Err(_))));
        assert!(matches!(parsed(&["timer", "soon"]), Some(Err(_))));
        assert!(matches!(parsed(&["timer", "1h30"]), Some(Err(_))));
        assert!(matches!(parsed(&["timer", "30m", "1h"]), Some(Err(_))));
        assert!(matches!(parsed(&["timer", "25h"]), Some(Err(_))));
        assert!(matches!(parsed(&["timer", "0"]), Some(Err(_))));
    }
}
