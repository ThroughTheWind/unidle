//! Text the Rust side shows: the tray, its menu, notifications and the macOS
//! app menu. The window's text is in `ui/src/strings.ts` (SPEC NFR-5).

use std::time::Duration;

#[cfg(target_os = "macos")]
pub const APP_NAME: &str = "Unidle";
pub const TOOLTIP_ON: &str = "Unidle: ON";
pub const TOOLTIP_OFF: &str = "Unidle: OFF";
pub const TURN_ON: &str = "Turn ON";
pub const TURN_OFF: &str = "Turn OFF";
pub const OPEN_WINDOW: &str = "Open window";
pub const QUIT: &str = "Quit";
#[cfg(target_os = "macos")]
pub const QUIT_APP: &str = "Quit Unidle";
pub const STATUS_OFF: &str = "Activity: OFF";
pub const STILL_RUNNING: &str = "Still running in the tray";
pub const STILL_RUNNING_BODY: &str =
    "Unidle keeps working with its window closed. Quit it from the tray menu.";

/// "Activity: ON for 1 h 12 min" (SPEC FR-5).
pub fn status_on(elapsed: Duration) -> String {
    let minutes = elapsed.as_secs() / 60;
    match (minutes / 60, minutes % 60) {
        (0, 0) => "Activity: ON for less than a minute".to_string(),
        (0, m) => format!("Activity: ON for {m} min"),
        (h, 0) => format!("Activity: ON for {h} h"),
        (h, m) => format!("Activity: ON for {h} h {m} min"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elapsed_reads_like_the_spec() {
        let at = |secs| status_on(Duration::from_secs(secs));
        assert_eq!(at(20), "Activity: ON for less than a minute");
        assert_eq!(at(12 * 60 + 59), "Activity: ON for 12 min");
        assert_eq!(at(3600), "Activity: ON for 1 h");
        assert_eq!(at(72 * 60), "Activity: ON for 1 h 12 min");
    }
}
