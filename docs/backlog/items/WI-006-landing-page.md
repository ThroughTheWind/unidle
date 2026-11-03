---
id: WI-006
title: Publish a one-page landing site with per-OS downloads
type: feature
status: review
branch: feature/WI-006-landing-page
created: 2026-11-03
updated: 2026-11-03
related: []
epic:
children: []
---

## Proposal (rationale)

Unidle has no public page. People who hear about it land on the repository, which is written for contributors. A one-page site that says what the app does and offers a download for each OS, plus a link to the code, is the front door for users once the first release ships.

## Decision

`accepted` — 2026-11-03, Antoine asked for a one-page landing page with per-OS download links and a GitHub link.

## Plan

### Requirements

- One static page in `site/` presenting the app, its v1 features (SPEC §5), and responsible use (SPEC §3).
- A download button for Windows, macOS and Linux that points at the GitHub Releases `latest/download/` URL of the asset `tauri-action` builds for that OS, plus a link to the repository.
- Uses the current design system: Geist, the graphite palette in `site/tokens.css` and the Halo power button from `site/power-button.css`, in light and dark mode.
- Deployed to GitHub Pages from `main` by a workflow; PRs touching the site only build it.

### Impacts

- New `site/` folder and `.github/workflows/pages.yml`. No change to the app, its UI or its network policy: the GitHub API call is made by the website, not the app.
- `site/tokens.css` and `site/power-button.css` are copies of the latest design files, which are newer than `docs/design/tokens.css`. Updating `docs/design/` to match is separate work.
- Asset file names include the version, so the workflow reads it from `Cargo.toml` at deploy time instead of anyone typing it.

### Approach

- Static HTML, CSS and a small script. No build tool, to match the "no UI framework" convention.
- Static links: `releases/latest/download/Unidle_<version>_x64-setup.exe` (and `.msi`), `_universal.dmg`, `_amd64.AppImage` (and `.deb`), the names Tauri 2 gives each bundle for the matrix in `release.yml`.
- In the browser, the script asks the GitHub API for the latest release and swaps in the exact asset URLs; if there is no published release yet, the buttons open the Releases page and a notice says the first release is coming. Offline or rate-limited, the static links stay.
- The visitor's OS is detected to highlight its card and label the hero button.

### Acceptance criteria / tests

- The page renders without horizontal scroll at 390 px and 1280 px wide, in light and dark mode (checked with Playwright screenshots).
- The Halo demo toggles ON/OFF with mouse and keyboard, and respects reduced motion.
- After the workflow's substitution, every download link names a versioned asset and no `__VERSION__` placeholder remains (the build step fails otherwise).
- With no published release, every download button opens the Releases page.

### Out of scope

- Code signing, and publishing the first release (needs a `v*` tag, and `release.yml` creates a draft that must be published before `latest` resolves).
- A custom domain, analytics, and updating `docs/design/` to the new design system. Nothing deferred to a work item yet.

## Execution

Branch `feature/WI-006-landing-page`. Built as planned.

## Review

Rendered with Playwright at 1280 px (light and dark) and 390 px: no horizontal overflow, Halo demo toggles, and with no release the API answers 404 and all buttons open the Releases page. The deploy step itself runs only on `main`, so Pages has to be enabled with "GitHub Actions" as the source before the first deploy.
