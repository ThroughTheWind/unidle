---
id: WI-018
title: Hold activity unless on a chosen Wi-Fi network or a VPN
type: feature
status: review
branch: feature/WI-018-network-conditions
created: 2026-11-05
updated: 2026-11-05
related: [WI-015]
epic:
children: []
---

## Proposal (rationale)

Phase 4's Could row: only on a given Wi-Fi network, or when a VPN is connected. For example, keep a work laptop awake at the office or on the company VPN, and let it sleep at a café. It is one more condition on the mechanism WI-015 built.

## Decision

`accepted` — 2026-11-05, Antoine ("Build the extras now" on the Phase 4 extras question).

## Plan

### Requirements

- SPEC §14: two rows in the conditions table, and the readings in CO-4.
- A network the OS won't name never holds (CO-1), including when Windows or macOS withhold the name for lack of Location access.

### Impacts

- `crates/core`: `platform/network.rs` (Wi-Fi names and VPN state on three OSes), adapter methods with `None` defaults, `Conditions::on_networks` and `only_on_vpn`, `Hold::NotOnNetwork` and `Hold::NoVpn`, settings fields. New `windows-sys` features: IpHelper, Ndis, WiFi, WinSock.
- `src-tauri`: the two hold lines. `ui`: two rows in the Conditions group.

### Approach

- **Wi-Fi names from what the OS already exposes**: NetworkManager over D-Bus, the WLAN API, `ipconfig getsummary`. No scanning.
- **VPN by interface, not by product.** A tunnel interface that is up with IPv4 covers OpenVPN, WireGuard, IPsec and PPP clients without naming any vendor; macOS's own `utun` interfaces carry IPv6 only, so they don't count.
- **Not asking for Location access.** Asking for it to read a network name is out of proportion for a Could row; the condition says in its hint that the OS may need it.

### Acceptance criteria / tests

1. Unit tests: the decision for each, with unknown readings; the controller holding on the network, then the VPN, then resuming; tunnel names; `ipconfig` output including a redacted name; settings round trip.
2. Core clippy clean for Linux, `x86_64-pc-windows-gnu` and `x86_64-apple-darwin`.
3. On real machines: switching Wi-Fi networks and connecting a VPN on each OS. **Needs real machines.**

### Out of scope

- Wired networks by name, and asking for Location access.

## Execution

- Branch `feature/WI-018-network-conditions`, stacked on `feature/WI-017-managed-device`.

## Review

1. Met. `cargo test --workspace`.
2. Met.
3. **Open.** Needs real machines.
