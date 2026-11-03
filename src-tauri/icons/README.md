# Icons

`icon.svg` is the source of the app icon from 48 px up; `icon-small.svg` (no rings, bigger disc and glyph) is the source from 16 to 32 px. `tray/*.svg` are the sources of the tray images, rendered at 32 × 32. Colors come from `docs/design/tokens.css` (dark theme).

To regenerate after changing a source:

1. Render `icon.svg` to a 1024 px PNG with any SVG renderer that handles filters and gradients (Chromium does; ImageMagick's built-in renderer does not).
2. `npx tauri icon <that png>` into a scratch folder, and copy the desktop files (the PNGs, `icon.icns`, `icon.ico`) here. Skip `android/` and `ios/`.
3. Render `icon-small.svg` at 16, 24, 30 and 32 px. Replace `32x32.png` and `Square30x30Logo.png`, and rebuild `icon.ico` with the 16, 24 and 32 px frames from `icon-small.svg` and the 48, 64 and 256 px frames from `icon.svg`.
4. Render each `tray/*.svg` to the `.png` of the same name at 32 × 32.
