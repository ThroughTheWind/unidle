//! The network conditions (SPEC §14): which Wi-Fi network this computer is
//! on, and whether a VPN is connected. Read-only and local, like the rest of
//! the environment: no traffic is sent or inspected (NFR-2).

/// Reads the OS. Holds the D-Bus connection on Linux.
#[derive(Default)]
pub struct Network {
    inner: sys::Network,
}

impl Network {
    /// The names (SSIDs) of the Wi-Fi networks this computer is joined to:
    /// empty when it is on none. `None` when the OS doesn't say, which on
    /// recent Windows and macOS can mean Location access wasn't granted.
    pub fn wifi_networks(&mut self) -> Option<Vec<String>> {
        self.inner.wifi_networks()
    }

    /// Whether a VPN tunnel is up. `None` when the OS doesn't say.
    pub fn vpn_connected(&mut self) -> Option<bool> {
        self.inner.vpn_connected()
    }
}

/// Wi-Fi names compare without case or surrounding spaces, as people type
/// them.
pub fn normalize_network_name(name: &str) -> String {
    name.trim().to_lowercase()
}

/// Tunnel interfaces as VPN clients name them: OpenVPN and most others
/// (`tun`, `tap`), WireGuard (`wg`), PPP-based VPNs (`ppp`), IPsec
/// (`ipsec`), and macOS's `utun`.
#[cfg_attr(windows, allow(dead_code))]
fn is_tunnel_name(name: &str) -> bool {
    ["tun", "tap", "wg", "ppp", "ipsec", "utun"]
        .iter()
        .any(|prefix| name.starts_with(prefix))
}

/// `ipconfig getsummary` prints `  SSID : name`; macOS redacts the name
/// without Location access.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn ssid_from_summary(output: &str) -> Option<String> {
    let ssid = output.lines().find_map(|line| {
        let (key, value) = line.split_once(" : ")?;
        (key.trim() == "SSID").then(|| value.trim().to_string())
    })?;
    (!ssid.is_empty() && ssid != "<redacted>").then_some(ssid)
}

/// An interface that is up with an IPv4 address, by name. macOS keeps
/// several `utun` interfaces up for its own services, with IPv6 only, so a
/// tunnel counts once it carries IPv4.
#[cfg(unix)]
fn tunnels_with_ipv4(also: impl Fn(&str) -> bool) -> Option<bool> {
    use std::ffi::CStr;

    let mut list: *mut libc::ifaddrs = std::ptr::null_mut();
    if unsafe { libc::getifaddrs(&mut list) } != 0 {
        return None;
    }
    let mut found = false;
    let mut entry = list;
    while !entry.is_null() {
        let ifa = unsafe { &*entry };
        entry = ifa.ifa_next;
        let up = ifa.ifa_flags & (libc::IFF_UP as libc::c_uint) != 0;
        let ipv4 = !ifa.ifa_addr.is_null()
            && i32::from(unsafe { (*ifa.ifa_addr).sa_family }) == libc::AF_INET;
        if !up || !ipv4 || ifa.ifa_name.is_null() {
            continue;
        }
        let name = unsafe { CStr::from_ptr(ifa.ifa_name) }.to_string_lossy();
        if is_tunnel_name(&name) || also(&name) {
            found = true;
            break;
        }
    }
    unsafe { libc::freeifaddrs(list) };
    Some(found)
}

#[cfg(target_os = "linux")]
mod sys {
    use zbus::blocking::{Connection, Proxy};
    use zbus::zvariant::OwnedObjectPath;

    const NM: &str = "org.freedesktop.NetworkManager";
    const DEVICE_TYPE_WIFI: u32 = 2;
    /// `ARPHRD_NONE`: tun and WireGuard devices, whatever they are named.
    const ARPHRD_NONE: &str = "65534";

    #[derive(Default)]
    pub struct Network {
        system: Option<Connection>,
    }

    impl Network {
        /// NetworkManager's active access point on each Wi-Fi device.
        pub fn wifi_networks(&mut self) -> Option<Vec<String>> {
            if self.system.is_none() {
                self.system = Connection::system().ok();
            }
            let system = self.system.as_ref()?;
            let proxy = |path: &str, interface: &'static str| {
                Proxy::new(system, NM, path.to_string(), interface)
            };
            let devices: Vec<OwnedObjectPath> = proxy("/org/freedesktop/NetworkManager", NM)
                .and_then(|nm| nm.get_property("Devices"))
                .ok()?;
            let mut names = Vec::new();
            for device in devices {
                let kind: u32 = proxy(device.as_str(), "org.freedesktop.NetworkManager.Device")
                    .and_then(|device| device.get_property("DeviceType"))
                    .unwrap_or(0);
                if kind != DEVICE_TYPE_WIFI {
                    continue;
                }
                let Ok(access_point) = proxy(
                    device.as_str(),
                    "org.freedesktop.NetworkManager.Device.Wireless",
                )
                .and_then(|wireless| wireless.get_property::<OwnedObjectPath>("ActiveAccessPoint")) else {
                    continue;
                };
                if access_point.as_str() == "/" {
                    continue;
                }
                if let Ok(ssid) = proxy(
                    access_point.as_str(),
                    "org.freedesktop.NetworkManager.AccessPoint",
                )
                .and_then(|point| point.get_property::<Vec<u8>>("Ssid"))
                {
                    names.push(String::from_utf8_lossy(&ssid).into_owned());
                }
            }
            Some(names)
        }

