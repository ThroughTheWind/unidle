//! What the conditions read from the OS (SPEC §14): whether the screen is
//! locked, the power source, which apps run, and how long the computer has
//! slept. Read-only, local, and polled; nothing here changes the system or
//! reads what the user types or shows (NFR-2).

use std::time::Duration;

/// Where the power comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PowerSource {
    /// Plugged in, or a desktop with no battery.
    pub on_ac: bool,
    /// The charge of the battery, if there is one and the OS says.
    pub battery_percent: Option<u8>,
}

/// Reads the OS. Holds what is costly to open again on every read, such as
/// a D-Bus connection on Linux.
#[derive(Default)]
pub struct Environment {
    inner: sys::Environment,
}

impl Environment {
    /// `None` when the OS can't say.
    pub fn screen_locked(&mut self) -> Option<bool> {
        self.inner.screen_locked()
    }

    pub fn power_source(&mut self) -> Option<PowerSource> {
        self.inner.power_source()
    }

    /// The names of running processes, lower case, without `.exe`. `None`
    /// when they can't be listed.
    pub fn process_names(&mut self) -> Option<Vec<String>> {
        self.inner.process_names().map(|names| {
            names
                .into_iter()
                .map(|name| normalize_process_name(&name))
                .collect()
        })
    }

    /// The total time the computer has spent asleep since it started. The
    /// difference between two reads is how long it slept in between.
    pub fn time_asleep(&mut self) -> Option<Duration> {
        self.inner.time_asleep()
    }
}

/// "Zoom.exe" and "zoom" name the same app.
pub fn normalize_process_name(name: &str) -> String {
    let name = name.trim().to_lowercase();
    match name.strip_suffix(".exe") {
        Some(stem) => stem.to_string(),
        None => name,
    }
}

#[cfg(target_os = "linux")]
mod sys {
    use std::fs;
    use std::path::Path;
    use std::time::Duration;

    use zbus::blocking::{Connection, Proxy};

    use super::PowerSource;

    #[derive(Default)]
    pub struct Environment {
        system: Option<Connection>,
        session: Option<Connection>,
    }

    impl Environment {
        /// logind's `LockedHint` on this session, which GNOME, KDE and most
        /// lockers set, else whether the screensaver is active.
        pub fn screen_locked(&mut self) -> Option<bool> {
            if self.system.is_none() {
                self.system = Connection::system().ok();
            }
            if let Some(system) = &self.system {
                let locked = Proxy::new(
                    system,
                    "org.freedesktop.login1",
                    "/org/freedesktop/login1/session/auto",
                    "org.freedesktop.login1.Session",
                )
                .and_then(|proxy| proxy.get_property::<bool>("LockedHint"));
                if let Ok(locked) = locked {
                    return Some(locked);
                }
            }
            if self.session.is_none() {
                self.session = Connection::session().ok();
            }
            let session = self.session.as_ref()?;
            Proxy::new(
                session,
                "org.freedesktop.ScreenSaver",
                "/org/freedesktop/ScreenSaver",
                "org.freedesktop.ScreenSaver",
            )
            .and_then(|proxy| proxy.call::<_, _, bool>("GetActive", &()))
            .ok()
        }

        pub fn power_source(&mut self) -> Option<PowerSource> {
            read_power_supplies(Path::new("/sys/class/power_supply"))
        }

        /// `/proc/<pid>/comm`, and the file name of `argv[0]`, since `comm`
        /// is cut at 15 characters.
        pub fn process_names(&mut self) -> Option<Vec<String>> {
            let mut names = Vec::new();
            for entry in fs::read_dir("/proc").ok()?.flatten() {
                let file_name = entry.file_name();
                let Some(pid) = file_name.to_str() else {
                    continue;
                };
                if !pid.bytes().all(|b| b.is_ascii_digit()) {
                    continue;
                }
                let dir = entry.path();
                if let Ok(comm) = fs::read_to_string(dir.join("comm")) {
                    names.push(comm.trim_end().to_string());
                }
                if let Ok(cmdline) = fs::read(dir.join("cmdline")) {
                    let argv0 = cmdline.split(|b| *b == 0).next().unwrap_or_default();
                    if let Some(base) = Path::new(&*String::from_utf8_lossy(argv0)).file_name() {
                        names.push(base.to_string_lossy().into_owned());
                    }
                }
            }
            Some(names)
        }

