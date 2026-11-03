# Changelog

This project follows [Semantic Versioning](https://semver.org).

## Unreleased

### Added

- Turning ON now really keeps the computer awake: the app holds an OS power assertion on Windows, macOS and Linux, and releases it on OFF, on quit and if the process dies. It never blocks a sleep you ask for, such as closing the lid.
- Project skeleton: Tauri 2 app, Rust simulation core, window UI, CI and release workflows. Nothing is simulated yet.
