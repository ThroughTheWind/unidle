# Changelog

This project follows [Semantic Versioning](https://semver.org).

## Unreleased

### Added

- A new app and tray icon in the app's graphite and green: the Halo power button, lit. The tray shows an outlined button when OFF and a filled green one when ON.
- The window has a new power button, in four styles to pick from in Settings: Halo (the default), Orbit, Pulse and Wake eye. A Settings screen now covers every setting, including recording a global shortcut. If turning ON fails, the window says why instead of staying silent. The Geist fonts ship with the app.
- Landing page in `site/` with per-OS download links, deployed to GitHub Pages.
- Opening the app while it's already running shows its window instead of starting a second copy.
- Settings are stored in `settings.json` in your config folder: nudge on or off, method, key and interval, keep display on, launch at login (starts in the tray), restore last state, a global shortcut, and a notification on each change. A broken file never stops the app; bad values fall back to their defaults.
- A tray / menu bar icon that shows ON or OFF by its shape, with a menu to turn activity on or off, see how long it has been on, open the window and quit. Closing the window keeps the app running in the tray and frees the window's memory. Quitting turns activity off first.
- While ON, the app sends a tiny input nudge about once a minute so chat apps see activity: a 1 px pointer move and back, or a key that types nothing. It skips the nudge while you are using the computer. On Wayland, and on macOS until you grant Accessibility, it keeps the system awake without nudges and says so.
- Turning ON now really keeps the computer awake: the app holds an OS power assertion on Windows, macOS and Linux, and releases it on OFF, on quit and if the process dies. It never blocks a sleep you ask for, such as closing the lid.
- Project skeleton: Tauri 2 app, Rust simulation core, window UI, CI and release workflows. Nothing is simulated yet.
