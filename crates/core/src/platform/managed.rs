//! Work computers (SPEC §16): whether this computer looks managed by an
//! organization, and the policy its IT team may have set for Unidle.
//!
//! Every check is local and read-only: a registry key, a file, or the OS's
//! own `profiles` tool. Nothing looks for monitoring, security or tracking
//! products by name, and nothing goes over the network (NFR-2).

use serde_json::Value;

/// Why the computer looks managed. Only ever a reason to warn: finding none
/// says nothing about whether a computer is a work computer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ManagedSignal {
    /// Joined to an Active Directory domain (Windows, macOS).
    Domain,
    /// Joined to Microsoft Entra ID (Windows).
    EntraId,
    /// Enrolled in device management (MDM).
    Mdm,
    /// Joined to a Kerberos realm or directory through SSSD (Linux).
    Realm,
}

impl ManagedSignal {
    pub const fn name(self) -> &'static str {
        match self {
            ManagedSignal::Domain => "domain",
            ManagedSignal::EntraId => "entraId",
            ManagedSignal::Mdm => "mdm",
            ManagedSignal::Realm => "realm",
        }
    }
}

/// What the organization allows, read once at launch.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Policy {
    /// No input nudges, whatever the settings say: the power assertion only.
    pub disable_input_simulation: bool,
}

/// The policy's one key, in each place it can be set: the registry value
/// name, the managed preference key, and the policy file's key.
#[cfg_attr(not(any(windows, target_os = "macos")), allow(dead_code))]
const DISABLE_INPUT: &str = "DisableInputSimulation";
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
const DISABLE_INPUT_JSON: &str = "disableInputSimulation";

/// The first signal found that an organization manages this computer.
pub fn managed_signal() -> Option<ManagedSignal> {
    sys::managed_signal()
}

/// The policy IT set for Unidle, or the default when there is none.
pub fn read_policy() -> Policy {
    sys::read_policy()
}

/// `/etc/unidle/policy.json` on Linux: `{"disableInputSimulation": true}`.
/// Anything unreadable is no policy, as a missing file is.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn policy_from_json(text: &str) -> Policy {
    let value: Value = serde_json::from_str(text).unwrap_or(Value::Null);
    Policy {
        disable_input_simulation: value
            .get(DISABLE_INPUT_JSON)
            .and_then(Value::as_bool)
            .unwrap_or(false),
    }
}

/// `profiles status -type enrollment` says "MDM enrollment: Yes" or
/// "Enrolled via DEP: Yes" on an enrolled Mac.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn enrolled_from_profiles(output: &str) -> bool {
    output.lines().any(|line| {
        let Some((key, value)) = line.split_once(':') else {
            return false;
        };
        let key = key.trim().to_ascii_lowercase();
        (key == "mdm enrollment" || key == "enrolled via dep")
            && value.trim().to_ascii_lowercase().starts_with("yes")
    })
}

#[cfg(target_os = "linux")]
mod sys {
    use std::path::Path;

    use super::{ManagedSignal, Policy, policy_from_json};

    pub fn managed_signal() -> Option<ManagedSignal> {
        realm_signal(Path::new("/"))
    }

    /// `realm join` and `adcli` write a Kerberos keytab and an SSSD
    /// configuration; neither exists on a computer nobody joined.
    pub(super) fn realm_signal(root: &Path) -> Option<ManagedSignal> {
        ["etc/sssd/sssd.conf", "etc/krb5.keytab"]
            .iter()
            .any(|path| root.join(path).exists())
            .then_some(ManagedSignal::Realm)
    }

    pub fn read_policy() -> Policy {
        std::fs::read_to_string("/etc/unidle/policy.json")
            .map(|text| policy_from_json(&text))
            .unwrap_or_default()
    }
}

#[cfg(windows)]
mod sys {
    use std::ffi::c_void;