        pub fn vpn_connected(&mut self) -> Option<bool> {
            super::tunnels_with_ipv4(|name| {
                std::fs::read_to_string(format!("/sys/class/net/{name}/type"))
                    .is_ok_and(|kind| kind.trim() == ARPHRD_NONE)
            })
        }
    }
}

#[cfg(target_os = "macos")]
mod sys {
    use std::process::Command;

    #[derive(Default)]
    pub struct Network {
        /// The Wi-Fi interface, such as `en0`, once found.
        wifi_device: Option<String>,
    }

    impl Network {
        /// The OS's own tools: `networksetup` names the Wi-Fi interface and
        /// `ipconfig getsummary` the network it's on.
        pub fn wifi_networks(&mut self) -> Option<Vec<String>> {
            if self.wifi_device.is_none() {
                let ports = Command::new("/usr/sbin/networksetup")
                    .arg("-listallhardwareports")
                    .output()
                    .ok()?;
                self.wifi_device = wifi_device(&String::from_utf8_lossy(&ports.stdout));
            }
            let device = self.wifi_device.as_ref()?;
            let summary = Command::new("/usr/sbin/ipconfig")
                .args(["getsummary", device])
                .output()
                .ok()?;
            let text = String::from_utf8_lossy(&summary.stdout);
            if text.contains("<redacted>") {
                return None;
            }
            Some(super::ssid_from_summary(&text).into_iter().collect())
        }

        pub fn vpn_connected(&mut self) -> Option<bool> {
            super::tunnels_with_ipv4(|_| false)
        }
    }

    /// The `Device:` line after `Hardware Port: Wi-Fi`.
    fn wifi_device(ports: &str) -> Option<String> {
        let mut lines = ports.lines();
        while let Some(line) = lines.next() {
            if line.trim() == "Hardware Port: Wi-Fi" {
                return lines
                    .next()
                    .and_then(|device| device.trim().strip_prefix("Device: "))
                    .map(str::to_string);
            }
        }
        None
    }
}

#[cfg(windows)]
mod sys {
    use std::ffi::c_void;

    use windows_sys::Win32::Foundation::{ERROR_BUFFER_OVERFLOW, ERROR_SUCCESS, HANDLE};
    use windows_sys::Win32::NetworkManagement::IpHelper::{
        GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_DNS_SERVER, GAA_FLAG_SKIP_MULTICAST,
        GetAdaptersAddresses, IP_ADAPTER_ADDRESSES_LH,
    };
    use windows_sys::Win32::NetworkManagement::Ndis::IfOperStatusUp;
    use windows_sys::Win32::NetworkManagement::WiFi::{
        WLAN_CONNECTION_ATTRIBUTES, WLAN_INTERFACE_INFO_LIST, WlanCloseHandle, WlanEnumInterfaces,
        WlanFreeMemory, WlanOpenHandle, WlanQueryInterface, wlan_interface_state_connected,
        wlan_intf_opcode_current_connection,
    };
    use windows_sys::Win32::Networking::WinSock::AF_UNSPEC;

    const IF_TYPE_PPP: u32 = 23;
    const IF_TYPE_PROP_VIRTUAL: u32 = 53;

    #[derive(Default)]
    pub struct Network;

    impl Network {
        /// The WLAN API's current connection on each connected interface.
        /// Windows 11 24H2 refuses it without Location access: `None`.
        pub fn wifi_networks(&mut self) -> Option<Vec<String>> {
            let mut version = 0u32;
            let mut handle: HANDLE = std::ptr::null_mut();
            if unsafe { WlanOpenHandle(2, std::ptr::null(), &mut version, &mut handle) }
                != ERROR_SUCCESS
            {
                // No WLAN service: a desktop on a cable is on no Wi-Fi.
                return Some(Vec::new());
            }
            let result = read_connections(handle);
            unsafe { WlanCloseHandle(handle, std::ptr::null()) };
            result
        }

