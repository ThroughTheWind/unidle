// The parts inside the power button for each style, as in
// docs/design/preview.html. The styling is docs/design/power-button.css.

import { strings } from "./strings";

export const BUTTON_STYLES = ["halo", "orbit", "pulse", "eye"] as const;
export type ButtonStyle = (typeof BUTTON_STYLES)[number];

/** A missing or unknown style falls back to Halo (design system). */
export function buttonStyle(value: unknown): ButtonStyle {
  return BUTTON_STYLES.find((style) => style === value) ?? "halo";
}

const glyph = (size: number) =>
  `<svg class="ub-glyph" width="${size}" height="${size}" viewBox="0 0 24 24" aria-hidden="true">` +
  `<path d="M12 3v8"/><path d="M6.3 7.2a8 8 0 1 0 11.4 0"/></svg>`;

const ECG = "M0 22h28l6-12 8 26 7-30 7 22 5-6h49";
const EYE = "M10 52C24 30 80 30 94 52C80 74 24 74 10 52Z";

const parts: Record<ButtonStyle, () => string> = {
  halo: () => glyph(56),
  orbit: () =>
    `<svg class="ub-ring" viewBox="0 0 196 196" aria-hidden="true">` +
    `<circle class="ub-track" cx="98" cy="98" r="94"/><circle class="ub-comet" cx="98" cy="98" r="94"/></svg>` +
    `<span class="ub-sat"></span>` +
    `<span class="ub-core">${glyph(48)}` +
    `<span class="ub-readout"><b class="ub-clock"></b><small>${strings.orbitCaption}</small></span></span>`,
  pulse: () =>
    `<svg class="ub-ecg" viewBox="0 0 110 44" aria-hidden="true">` +
    `<path class="ub-base" d="${ECG}"/><path d="${ECG}"/></svg>` +
    `<span class="ub-knob">${glyph(36)}</span>`,
  eye: () =>
    `<svg viewBox="0 0 104 104" aria-hidden="true"><g class="ub-open">` +
    `<path class="ub-white" d="${EYE}"/><g class="ub-look"><circle class="ub-pupil" cx="52" cy="52" r="13"/></g>` +
    `<path class="ub-lid" d="${EYE}"/></g>` +
    `<text class="ub-z" x="74" y="34">z</text><text class="ub-z ub-z2" x="86" y="22">z</text></svg>`,
};

/** Swaps the button's parts without touching its on/off state. */
export function setButtonStyle(button: HTMLElement, style: ButtonStyle): void {
  if (button.dataset.style === style) return;
  button.dataset.style = style;
  button.innerHTML = parts[style]();
}
