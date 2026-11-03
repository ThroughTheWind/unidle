# Icons

Two app icons, chosen in Settings → App icon (`appIcon` in `settings.json`):

| | Wake eye (default) | Power button |
|---|---|---|
| App icon, 48 px and up | `icon.svg` | `power/icon.svg` |
| App icon, 16 to 32 px (no rings, bigger disc) | `icon-small.svg` | `power/icon-small.svg` |
| Runtime image for the window and Dock (512 px) | `icon.png` | `power/icon.png` |
| Tray, 32 × 32 | `tray/*.svg` | `tray/power/*.svg` |

The bundled files (`*.png` here, `icon.icns`, `icon.ico`) are the Wake eye: installers and launchers read them before the app starts. Colors come from `docs/design/tokens.css` (dark theme).

To regenerate after changing a source:

1. Render `icon.svg` to a 1024 px PNG with any SVG renderer that handles filters and gradients (Chromium does; ImageMagick's built-in renderer does not).
2. `npx tauri icon <that png>` into a scratch folder, and copy the desktop files (the PNGs, `icon.icns`, `icon.ico`) here. Skip `android/` and `ios/`.
3. Render `icon-small.svg` at 16, 24, 30 and 32 px. Replace `32x32.png` and `Square30x30Logo.png`, and rebuild `icon.ico` with the 16, 24 and 32 px frames from `icon-small.svg` and the 48, 64 and 256 px frames from `icon.svg`.
4. Render `power/icon.svg` to `power/icon.png` at 512 px.
5. Render each tray `.svg` to the `.png` of the same name at 32 × 32.