    use windows_sys::Win32::Foundation::ERROR_SUCCESS;
    use windows_sys::Win32::NetworkManagement::NetManagement::{
        NETSETUP_JOIN_STATUS, NetApiBufferFree, NetGetJoinInformation, NetSetupDomainName,
    };
    use windows_sys::Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, RRF_RT_REG_DWORD, RRF_RT_REG_SZ,
        RegCloseKey, RegEnumKeyExW, RegGetValueW, RegOpenKeyExW,
    };

    use super::{DISABLE_INPUT, ManagedSignal, Policy};

    const POLICY_KEY: &str = r"SOFTWARE\Policies\Unidle";
    const ENTRA_JOIN_KEY: &str = r"SYSTEM\CurrentControlSet\Control\CloudDomainJoin\JoinInfo";
    const ENROLLMENTS_KEY: &str = r"SOFTWARE\Microsoft\Enrollments";

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(Some(0)).collect()
    }

    pub fn managed_signal() -> Option<ManagedSignal> {
        if domain_joined() {
            Some(ManagedSignal::Domain)
        } else if !subkeys(ENTRA_JOIN_KEY).is_empty() {
            Some(ManagedSignal::EntraId)
        } else if mdm_enrolled() {
            Some(ManagedSignal::Mdm)
        } else {
            None
        }
    }

    fn domain_joined() -> bool {
        let mut name = std::ptr::null_mut();
        let mut status: NETSETUP_JOIN_STATUS = 0;
        let result = unsafe { NetGetJoinInformation(std::ptr::null(), &mut name, &mut status) };
        if !name.is_null() {
            unsafe { NetApiBufferFree(name as *const c_void) };
        }
        result == 0 && status == NetSetupDomainName
    }

    /// An enrollment with a provider, such as Intune's "MS DM Server". The
    /// key also holds bookkeeping subkeys without one.
    fn mdm_enrolled() -> bool {
        subkeys(ENROLLMENTS_KEY).iter().any(|subkey| {
            let path = format!(r"{ENROLLMENTS_KEY}\{subkey}");
            string_value(HKEY_LOCAL_MACHINE, &path, "ProviderID")
                .is_some_and(|provider| !provider.is_empty())
        })
    }

    fn subkeys(path: &str) -> Vec<String> {
        let mut key: HKEY = std::ptr::null_mut();
        let path = wide(path);
        if unsafe { RegOpenKeyExW(HKEY_LOCAL_MACHINE, path.as_ptr(), 0, KEY_READ, &mut key) }
            != ERROR_SUCCESS
        {
            return Vec::new();
        }
        let mut names = Vec::new();
        for index in 0.. {
            let mut buffer = [0u16; 256];
            let mut length = buffer.len() as u32;
            let result = unsafe {
                RegEnumKeyExW(
                    key,
                    index,
                    buffer.as_mut_ptr(),
                    &mut length,
                    std::ptr::null(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                )
            };
            if result != ERROR_SUCCESS {
                break;
            }
            names.push(String::from_utf16_lossy(&buffer[..length as usize]));
        }
        unsafe { RegCloseKey(key) };
        names
    }

    fn string_value(root: HKEY, path: &str, name: &str) -> Option<String> {
        let (path, name) = (wide(path), wide(name));
        let mut buffer = [0u16; 256];
        let mut size = std::mem::size_of_val(&buffer) as u32;
        let result = unsafe {
            RegGetValueW(
                root,
                path.as_ptr(),
                name.as_ptr(),
                RRF_RT_REG_SZ,
                std::ptr::null_mut(),
                buffer.as_mut_ptr().cast(),
                &mut size,
            )
        };
        if result != ERROR_SUCCESS {
            return None;
        }
        let length = (size as usize / 2).saturating_sub(1);
        Some(String::from_utf16_lossy(
            &buffer[..length.min(buffer.len())],
        ))
    }

    fn dword_value(root: HKEY, path: &str, name: &str) -> Option<u32> {
        let (path, name) = (wide(path), wide(name));
        let mut value = 0u32;
        let mut size = std::mem::size_of::<u32>() as u32;
        let result = unsafe {
            RegGetValueW(
                root,
                path.as_ptr(),
                name.as_ptr(),
                RRF_RT_REG_DWORD,
                std::ptr::null_mut(),
                (&mut value as *mut u32).cast(),
                &mut size,
            )
        };
        (result == ERROR_SUCCESS).then_some(value)
    }

    /// Group Policy or Intune writes `HKLM` (or `HKCU`)
    /// `\SOFTWARE\Policies\Unidle\DisableInputSimulation` = 1.
    pub fn read_policy() -> Policy {
        let set = [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER]
            .into_iter()
            .any(|root| {
                dword_value(root, POLICY_KEY, DISABLE_INPUT).is_some_and(|value| value != 0)
            });
        Policy {
            disable_input_simulation: set,
        }
    }
}

#[cfg(target_os = "macos")]
mod sys {
    use std::ffi::c_void;
    use std::path::Path;
    use std::process::Command;

    use super::super::macos::{CFRelease, CfString};
    use super::{DISABLE_INPUT, ManagedSignal, Policy, enrolled_from_profiles};