        /// `CLOCK_BOOTTIME` counts suspend, `CLOCK_MONOTONIC` does not.
        pub fn time_asleep(&mut self) -> Option<Duration> {
            let boot = clock(libc::CLOCK_BOOTTIME)?;
            let awake = clock(libc::CLOCK_MONOTONIC)?;
            Some(boot.saturating_sub(awake))
        }
    }

    fn clock(id: libc::clockid_t) -> Option<Duration> {
        let mut time = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        // SAFETY: `time` is a valid timespec to write to.
        if unsafe { libc::clock_gettime(id, &mut time) } != 0 {
            return None;
        }
        Some(Duration::new(
            u64::try_from(time.tv_sec).ok()?,
            u32::try_from(time.tv_nsec).ok()?,
        ))
    }

    /// Mains, USB-C or another non-battery supply online means AC. No
    /// battery at all means a desktop, so AC as well.
    pub(super) fn read_power_supplies(root: &Path) -> Option<PowerSource> {
        let mut on_ac = false;
        let mut battery: Option<u8> = None;
        let mut saw_battery = false;
        for entry in fs::read_dir(root).ok()?.flatten() {
            let dir = entry.path();
            let read = |name: &str| {
                fs::read_to_string(dir.join(name))
                    .ok()
                    .map(|text| text.trim().to_string())
            };
            match read("type").as_deref() {
                Some("Battery") => {
                    // Peripherals (a mouse, a headset) report batteries too.
                    if read("scope").as_deref() == Some("Device") {
                        continue;
                    }
                    saw_battery = true;
                    if let Some(capacity) = read("capacity").and_then(|c| c.parse::<u8>().ok()) {
                        battery = Some(battery.map_or(capacity, |b| b.min(capacity)));
                    }
                }
                Some(_) => {
                    on_ac |= read("online").as_deref() == Some("1");
                }
                None => {}
            }
        }
        if !saw_battery {
            return Some(PowerSource {
                on_ac: true,
                battery_percent: None,
            });
        }
        Some(PowerSource {
            on_ac,
            battery_percent: battery,
        })
    }
}

