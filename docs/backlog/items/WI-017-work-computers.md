---
id: WI-017
title: Warn once on a managed computer, and obey an IT policy that turns input simulation off
type: feature
status: review
branch: feature/WI-017-managed-device
created: 2026-11-05
updated: 2026-11-05
related: [WI-014, WI-016]
epic:
children: []
---

## Proposal (rationale)

Phase 4's managed-device notice (Should) and admin policy (Could), from the "Unidle on work machines" risk notes. On a managed computer, Unidle will be seen, and the organization's rules decide whether it may be used. The app should say so once, point to power-only mode, and let IT turn input simulation off.

## Decision

`accepted` — 2026-11-05, Antoine ("continue with the rest of phase 4").

## Plan

### Requirements

- SPEC §16, WC-1 to WC-4.
- The check is local and read-only, never names monitoring or security products, and never says a computer is safe.
- The policy is enforced in the core, where every nudge checks it, not only in the window.

### Impacts

- `crates/core`: `platform/managed.rs` (the signals and the policy on three OSes), `InputUnavailable::Policy`, `Input::set_policy`, `SystemAdapter` reading it at launch, `managedCheckDone` setting. New `windows-sys` features: `NetworkManagement_NetManagement`, `System_Registry`.
- `src-tauri`: `work_computer.rs` commands, the "Learn more" link, the CLI status line.
- `ui`: a notice view, a Work computer row in Settings → System, the policy status line. README "On a work computer" with the disclaimer, FAQ and IT instructions.

### Approach

- **The policy is an input capability.** A policy that turns input off looks to the controller like Wayland does: nudges are unavailable, the assertion holds, the window says why. No new state.
- **Run the check from the window, not at startup.** It runs the first time the window asks, so a launch at login in the tray never spends it unseen.
- **No product names.** Scanning for trackers or EDR would turn the feature into a detection-avoidance checklist (the risk notes' "what not to check").

### Acceptance criteria / tests

1. Unit tests: the policy file read leniently; `profiles` output read as enrolled only on Yes; the Linux realm files; the setting's default and round trip.
2. Core clippy clean for Linux, `x86_64-pc-windows-gnu` and `x86_64-apple-darwin`.
3. Screenshot of the notice.
4. On real machines: a domain-joined or Intune-enrolled Windows PC, an MDM-enrolled Mac, and the policy on each OS. **Needs real machines.**

### Out of scope

- Disabling the whole app by policy: application control (AppLocker, WDAC, Santa) does that better.
- An IT approval kit (MSI, signed .pkg, deployment notes): Phase 5, with code signing.

## Execution

- Branch `feature/WI-017-managed-device`, stacked on `feature/WI-016-activity-log` (settings fields).

## Review

1. Met. `cargo test --workspace`.
2. Met.
3. Met. Checked by eye.
4. **Open.** Needs real machines.
