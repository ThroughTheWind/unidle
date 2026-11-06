//! Conditions (SPEC §14, ROADMAP Phase 4): when simulation is wanted ON but
//! shouldn't run right now, the controller holds it OFF and says why, then
//! resumes on its own.
//!
//! A reading the OS can't give never holds anything: Unidle doesn't pause on
//! a guess.

use serde::{Deserialize, Serialize};

use crate::platform::{PowerSource, normalize_process_name};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Conditions {
    /// Run only while plugged in.
    pub only_on_ac: bool,
    /// Pause while on battery below this charge, in percent.
    pub min_battery: Option<u8>,
    /// Run only while one of these apps is running. Empty means always.
    pub while_running: Vec<String>,
    /// Let the computer sleep while the screen is locked. Nudges always stop
    /// while it is locked, this or not.
    pub release_when_locked: bool,
    /// After the computer sleeps (the user chose Sleep or closed the lid),
    /// turn OFF instead of carrying on when it wakes.
    pub off_after_sleep: bool,
}

/// A new install: only `off_after_sleep`.
impl Default for Conditions {
    fn default() -> Self {
        Self {
            only_on_ac: false,
            min_battery: None,
            while_running: Vec::new(),
            release_when_locked: false,
            off_after_sleep: true,
        }
    }
}

impl Conditions {
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether anything has to be read from the OS while ON or held.
    pub fn needs_watching(&self) -> bool {
        self.only_on_ac
            || self.min_battery.is_some()
            || !self.while_running.is_empty()
            || self.release_when_locked
    }

    /// Drops blank names, and battery levels outside 1 to 99.
    pub fn cleaned(mut self) -> Self {
        self.while_running = self
            .while_running
            .iter()
            .map(|name| name.trim().to_string())
            .filter(|name| !name.is_empty())
            .collect();
        self.while_running.dedup();
        self.min_battery = self
            .min_battery
            .filter(|percent| (1..=99).contains(percent));
        self
    }
}

/// Why simulation is wanted ON but held OFF.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hold {
    Locked,
    OnBattery,
    LowBattery,
    AppNotRunning,
}

impl Hold {
    pub const fn name(self) -> &'static str {
        match self {
            Hold::Locked => "locked",
            Hold::OnBattery => "onBattery",
            Hold::LowBattery => "lowBattery",
            Hold::AppNotRunning => "appNotRunning",
        }
    }
}

/// What the OS said, read only for the conditions that are set.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Reading {
    pub locked: Option<bool>,
    pub power: Option<PowerSource>,
    /// Normalized process names.
    pub processes: Option<Vec<String>>,
}

/// The first condition that holds simulation OFF, if any. Checked in a fixed
/// order so the reason shown doesn't flicker between two.
pub fn hold(conditions: &Conditions, reading: &Reading) -> Option<Hold> {
    if conditions.release_when_locked && reading.locked == Some(true) {
        return Some(Hold::Locked);
    }
    if let Some(power) = reading.power {
        if conditions.only_on_ac && !power.on_ac {
            return Some(Hold::OnBattery);
        }
        if let (Some(min), Some(percent), false) =
            (conditions.min_battery, power.battery_percent, power.on_ac)
            && percent < min
        {
            return Some(Hold::LowBattery);
        }
    }
    if !conditions.while_running.is_empty()
        && let Some(processes) = &reading.processes
    {
        let running = conditions
            .while_running
            .iter()
            .map(|name| normalize_process_name(name))
            .any(|wanted| processes.contains(&wanted));
        if !running {
            return Some(Hold::AppNotRunning);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn battery(on_ac: bool, percent: u8) -> Option<PowerSource> {
        Some(PowerSource {
            on_ac,
            battery_percent: Some(percent),
        })
    }

    #[test]
    fn nothing_set_holds_nothing() {
        let reading = Reading {
            locked: Some(true),
            power: battery(false, 3),
            processes: Some(vec![]),
        };
        assert_eq!(hold(&Conditions::new(), &reading), None);
    }

    #[test]
    fn power_conditions() {
        let only_ac = Conditions {
            only_on_ac: true,
            ..Conditions::new()
        };
        let reading = |power| Reading {
            power,
            ..Reading::default()
        };
        assert_eq!(
            hold(&only_ac, &reading(battery(false, 90))),
            Some(Hold::OnBattery)
        );
        assert_eq!(hold(&only_ac, &reading(battery(true, 90))), None);
        // Unknown power never holds.
        assert_eq!(hold(&only_ac, &reading(None)), None);

        let min = Conditions {
            min_battery: Some(20),
            ..Conditions::new()
        };
        assert_eq!(
            hold(&min, &reading(battery(false, 19))),
            Some(Hold::LowBattery)
        );
        assert_eq!(hold(&min, &reading(battery(false, 20))), None);
        // Charging below the limit is fine: it's plugged in.
        assert_eq!(hold(&min, &reading(battery(true, 5))), None);
    }

    #[test]
    fn app_condition_matches_any_name_loosely() {
        let conditions = Conditions {
            while_running: vec!["Zoom.exe".into(), "blender".into()],
            ..Conditions::new()
        };
        let reading = |names: &[&str]| Reading {
            processes: Some(names.iter().map(|n| normalize_process_name(n)).collect()),
            ..Reading::default()
        };
        assert_eq!(hold(&conditions, &reading(&["explorer", "zoom"])), None);
        assert_eq!(hold(&conditions, &reading(&["Blender"])), None);
        assert_eq!(
            hold(&conditions, &reading(&["explorer"])),
            Some(Hold::AppNotRunning)
        );
        assert_eq!(hold(&conditions, &Reading::default()), None);
    }

    #[test]
    fn locked_holds_only_when_asked() {
        let reading = Reading {
            locked: Some(true),
            ..Reading::default()
        };
        let release = Conditions {
            release_when_locked: true,
            ..Conditions::new()
        };
        assert_eq!(hold(&release, &reading), Some(Hold::Locked));
        assert_eq!(hold(&Conditions::new(), &reading), None);
    }

    #[test]
    fn cleaning_and_serde_defaults() {
        let conditions = Conditions {
            min_battery: Some(0),
            while_running: vec![" zoom ".into(), "".into(), "zoom".into()],
            ..Conditions::new()
        }
        .cleaned();
        assert_eq!(conditions.min_battery, None);
        assert_eq!(conditions.while_running, ["zoom"]);
        let parsed: Conditions = serde_json::from_str("{}").unwrap();
        assert_eq!(parsed, Conditions::new());
    }
}
