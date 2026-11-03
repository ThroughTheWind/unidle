# Changelog

This project follows [Semantic Versioning](https://semver.org).

## Unreleased

### Added

- While ON, the app sends a tiny input nudge about once a minute so chat apps see activity: a 1 px pointer move and back, or a key that types nothing. It skips the nudge while you are using the computer. On Wayland, and on macOS until you grant Accessibility, it keeps the system awake without nudges and says so.
- Turning ON now really keeps the computer awake: the app holds an OS power assertion on Windows, macOS and Linux, and releases it on OFF, on quit and if the process dies. It never blocks a sleep you ask for, such as closing the lid.
- Project skeleton: Tauri 2 app, Rust simulation core, window UI, CI and release workflows. Nothing is simulated yet.
