# Study: telling users a new version exists

| | |
|---|---|
| Status | Kept for later. For now the window only links to the releases page (no network). |
| Date | 2026-11-03 |
| Related | [SPEC](../SPEC.md) NFR-2, [ROADMAP](../ROADMAP.md) Phase 5 "Opt-in auto-update" |

## What shipped instead

An icon button next to Settings opens `github.com/ThroughTheWind/unidle/releases` in the browser, and the Settings screen adds a second one for the website. The browser loads the page, so the app itself still makes no network request and SPEC NFR-2 holds as written.

## Option A: plain GitHub API check (size S, about 1 to 2 days)

1. On launch, at most once per 24 h, and only if a `checkForUpdates` setting is on (default off), Rust sends one `GET https://api.github.com/repos/ThroughTheWind/unidle/releases/latest` with `User-Agent: Unidle/<version>` and nothing else.
2. Strip the `v` from `tag_name` and compare with the `semver` crate against `app.package_info().version` (from the workspace `Cargo.toml`).
3. If newer, show "Update available: vX.Y.Z" in the tray menu and the window, linking to the release's `html_url`. Never download or install.
4. Any failure (offline, 404, rate limit, bad JSON) is silent.

Pieces: `ureq` with `native-tls` (Schannel, Security.framework, and the OpenSSL that WebKitGTK already pulls in) is the lightest client, a few hundred KB; `reqwest` + `rustls` adds more size and a tokio runtime. Doing it in Rust keeps the webview CSP at `ipc:` only. Version comparison and JSON parsing are pure functions that belong in `unidle-core` and test without network.

Risks:

- **Spec.** NFR-2 and acceptance criterion 9 promise zero connections. Opt-in, default off, fixes it (wording below).
- **Draft releases.** `release.yml` creates drafts, and `releases/latest` ignores drafts and prereleases: the check sees a release only after it is published.
- **Rate limit.** 60 unauthenticated requests per hour per IP; one a day is far below, even behind a NAT.
- **Tags.** A non-SemVer tag (`v1.0`) must be ignored, not misread.
- **Proxies.** `ureq` does not read system proxy settings by default. Acceptable for a best-effort check.
- **Trust.** The notice only links to GitHub, so a spoofed response can at worst show a wrong version string.
- **Perception.** People run this on work machines; "phones home" matters. Default off, one documented endpoint, no identifiers.

## Option B: Tauri updater plugin (size M, Phase 5)

`tauri-plugin-updater` downloads and installs in place. It needs an updater signing keypair (`tauri signer generate`, private key and password as GitHub secrets; losing the key strands every existing install), `bundle.createUpdaterArtifacts`, a `plugins.updater` block with the public key and endpoint, `latest.json` from `tauri-action`, capability permissions, and progress and restart UI.

Limits today: on Linux only the AppImage self-updates (not the `.deb`); on Windows and macOS unsigned installs keep SmartScreen and Gatekeeper friction until OS code signing (also Phase 5) lands.

Option A's setting, wording and notice carry straight over to B.

## Comparison

| | A: GitHub API notice | B: Tauri updater |
|---|---|---|
| User gets | Notice and link | One-click install |
| Signing | None | Updater keypair; OS signing for a good experience |
| CI changes | None | Updater artifacts, `latest.json`, secrets |
| Size | Small | Plugin and HTTP stack |
| Effort | S | M |
| Main risk | Spec wording | Key loss, platform gaps |

## Spec changes if A ships

- §1 and §2 goal 5: "no network access" becomes "no network access by default".
- NFR-2: "No network access by default. The only optional connection is an update check (setting, default off) that sends one HTTPS GET to `api.github.com/repos/ThroughTheWind/unidle/releases/latest` at most once a day, with no identifiers beyond a `User-Agent` naming the app version. No telemetry. No logging of user input."
- §5.4: new row "Check for updates | Off | One request a day to GitHub Releases; shows a notice, never installs."
- New FR: "When update checks are on and a newer published release exists, the tray menu and window show 'Update available: vX.Y.Z', linking to the release page. Failures are silent."
- Acceptance criterion 9: "With update checks off (the default), the app makes zero outbound network connections. With them on, the only connection is to `api.github.com`."
- README: a short Privacy section.