#[cfg(windows)]
mod sys {
    use std::time::Duration;

    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
        TH32CS_SNAPPROCESS,
    };
    use windows_sys::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
    use windows_sys::Win32::System::StationsAndDesktops::{
        CloseDesktop, DESKTOP_READOBJECTS, GetUserObjectInformationW, OpenInputDesktop, UOI_NAME,
    };
    use windows_sys::Win32::System::WindowsProgramming::{
        QueryInterruptTime, QueryUnbiasedInterruptTime,
    };

    use super::PowerSource;

    #[derive(Default)]
    pub struct Environment;

    impl Environment {
        /// The input desktop is "Default" while the user can type into it.
        /// Locked, it is the secure "Winlogon" desktop, which a normal
        /// process can't open.
        pub fn screen_locked(&mut self) -> Option<bool> {
            // SAFETY: plain call; the handle is closed below.
            let desktop = unsafe { OpenInputDesktop(0, 0, DESKTOP_READOBJECTS) };
            if desktop.is_null() {
                return Some(true);
            }
            let mut name = [0u16; 64];
            let mut needed = 0u32;
            // SAFETY: `name` is writable for its size in bytes.
            let ok = unsafe {
                GetUserObjectInformationW(
                    desktop,
                    UOI_NAME,
                    name.as_mut_ptr().cast(),
                    std::mem::size_of_val(&name) as u32,
                    &mut needed,
                )
            };
            // SAFETY: we opened it.
            unsafe { CloseDesktop(desktop) };
            if ok == 0 {
                return None;
            }
            let len = name.iter().position(|c| *c == 0).unwrap_or(name.len());
            let name = String::from_utf16_lossy(&name[..len]);
            Some(!name.eq_ignore_ascii_case("Default"))
        }

        pub fn power_source(&mut self) -> Option<PowerSource> {
            let mut status: SYSTEM_POWER_STATUS = unsafe { std::mem::zeroed() };
            // SAFETY: `status` is writable.
            if unsafe { GetSystemPowerStatus(&mut status) } == 0 {
                return None;
            }
            const NO_BATTERY: u8 = 128;
            const UNKNOWN: u8 = 255;
            let has_battery = status.BatteryFlag != UNKNOWN && status.BatteryFlag & NO_BATTERY == 0;
            Some(PowerSource {
                // Unknown counts as AC: never pause on a guess.
                on_ac: status.ACLineStatus != 0 || !has_battery,
                battery_percent: (has_battery && status.BatteryLifePercent <= 100)
                    .then_some(status.BatteryLifePercent),
            })
        }

        pub fn process_names(&mut self) -> Option<Vec<String>> {
            // SAFETY: plain call; the handle is closed below.
            let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
            if snapshot == INVALID_HANDLE_VALUE {
                return None;
            }
            let mut names = Vec::new();
            let mut entry: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
            entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
            // SAFETY: `entry` is sized as the API asks.
            let mut more = unsafe { Process32FirstW(snapshot, &mut entry) } != 0;
            while more {
                let file = &entry.szExeFile;
                let len = file.iter().position(|c| *c == 0).unwrap_or(file.len());
                names.push(String::from_utf16_lossy(&file[..len]));
                // SAFETY: as above.
                more = unsafe { Process32NextW(snapshot, &mut entry) } != 0;
            }
            // SAFETY: we own the snapshot.
            unsafe { CloseHandle(snapshot) };
            Some(names)
        }

        /// The interrupt time counts sleep; the unbiased one does not. Both
        /// in 100 ns units.
        pub fn time_asleep(&mut self) -> Option<Duration> {
            let mut unbiased = 0u64;
            // SAFETY: writes one u64.
            if unsafe { QueryUnbiasedInterruptTime(&mut unbiased) } == 0 {
                return None;
            }
            let mut total = 0u64;
            // SAFETY: writes one u64.
            unsafe { QueryInterruptTime(&mut total) };
            Some(Duration::from_nanos(total.saturating_sub(unbiased) * 100))
        }
    }
}

#[cfg(target_os = "macos")]
mod sys {
    use std::ffi::{c_char, c_int, c_void};
    use std::time::Duration;

    use super::super::macos::{CFRelease, CfString};
    use super::PowerSource;