    type CFTypeRef = *const c_void;

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFPreferencesCopyAppValue(key: CFTypeRef, app: CFTypeRef) -> CFTypeRef;
        fn CFPreferencesAppValueIsForced(key: CFTypeRef, app: CFTypeRef) -> u8;
        fn CFBooleanGetValue(boolean: CFTypeRef) -> u8;
        fn CFGetTypeID(cf: CFTypeRef) -> usize;
        fn CFBooleanGetTypeID() -> usize;
        fn CFNumberGetTypeID() -> usize;
        fn CFNumberGetValue(number: CFTypeRef, kind: isize, value: *mut c_void) -> u8;
    }

    const BUNDLE_ID: &str = "com.throughthewind.unidle";
    const K_CF_NUMBER_SINT64: isize = 4;

    pub fn managed_signal() -> Option<ManagedSignal> {
        if mdm_enrolled() {
            Some(ManagedSignal::Mdm)
        } else if ad_bound() {
            Some(ManagedSignal::Domain)
        } else {
            None
        }
    }

    /// The OS's own tool; it needs no administrator rights for this.
    fn mdm_enrolled() -> bool {
        Command::new("/usr/bin/profiles")
            .args(["status", "-type", "enrollment"])
            .output()
            .is_ok_and(|output| enrolled_from_profiles(&String::from_utf8_lossy(&output.stdout)))
    }

    /// Binding to Active Directory leaves a configuration here.
    fn ad_bound() -> bool {
        std::fs::read_dir(Path::new(
            "/Library/Preferences/OpenDirectory/Configurations/Active Directory",
        ))
        .is_ok_and(|mut entries| entries.next().is_some())
    }

    /// A managed preference (an MDM configuration profile for
    /// `com.throughthewind.unidle`). A value the user set themselves with
    /// `defaults write` is not a policy, so only forced values count.
    pub fn read_policy() -> Policy {
        let (Ok(key), Ok(app)) = (CfString::new(DISABLE_INPUT), CfString::new(BUNDLE_ID)) else {
            return Policy::default();
        };
        let (key, app) = (key.as_ptr() as CFTypeRef, app.as_ptr() as CFTypeRef);
        if unsafe { CFPreferencesAppValueIsForced(key, app) } == 0 {
            return Policy::default();
        }
        let value = unsafe { CFPreferencesCopyAppValue(key, app) };
        if value.is_null() {
            return Policy::default();
        }
        let set = unsafe {
            let kind = CFGetTypeID(value);
            if kind == CFBooleanGetTypeID() {
                CFBooleanGetValue(value) != 0
            } else if kind == CFNumberGetTypeID() {
                let mut number = 0i64;
                CFNumberGetValue(value, K_CF_NUMBER_SINT64, (&mut number as *mut i64).cast()) != 0
                    && number != 0
            } else {
                false
            }
        };
        unsafe { CFRelease(value) };
        Policy {
            disable_input_simulation: set,
        }
    }
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
mod sys {
    use super::{ManagedSignal, Policy};

    pub fn managed_signal() -> Option<ManagedSignal> {
        None
    }

    pub fn read_policy() -> Policy {
        Policy::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_policy_file_is_read_leniently() {
        assert!(policy_from_json(r#"{"disableInputSimulation": true}"#).disable_input_simulation);
        assert!(!policy_from_json(r#"{"disableInputSimulation": false}"#).disable_input_simulation);
        assert!(!policy_from_json(r#"{"disableInputSimulation": "yes"}"#).disable_input_simulation);
        assert_eq!(policy_from_json("not json"), Policy::default());
        assert_eq!(policy_from_json("{}"), Policy::default());
    }

    #[test]
    fn profiles_output_is_read_as_enrolled_only_on_yes() {
        assert!(enrolled_from_profiles(
            "Enrolled via DEP: No\nMDM enrollment: Yes (User Approved)\n"
        ));
        assert!(enrolled_from_profiles(
            "Enrolled via DEP: Yes\nMDM enrollment: No\n"
        ));
        assert!(!enrolled_from_profiles(
            "Enrolled via DEP: No\nMDM enrollment: No\n"
        ));
        assert!(!enrolled_from_profiles(""));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_realm_is_found_by_its_files() {
        let root = std::env::temp_dir().join(format!("unidle-realm-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("etc/sssd")).unwrap();
        assert_eq!(sys::realm_signal(&root), None);
        std::fs::write(root.join("etc/sssd/sssd.conf"), "").unwrap();
        assert_eq!(sys::realm_signal(&root), Some(ManagedSignal::Realm));
        let _ = std::fs::remove_dir_all(&root);
    }
}
