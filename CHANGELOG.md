# Changelog

This project follows [Semantic Versioning](https://semver.org).

## Unreleased

## 0.2.0

Timers and schedules: keep the computer awake for a while, or at set times each week, without touching the toggle.

### Added

- Quick timers: keep the computer awake for 15 minutes, 30 minutes, 1 hour, 2 hours or until a time, from the tray's "Keep awake for" menu or the row under the button. When the timer ends, it turns off, or the schedule takes over.
- The tray and window say why the app is on and what happens next: "Timer · OFF at 18:00", "Scheduled · ON tomorrow at 09:00", "Schedule paused until 18:00".
- A weekly schedule: in Settings, "Turn on automatically" with as many times per day as you like (overnight too) and days off for holidays. Turning it off by hand during a scheduled time pauses it until that time ends. Times follow your local clock across daylight saving changes, and after sleep the app catches up within 15 seconds.
- Import and export the schedule's times and days off as a file, to back it up or use it on another computer.
- Settings can hide the timer row on the main screen (the tray keeps its timers), and can turn off "Click the tray icon to turn on or off". That is on by default on Windows and Linux, and when it's on, right-click opens the menu.

### Changed

- The main screen no longer shows the "Unidle" title: the button is the window.
- Lists, time and date fields look cleaner, and their pop-ups follow the app's Light or Dark theme.

### Known limitations

- The installers are still not code-signed, so Windows SmartScreen and macOS Gatekeeper ask you to confirm the first launch.
- The app has to be running for a schedule to turn it on. Launch at login keeps it running.

## 0.1.0

The first release: one ON/OFF toggle that keeps your computer awake and your chat status active, on Windows, macOS and Linux.

### Added

- Turning ON really keeps the computer awake: the app holds an OS power assertion on Windows, macOS and Linux, and releases it on OFF, on quit and if the process dies. It never blocks a sleep you ask for, such as closing the lid.
- While ON, the app sends a tiny input nudge about once a minute so chat apps see activity: a 1 px pointer move and back, or a key that types nothing. It skips the nudge while you are using the computer. On Wayland, and on macOS until you grant Accessibility, it keeps the system awake without nudges and says so.
- The app and tray icon is the Wake eye, in the app's graphite and green. The tray shows a closed eye when OFF and an open eye when ON. Settings can switch the tray, taskbar and Dock icons to the Halo power button instead, which is outlined when OFF and filled green when ON.
- A tray / menu bar menu to turn activity on or off, see how long it has been on, open the window and quit. Closing the window keeps the app running in the tray and frees the window's memory. Quitting turns activity off first.
- The window has a power button in four styles to pick from in Settings: Halo (the default), Orbit, Pulse and Wake eye. If turning ON fails, the window says why instead of staying silent. The Geist fonts ship with the app.
- A Theme setting: System (the default), Light or Dark. The window and its title bar follow it.
- A Settings screen covers every setting: nudge on or off, method, key and interval, keep display on, launch at login (starts in the tray), restore last state, a global shortcut you record in place, and a notification on each change. Settings are stored in `settings.json` in your config folder. A broken file never stops the app; bad values fall back to their defaults.
- A "Check for new versions" icon next to Settings opens the releases page in your browser, and Settings links to the website. The app itself still makes no network request.
- Opening the app while it's already running shows its window instead of starting a second copy.
- Installers for Windows (`.exe`, `.msi`), macOS (universal `.dmg`) and Linux (`.AppImage`, `.deb`), and a landing page with per-OS download links at [throughthewind.github.io/unidle](https://throughthewind.github.io/unidle/).

### Known limitations

- The installers are not code-signed yet, so Windows SmartScreen and macOS Gatekeeper ask you to confirm the first launch.
- On Wayland, input nudges are not possible; the app keeps the system awake only.
- On macOS, nudges need the Accessibility permission. Until you grant it, the app keeps the system awake only.