    type CFTypeRef = *const c_void;

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFDictionaryGetValue(dict: CFTypeRef, key: CFTypeRef) -> CFTypeRef;
        fn CFBooleanGetValue(boolean: CFTypeRef) -> u8;
        fn CFStringCompare(a: CFTypeRef, b: CFTypeRef, options: u64) -> isize;
        fn CFArrayGetCount(array: CFTypeRef) -> isize;
        fn CFArrayGetValueAtIndex(array: CFTypeRef, index: isize) -> CFTypeRef;
        fn CFNumberGetValue(number: CFTypeRef, kind: isize, value: *mut c_void) -> u8;
        fn CFGetTypeID(cf: CFTypeRef) -> usize;
        fn CFBooleanGetTypeID() -> usize;
        fn CFNumberGetTypeID() -> usize;
    }

    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGSessionCopyCurrentDictionary() -> CFTypeRef;
    }

    #[link(name = "IOKit", kind = "framework")]
    unsafe extern "C" {
        fn IOPSCopyPowerSourcesInfo() -> CFTypeRef;
        fn IOPSCopyPowerSourcesList(blob: CFTypeRef) -> CFTypeRef;
        fn IOPSGetPowerSourceDescription(blob: CFTypeRef, source: CFTypeRef) -> CFTypeRef;
        fn IOPSGetProvidingPowerSourceType(blob: CFTypeRef) -> CFTypeRef;
    }

    unsafe extern "C" {
        fn proc_listallpids(buffer: *mut c_void, size: c_int) -> c_int;
        fn proc_name(pid: c_int, buffer: *mut c_void, size: u32) -> c_int;
        fn mach_absolute_time() -> u64;
        fn mach_continuous_time() -> u64;
        fn mach_timebase_info(info: *mut TimebaseInfo) -> c_int;
    }

    #[repr(C)]
    #[derive(Default)]
    struct TimebaseInfo {
        numer: u32,
        denom: u32,
    }

    const K_CF_NUMBER_SINT32: isize = 3;

    /// Releases a CF object it owns on drop.
    struct Owned(CFTypeRef);

    impl Drop for Owned {
        fn drop(&mut self) {
            if !self.0.is_null() {
                // SAFETY: we own one reference.
                unsafe { CFRelease(self.0) };
            }
        }
    }

    fn get(dict: CFTypeRef, key: &str) -> Option<CFTypeRef> {
        let key = CfString::new(key).ok()?;
        // SAFETY: `dict` is a live dictionary; the value is borrowed.
        let value = unsafe { CFDictionaryGetValue(dict, key.as_ptr()) };
        (!value.is_null()).then_some(value)
    }

    fn number(value: CFTypeRef) -> Option<i32> {
        // SAFETY: type checked before reading.
        if unsafe { CFGetTypeID(value) != CFNumberGetTypeID() } {
            return None;
        }
        let mut out = 0i32;
        // SAFETY: `out` holds an SInt32.
        let ok =
            unsafe { CFNumberGetValue(value, K_CF_NUMBER_SINT32, (&mut out as *mut i32).cast()) };
        (ok != 0).then_some(out)
    }

    #[derive(Default)]
    pub struct Environment;

    impl Environment {
        /// The login window's session dictionary says
        /// `CGSSessionScreenIsLocked` while the screen is locked.
        pub fn screen_locked(&mut self) -> Option<bool> {
            // SAFETY: returns an owned dictionary or null.
            let session = Owned(unsafe { CGSessionCopyCurrentDictionary() });
            if session.0.is_null() {
                return None;
            }
            let Some(value) = get(session.0, "CGSSessionScreenIsLocked") else {
                return Some(false);
            };
            // SAFETY: type checked before reading.
            if unsafe { CFGetTypeID(value) == CFBooleanGetTypeID() } {
                Some(unsafe { CFBooleanGetValue(value) } != 0)
            } else {
                Some(number(value).is_some_and(|n| n != 0))
            }
        }

        pub fn power_source(&mut self) -> Option<PowerSource> {
            // SAFETY: owned blob or null.
            let blob = Owned(unsafe { IOPSCopyPowerSourcesInfo() });
            if blob.0.is_null() {
                return None;
            }
            // SAFETY: borrowed string from the blob.
            let providing = unsafe { IOPSGetProvidingPowerSourceType(blob.0) };
            let ac = CfString::new("AC Power").ok()?;
            let on_ac =
                providing.is_null() || unsafe { CFStringCompare(providing, ac.as_ptr(), 0) } == 0;

            // SAFETY: owned array or null.
            let list = Owned(unsafe { IOPSCopyPowerSourcesList(blob.0) });
            let mut battery: Option<u8> = None;
            if !list.0.is_null() {
                // SAFETY: a live array.
                let count = unsafe { CFArrayGetCount(list.0) };
                for index in 0..count {
                    // SAFETY: in bounds; borrowed values.
                    let source = unsafe { CFArrayGetValueAtIndex(list.0, index) };
                    let description = unsafe { IOPSGetPowerSourceDescription(blob.0, source) };
                    if description.is_null() {
                        continue;
                    }
                    let current = get(description, "Current Capacity").and_then(number);
                    let max = get(description, "Max Capacity").and_then(number);
                    if let (Some(current), Some(max)) = (current, max)
                        && max > 0
                    {
                        let percent = (current.clamp(0, max) * 100 / max) as u8;
                        battery = Some(battery.map_or(percent, |b| b.min(percent)));
                    }
                }
            }
            Some(PowerSource {
                on_ac,
                battery_percent: battery,
            })
        }

        pub fn process_names(&mut self) -> Option<Vec<String>> {
            // SAFETY: a null buffer asks for the count.
            let count = unsafe { proc_listallpids(std::ptr::null_mut(), 0) };
            if count <= 0 {
                return None;
            }
            // Room for processes started in between.
            let mut pids = vec![0 as c_int; count as usize + 64];
            let size = (pids.len() * std::mem::size_of::<c_int>()) as c_int;
            // SAFETY: `pids` is writable for `size` bytes.
            let count = unsafe { proc_listallpids(pids.as_mut_ptr().cast(), size) };
            if count <= 0 {
                return None;
            }
            let mut names = Vec::new();
            let mut buffer = [0 as c_char; 256];
            for &pid in &pids[..(count as usize).min(pids.len())] {
                // SAFETY: `buffer` is writable for its size.
                let len =
                    unsafe { proc_name(pid, buffer.as_mut_ptr().cast(), buffer.len() as u32) };
                if len > 0 {
                    let bytes: Vec<u8> = buffer[..len as usize].iter().map(|c| *c as u8).collect();
                    names.push(String::from_utf8_lossy(&bytes).into_owned());
                }
            }
            Some(names)
        }

        /// `mach_continuous_time` counts sleep, `mach_absolute_time` does not.
        pub fn time_asleep(&mut self) -> Option<Duration> {
            let mut info = TimebaseInfo::default();
            // SAFETY: writes the struct.
            if unsafe { mach_timebase_info(&mut info) } != 0 || info.denom == 0 {
                return None;
            }
            // SAFETY: plain reads of the clocks.
            let (total, awake) = unsafe { (mach_continuous_time(), mach_absolute_time()) };
            let ticks = u128::from(total.saturating_sub(awake));
            let nanos = ticks * u128::from(info.numer) / u128::from(info.denom);
            Some(Duration::from_nanos(u64::try_from(nanos).ok()?))
        }
    }
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
mod sys {
    use std::time::Duration;

