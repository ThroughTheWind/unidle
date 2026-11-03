# Changelog

This project follows [Semantic Versioning](https://semver.org).

## Unreleased

## 0.1.0

The first release: one ON/OFF toggle that keeps your computer awake and your chat status active, on Windows, macOS and Linux.

### Added

- Turning ON really keeps the computer awake: the app holds an OS power assertion on Windows, macOS and Linux, and releases it on OFF, on quit and if the process dies. It never blocks a sleep you ask for, such as closing the lid.
- While ON, the app sends a tiny input nudge about once a minute so chat apps see activity: a 1 px pointer move and back, or a key that types nothing. It skips the nudge while you are using the computer. On Wayland, and on macOS until you grant Accessibility, it keeps the system awake without nudges and says so.
- A tray / menu bar icon that shows ON or OFF by its shape, with a menu to turn activity on or off, see how long it has been on, open the window and quit. Closing the window keeps the app running in the tray and frees the window's memory. Quitting turns activity off first.
- The window has a power button in four styles to pick from in Settings: Halo (the default), Orbit, Pulse and Wake eye. If turning ON fails, the window says why instead of staying silent. The Geist fonts ship with the app.
- A Settings screen covers every setting: nudge on or off, method, key and interval, keep display on, launch at login (starts in the tray), restore last state, a global shortcut you record in place, and a notification on each change. Settings are stored in `settings.json` in your config folder. A broken file never stops the app; bad values fall back to their defaults.
- Opening the app while it's already running shows its window instead of starting a second copy.
- Installers for Windows (`.exe`, `.msi`), macOS (universal `.dmg`) and Linux (`.AppImage`, `.deb`), and a landing page with per-OS download links at [throughthewind.github.io/unidle](https://throughthewind.github.io/unidle/).

### Known limitations

- The installers are not code-signed yet, so Windows SmartScreen and macOS Gatekeeper ask you to confirm the first launch.
- On Wayland, input nudges are not possible; the app keeps the system awake only.
- On macOS, nudges need the Accessibility permission. Until you grant it, the app keeps the system awake only.