        /// An adapter that is up and is a PPP link, a virtual adapter, or
        /// describes itself as a VPN, WireGuard or TAP tunnel.
        pub fn vpn_connected(&mut self) -> Option<bool> {
            let flags = GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST | GAA_FLAG_SKIP_DNS_SERVER;
            let mut size = 16 * 1024u32;
            let mut buffer: Vec<u64> = Vec::new();
            for _ in 0..3 {
                buffer = vec![0u64; size as usize / 8 + 1];
                let result = unsafe {
                    GetAdaptersAddresses(
                        u32::from(AF_UNSPEC),
                        flags,
                        std::ptr::null(),
                        buffer.as_mut_ptr().cast(),
                        &mut size,
                    )
                };
                match result {
                    ERROR_SUCCESS => break,
                    ERROR_BUFFER_OVERFLOW => continue,
                    _ => return None,
                }
            }
            let mut adapter = buffer.as_ptr() as *const IP_ADAPTER_ADDRESSES_LH;
            while !adapter.is_null() {
                let current = unsafe { &*adapter };
                adapter = current.Next;
                if current.OperStatus != IfOperStatusUp {
                    continue;
                }
                let description = wide_to_string(current.Description).to_lowercase();
                if matches!(current.IfType, IF_TYPE_PPP | IF_TYPE_PROP_VIRTUAL)
                    || ["vpn", "wireguard", "tap-windows", "wintun"]
                        .iter()
                        .any(|word| description.contains(word))
                {
                    return Some(true);
                }
            }
            Some(false)
        }
    }

    fn read_connections(handle: HANDLE) -> Option<Vec<String>> {
        let mut list: *mut WLAN_INTERFACE_INFO_LIST = std::ptr::null_mut();
        if unsafe { WlanEnumInterfaces(handle, std::ptr::null(), &mut list) } != ERROR_SUCCESS {
            return None;
        }
        let mut names = Some(Vec::new());
        let count = unsafe { (*list).dwNumberOfItems } as usize;
        let first = unsafe { (*list).InterfaceInfo.as_ptr() };
        for index in 0..count {
            let interface = unsafe { &*first.add(index) };
            if interface.isState != wlan_interface_state_connected {
                continue;
            }
            let mut size = 0u32;
            let mut data: *mut c_void = std::ptr::null_mut();
            let result = unsafe {
                WlanQueryInterface(
                    handle,
                    &interface.InterfaceGuid,
                    wlan_intf_opcode_current_connection,
                    std::ptr::null(),
                    &mut size,
                    &mut data,
                    std::ptr::null_mut(),
                )
            };
            if result != ERROR_SUCCESS || data.is_null() {
                // Connected, but Windows won't say to what.
                names = None;
                break;
            }
            let attributes = unsafe { &*(data as *const WLAN_CONNECTION_ATTRIBUTES) };
            let ssid = &attributes.wlanAssociationAttributes.dot11Ssid;
            let length = (ssid.uSSIDLength as usize).min(ssid.ucSSID.len());
            if let Some(names) = &mut names {
                names.push(String::from_utf8_lossy(&ssid.ucSSID[..length]).into_owned());
            }
            unsafe { WlanFreeMemory(data) };
        }
        unsafe { WlanFreeMemory(list as *const c_void) };
        names
    }

    fn wide_to_string(text: *const u16) -> String {
        if text.is_null() {
            return String::new();
        }
        let mut length = 0;
        while unsafe { *text.add(length) } != 0 {
            length += 1;
        }
        String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(text, length) })
    }
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
mod sys {
    #[derive(Default)]
    pub struct Network;

    impl Network {
        pub fn wifi_networks(&mut self) -> Option<Vec<String>> {
            None
        }
        pub fn vpn_connected(&mut self) -> Option<bool> {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_compare_loosely() {
        assert_eq!(normalize_network_name(" Office WiFi "), "office wifi");
    }

    #[test]
    fn tunnel_names() {
        for name in ["tun0", "tap1", "wg0", "ppp0", "ipsec0", "utun4"] {
            assert!(is_tunnel_name(name), "{name}");
        }
        for name in ["eth0", "en0", "wlan0", "lo", "docker0"] {
            assert!(!is_tunnel_name(name), "{name}");
        }
    }

    #[test]
    fn ssid_from_ipconfig() {
        let summary =
            "<dictionary> {\n  InterfaceType : WiFi\n  SSID : Office 5G\n  Security : WPA2\n}";
        assert_eq!(ssid_from_summary(summary).as_deref(), Some("Office 5G"));
        assert_eq!(ssid_from_summary("  SSID : <redacted>"), None);
        assert_eq!(ssid_from_summary("  InterfaceType : WiFi"), None);
    }

    #[test]
    fn reads_this_machine_without_failing() {
        let mut network = Network::default();
        let _ = network.wifi_networks();
        let _ = network.vpn_connected();
    }
}