    use super::PowerSource;

    #[derive(Default)]
    pub struct Environment;

    impl Environment {
        pub fn screen_locked(&mut self) -> Option<bool> {
            None
        }
        pub fn power_source(&mut self) -> Option<PowerSource> {
            None
        }
        pub fn process_names(&mut self) -> Option<Vec<String>> {
            None
        }
        pub fn time_asleep(&mut self) -> Option<Duration> {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_names_match_without_case_or_exe() {
        assert_eq!(normalize_process_name("Zoom.exe"), "zoom");
        assert_eq!(normalize_process_name(" blender "), "blender");
        assert_eq!(normalize_process_name("zoom.us"), "zoom.us");
    }

    #[test]
    fn reads_this_machine_without_failing() {
        let mut environment = Environment::default();
        // Whatever the answer, reading never panics, and the clock math holds.
        let _ = environment.screen_locked();
        let _ = environment.power_source();
        if let Some(names) = environment.process_names() {
            assert!(!names.is_empty());
        }
        if let (Some(a), Some(b)) = (environment.time_asleep(), environment.time_asleep()) {
            assert!(b >= a);
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn sysfs_power_supplies() {
        use std::fs;
        let root = std::env::temp_dir().join(format!("unidle-psu-{}", std::process::id()));
        let write = |dir: &str, files: &[(&str, &str)]| {
            let dir = root.join(dir);
            fs::create_dir_all(&dir).unwrap();
            for (name, value) in files {
                fs::write(dir.join(name), format!("{value}\n")).unwrap();
            }
        };
        let read = || sys::read_power_supplies(&root).unwrap();

        // A desktop: no battery at all.
        fs::create_dir_all(&root).unwrap();
        assert_eq!(
            read(),
            PowerSource {
                on_ac: true,
                battery_percent: None
            }
        );

        // A laptop on battery, with a wireless mouse that has its own.
        write("AC", &[("type", "Mains"), ("online", "0")]);
        write("BAT0", &[("type", "Battery"), ("capacity", "42")]);
        write(
            "hidpp_battery_0",
            &[("type", "Battery"), ("scope", "Device"), ("capacity", "5")],
        );
        assert_eq!(
            read(),
            PowerSource {
                on_ac: false,
                battery_percent: Some(42)
            }
        );

        // Plugged in over USB-C.
        write("ucsi-source-psy", &[("type", "USB"), ("online", "1")]);
        assert_eq!(
            read(),
            PowerSource {
                on_ac: true,
                battery_percent: Some(42)
            }
        );
        let _ = fs::remove_dir_all(&root);
    }
}
