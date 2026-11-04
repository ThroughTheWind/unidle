import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

import { type ButtonStyle, BUTTON_STYLES, buttonStyle, setButtonStyle } from "./power-button";
import { shortcutFromEvent } from "./shortcut";
import { strings } from "./strings";
import { nextHour, shortWhen, when } from "./when";

interface Status {
  on: boolean;
  elapsedSecs: number;
  powerAssertion: boolean;
  nudging: boolean;
  inputUnavailable: "needsPermission" | "wayland" | "noDisplay" | "unsupported" | null;
  /** Who turned it on (SPEC SC-4). */
  source: "manual" | "timer" | "schedule" | null;
  /** The next time a timer or the schedule turns it ON or OFF; ms since the epoch. */
  nextChange: { at: number; on: boolean } | null;
  /** While a manual OFF pauses a schedule window: when the window ends. */
  pausedUntil: number | null;
}

type NudgeMethod = "pointer" | "keystroke" | "both";

const THEMES = ["system", "light", "dark"] as const;
type Theme = (typeof THEMES)[number];

/** The settings file's fields, as `get_settings` returns them. */
interface Settings {
  nudgeEnabled: boolean;
  nudgeMethod: NudgeMethod;
  nudgeKey: string;
  nudgeIntervalSecs: number;
  keepDisplayOn: boolean;
  launchAtLogin: boolean;
  restoreLastState: boolean;
  lastStateOn: boolean;
  globalShortcut: string | null;
  notifyOnChange: boolean;
  trayNoticeShown: boolean;
  buttonStyle: ButtonStyle;
  theme: Theme;
  appIcon: AppIcon;
}

/** Mirrors `APP_ICONS` in crates/core/src/settings.rs. The first is the default. */
const APP_ICONS = ["eye", "power"] as const;
type AppIcon = (typeof APP_ICONS)[number];

function appIcon(value: unknown): AppIcon {
  return APP_ICONS.find((icon) => icon === value) ?? "eye";
}

const MIN_INTERVAL = 10;
const MAX_INTERVAL = 600;

function element<T extends HTMLElement = HTMLElement>(id: string): T {
  const found = document.getElementById(id);
  if (!found) throw new Error(`Missing element #${id}`);
  return found as T;
}

function text(id: string, value: string): void {
  element(id).textContent = value;
}

function message(error: unknown): string {
  return typeof error === "string" ? error : error instanceof Error ? error.message : String(error);
}

// ── Main view ────────────────────────────────────────────────────────────

const shell = element("window");
const power = element<HTMLButtonElement>("power");
const state = element("state");
const timer = element("timer");
const statusLine = element("status");
const statusText = element("status-text");
const statusAction = element<HTMLButtonElement>("status-action");
const detail = element("detail");
const detailText = element("detail-text");

let status: Status | null = null;
let settings: Settings | null = null;
/** Set while a toggle is in flight: the button shows where it is going. */
let pending: boolean | null = null;
/** The last toggle's failure, until the next toggle succeeds (F-003). */
let toggleError: string | null = null;

function formatElapsed(totalSecs: number): string {
  const pad = (value: number) => String(value).padStart(2, "0");
  const hours = Math.floor(totalSecs / 3600);
  const minutes = Math.floor((totalSecs % 3600) / 60);
  return `${pad(hours)}:${pad(minutes)}:${pad(totalSecs % 60)}`;
}

function setStatusLine(kind: "warning" | "error" | null, line = ""): void {
  statusLine.hidden = kind === null;
  statusLine.className = `as-status status-line ${kind ?? ""}`;
  statusText.textContent = line;
  statusLine.title = line;
}

function renderStatus(): void {
  if (!status) return;
  const style = buttonStyle(settings?.buttonStyle);
  const eye = style === "eye";
  const on = pending ?? status.on;

  power.setAttribute("aria-checked", String(on));
  power.setAttribute("aria-busy", String(pending !== null));
  power.disabled = pending !== null;
  shell.classList.toggle("is-on", on);

  const elapsed = status.on ? formatElapsed(status.elapsedSecs) : "";
  const clock = power.querySelector(".ub-clock");
  if (clock) clock.textContent = elapsed || formatElapsed(0);

  if (pending !== null) {
    state.textContent = pending ? strings.starting : strings.stopping;
    timer.textContent = "";
  } else if (on) {
    state.textContent = eye ? strings.awake : strings.on;
    // Orbit shows the time inside its core.
    timer.textContent = style === "orbit" ? strings.keepingAwake : elapsed;
  } else {
    state.textContent = eye ? strings.asleep : strings.off;
    timer.textContent = eye ? strings.clickToWake : strings.clickToStart;
  }

  renderDetail(status);

  // Only what needs attention gets a line; the state word says the rest.
  const wantsNudges = settings?.nudgeEnabled ?? true;
  statusAction.hidden = true;
  if (toggleError !== null) {
    setStatusLine("error", toggleError);
  } else if (status.on && !status.powerAssertion) {
    // Never claim to keep the machine awake unless the adapter can.
    setStatusLine("warning", strings.notImplemented);
  } else if (status.on && wantsNudges && status.inputUnavailable !== null) {
    // Power assertion only: say so, without blocking anything (SPEC PB-2).
    setStatusLine("warning", strings.inputUnavailable);
    statusAction.hidden = status.inputUnavailable !== "needsPermission";
  } else {
    setStatusLine(null);
  }
}

/** "Timer · OFF at 18:00", "Scheduled · ON tomorrow at 09:00" (SPEC SC-4). */
function renderDetail(current: Status): void {
  let line: string | null = null;
  const next = current.nextChange ? strings.nextChange(current.nextChange.on, when(current.nextChange.at)) : null;
  if (current.pausedUntil !== null) {
    line = strings.paused(shortWhen(current.pausedUntil));
  } else if (current.source !== null) {
    const source = strings.sources[current.source];
    line = next ? `${source} · ${next}` : source;
  } else if (next) {
    // OFF: only the schedule plans an ON.
    line = `${strings.sources.schedule} · ${next}`;
  }
  detail.hidden = line === null;
  // Blue is for scheduling (design system), so a plain manual ON stays muted.
  const planned = current.nextChange !== null || current.pausedUntil !== null;
  detail.className = `as-status detail-line ${planned ? "scheduled" : "idle"}`;
  detailText.textContent = line ?? "";
}

function applyStatus(next: Status): void {
  status = next;
  renderStatus();
}

async function refreshStatus(): Promise<void> {
  applyStatus(await invoke<Status>("get_status"));
}

power.addEventListener("click", async () => {
  if (pending !== null || !status) return;
  const on = !status.on;
  pending = on;
  renderStatus();
  try {
    const next = await invoke<Status>("set_active", { on });
    toggleError = null;
    pending = null;
    applyStatus(next);
  } catch (error) {
    // Snap back to the real state and say why (design system: error state).
    toggleError = `${on ? strings.turnOnFailed : strings.turnOffFailed}: ${message(error)}`;
    pending = null;
    await refreshStatus();
  }
});

// ── Quick timers ─────────────────────────────────────────────────────────

const timers = element("timers");
const untilForm = element<HTMLFormElement>("until-form");
const untilTime = element<HTMLInputElement>("until-time");
const untilCancel = element<HTMLButtonElement>("until-cancel");

async function startTimer(args: { minutes: number } | { until: string }): Promise<void> {
  if (pending !== null) return;
  try {
    applyStatus(await invoke<Status>("start_timer", args));
    toggleError = null;
  } catch (error) {
    toggleError = `${strings.timerFailed}: ${message(error)}`;
    await refreshStatus();
  }
}

function showUntil(open: boolean): void {
  timers.hidden = open;
  untilForm.hidden = !open;
  if (open) {
    untilTime.value = nextHour();
    untilTime.focus();
  } else {
    timers.querySelector<HTMLButtonElement>("[data-until]")?.focus();
  }
}

timers.setAttribute("aria-label", strings.keepAwakeFor);
for (const timer of strings.timers) {
  const button = document.createElement("button");
  button.type = "button";
  button.className = "as-btn secondary timer";
  button.textContent = timer.label;
  button.setAttribute("aria-label", `${strings.keepAwakeFor} ${timer.name}`);
  button.title = button.getAttribute("aria-label") ?? "";
  button.addEventListener("click", () => void startTimer({ minutes: timer.minutes }));
  timers.append(button);
}
const untilButton = document.createElement("button");
untilButton.type = "button";
untilButton.className = "as-btn secondary timer";
untilButton.dataset.until = "";
untilButton.textContent = strings.timerUntil;
untilButton.setAttribute("aria-label", `${strings.keepAwakeFor}: ${strings.timerUntilName}`);
untilButton.title = strings.timerUntilName;
untilButton.addEventListener("click", () => showUntil(true));
timers.append(untilButton);

untilForm.addEventListener("submit", (event) => {
  event.preventDefault();
  if (!untilTime.value) return;
  showUntil(false);
  void startTimer({ until: untilTime.value.slice(0, 5) });
});
untilCancel.addEventListener("click", () => showUntil(false));
untilForm.addEventListener("keydown", (event) => {
  if (event.key === "Escape") {
    event.stopPropagation();
    showUntil(false);
  }
});

statusAction.textContent = strings.openAccessibility;
statusAction.addEventListener("click", () => {
  void invoke("open_input_permission_settings");
});

// ── Settings view ────────────────────────────────────────────────────────

const mainView = element("main-view");
const settingsView = element("settings-view");
const back = element<HTMLButtonElement>("back");
const openSettings = element<HTMLButtonElement>("open-settings");
const checkUpdates = element<HTMLButtonElement>("check-updates");
const openWebsite = element<HTMLButtonElement>("open-website");
const stylePicker = element("style-picker");
const methodPicker = element("method-picker");
const iconPicker = element("icon-picker");
const nudgeEnabled = element<HTMLInputElement>("nudge-enabled");
const nudgeKey = element<HTMLSelectElement>("nudge-key");
const nudgeInterval = element<HTMLInputElement>("nudge-interval");
const keepDisplayOn = element<HTMLInputElement>("keep-display-on");
const launchAtLogin = element<HTMLInputElement>("launch-at-login");
const restoreLastState = element<HTMLInputElement>("restore-last-state");
const notifyOnChange = element<HTMLInputElement>("notify-on-change");
const shortcutRecord = element<HTMLButtonElement>("shortcut-record");
const shortcutClear = element<HTMLButtonElement>("shortcut-clear");
const settingsError = element("settings-error");
const settingsErrorText = element("settings-error-text");

let recording = false;

function showSettingsError(line: string | null): void {
  settingsError.hidden = line === null;
  settingsErrorText.textContent = line ?? "";
}

/** A segmented control with radio semantics and arrow-key movement. */
function segmented<T extends string>(
  group: HTMLElement,
  options: readonly T[],
  label: (option: T) => string,
  choose: (option: T) => void,
): (selected: T) => void {
  const radios = options.map((option) => {
    const radio = document.createElement("button");
    radio.type = "button";
    radio.setAttribute("role", "radio");
    radio.dataset.value = option;
    radio.textContent = label(option);
    radio.addEventListener("click", () => choose(option));
    group.append(radio);
    return radio;
  });
  group.addEventListener("keydown", (event) => {
    const step = { ArrowRight: 1, ArrowDown: 1, ArrowLeft: -1, ArrowUp: -1 }[event.key];
    if (step === undefined) return;
    event.preventDefault();
    const current = radios.findIndex((radio) => radio.getAttribute("aria-checked") === "true");
    const next = radios[(current + step + radios.length) % radios.length];
    next.focus();
    next.click();
  });
  return (selected) => {
    for (const radio of radios) {
      const checked = radio.dataset.value === selected;
      radio.setAttribute("aria-checked", String(checked));
      radio.classList.toggle("sel", checked);
      radio.tabIndex = checked ? 0 : -1;
    }
  };
}

const selectStyle = segmented(
  stylePicker,
  BUTTON_STYLES,
  (style) => strings.buttonStyles[style],
  (style) => {
    // Swap at once; the setting follows (design system: Button style).
    setButtonStyle(power, style);
    if (settings) settings.buttonStyle = style;
    selectStyle(style);
    renderStatus();
    void update({ buttonStyle: style });
  },
);

/** "system" follows the OS through `prefers-color-scheme` (tokens.css). */
function applyTheme(theme: Theme): void {
  const root = document.documentElement;
  if (theme === "system") delete root.dataset.theme;
  else root.dataset.theme = theme;
  // The native title bar follows too; null means the OS theme.
  void getCurrentWindow()
    .setTheme(theme === "system" ? null : theme)
    .catch(() => {});
}

const selectTheme = segmented(
  element("theme-picker"),
  THEMES,
  (theme) => strings.themes[theme],
  (theme) => {
    applyTheme(theme);
    selectTheme(theme);
    void update({ theme });
  },
);

const selectIcon = segmented(
  iconPicker,
  APP_ICONS,
  (icon) => strings.appIcons[icon],
  (icon) => {
    if (settings) settings.appIcon = icon;
    selectIcon(icon);
    void update({ appIcon: icon });
  },
);

const methods: readonly NudgeMethod[] = ["pointer", "keystroke", "both"];
const selectMethod = segmented(
  methodPicker,
  methods,
  (method) => strings.nudgeMethods[method],
  (method) => void update({ nudgeMethod: method }),
);

function renderShortcut(): void {
  shortcutRecord.textContent = recording
    ? strings.shortcutRecording
    : (settings?.globalShortcut ?? strings.shortcutNone);
  shortcutRecord.setAttribute("aria-pressed", String(recording));
  shortcutClear.hidden = recording || !settings?.globalShortcut;
}

function renderSettings(): void {
  if (!settings) return;
  setButtonStyle(power, buttonStyle(settings.buttonStyle));
  selectStyle(buttonStyle(settings.buttonStyle));
  const theme = THEMES.find((name) => name === settings?.theme) ?? "system";
  applyTheme(theme);
  selectTheme(theme);
  selectIcon(appIcon(settings.appIcon));
  selectMethod(settings.nudgeMethod);
  nudgeEnabled.checked = settings.nudgeEnabled;
  nudgeKey.value = settings.nudgeKey;
  nudgeInterval.value = String(settings.nudgeIntervalSecs);
  keepDisplayOn.checked = settings.keepDisplayOn;
  launchAtLogin.checked = settings.launchAtLogin;
  restoreLastState.checked = settings.restoreLastState;
  notifyOnChange.checked = settings.notifyOnChange;

  for (const row of settingsView.querySelectorAll<HTMLElement>("[data-needs-nudge]")) {
    const keyRow = row.hasAttribute("data-needs-key");
    const enabled = settings.nudgeEnabled && (!keyRow || settings.nudgeMethod !== "pointer");
    row.classList.toggle("is-disabled", !enabled);
    for (const control of row.querySelectorAll<HTMLInputElement | HTMLSelectElement | HTMLButtonElement>(
      "input, select, button",
    )) {
      control.disabled = !enabled;
    }
  }
  renderShortcut();
}

/** Sends a change; on failure, says why and shows what is actually kept. */
async function update(patch: Partial<Settings>): Promise<void> {
  try {
    settings = await invoke<Settings>("update_settings", { patch });
    showSettingsError(null);
  } catch (error) {
    showSettingsError(message(error));
  }
  renderSettings();
  renderStatus();
}

function onToggle(input: HTMLInputElement, field: keyof Settings): void {
  input.addEventListener("change", () => void update({ [field]: input.checked }));
}

onToggle(nudgeEnabled, "nudgeEnabled");
onToggle(keepDisplayOn, "keepDisplayOn");
onToggle(launchAtLogin, "launchAtLogin");
onToggle(restoreLastState, "restoreLastState");
onToggle(notifyOnChange, "notifyOnChange");

nudgeKey.addEventListener("change", () => void update({ nudgeKey: nudgeKey.value }));

nudgeInterval.addEventListener("change", () => {
  const secs = Number(nudgeInterval.value);
  if (!Number.isInteger(secs) || secs < MIN_INTERVAL || secs > MAX_INTERVAL) {
    showSettingsError(strings.intervalRange);
    renderSettings();
    return;
  }
  void update({ nudgeIntervalSecs: secs });
});

function stopRecording(): void {
  recording = false;
  window.removeEventListener("keydown", onShortcutKey, true);
  renderShortcut();
}

function onShortcutKey(event: KeyboardEvent): void {
  event.preventDefault();
  event.stopPropagation();
  if (event.key === "Escape") {
    stopRecording();
    return;
  }
  const result = shortcutFromEvent(event);
  if (result === "modifier-only") return;
  if (result === null) {
    showSettingsError(strings.shortcutNeedsModifier);
    return;
  }
  stopRecording();
  void update({ globalShortcut: result });
}

shortcutRecord.addEventListener("click", () => {
  if (recording) {
    stopRecording();
    return;
  }
  recording = true;
  showSettingsError(null);
  window.addEventListener("keydown", onShortcutKey, true);
  renderShortcut();
});
shortcutRecord.addEventListener("blur", () => {
  if (recording) stopRecording();
});
shortcutClear.addEventListener("click", () => void update({ globalShortcut: null }));

function showView(name: "main" | "settings"): void {
  const inSettings = name === "settings";
  mainView.hidden = inSettings;
  settingsView.hidden = !inSettings;
  back.hidden = !inSettings;
  openSettings.hidden = inSettings;
  // The main screen stays uncluttered: only the update link sits there.
  openWebsite.hidden = !inSettings;
  shell.classList.toggle("in-settings", inSettings);
  text("title", inSettings ? strings.settings : strings.appName);
  if (recording) stopRecording();
  showSettingsError(null);
  (inSettings ? back : power).focus();
}

openSettings.addEventListener("click", () => showView("settings"));
back.addEventListener("click", () => showView("main"));
checkUpdates.addEventListener("click", () => void invoke("open_link", { link: "releases" }));
openWebsite.addEventListener("click", () => void invoke("open_link", { link: "website" }));
window.addEventListener("keydown", (event) => {
  if (event.key === "Escape" && !settingsView.hidden && !recording) showView("main");
});

// ── Start ────────────────────────────────────────────────────────────────

text("title", strings.appName);
text("h-appearance", strings.appearance);
text("l-style", strings.buttonStyle);
text("l-theme", strings.theme);
text("l-icon", strings.appIcon);
text("d-icon", strings.appIconHint);
text("h-activity", strings.activity);
text("l-nudge", strings.nudgeEnabled);
text("d-nudge", strings.nudgeEnabledHint);
text("l-method", strings.nudgeMethod);
text("l-key", strings.nudgeKey);
text("l-interval", strings.nudgeInterval);
text("d-interval", strings.nudgeIntervalHint);
text("u-interval", strings.seconds);
text("h-system", strings.system);
text("l-display", strings.keepDisplayOn);
text("l-login", strings.launchAtLogin);
text("l-restore", strings.restoreLastState);
text("l-notify", strings.notifyOnChange);
text("l-shortcut", strings.shortcut);
text("d-shortcut", strings.shortcutHint);
shortcutClear.textContent = strings.shortcutClear;
text("l-until", strings.untilLabel);
text("until-start", strings.untilStart);
text("until-cancel", strings.untilCancel);
power.setAttribute("aria-label", strings.toggleLabel);
openSettings.setAttribute("aria-label", strings.openSettings);
back.setAttribute("aria-label", strings.back);
for (const [button, label] of [
  [checkUpdates, strings.checkUpdates],
  [openWebsite, strings.website],
] as const) {
  button.setAttribute("aria-label", label);
  button.title = label;
}
setButtonStyle(power, "halo");

const [loaded, keys, version] = await Promise.all([
  invoke<Settings>("get_settings"),
  invoke<string[]>("nudge_keys"),
  getVersion(),
]);
for (const key of keys) nudgeKey.add(new Option(key, key));
text("version", strings.version(version));
settings = loaded;
renderSettings();

// The tray and the global shortcut change the state while the window is open.
await listen<Status>("status-changed", (event) => {
  if (pending !== null) return;
  // A change made elsewhere makes the last failure old news.
  if (event.payload.on !== status?.on) toggleError = null;
  applyStatus(event.payload);
});

// "Until a time…" in the tray opens the window on the picker.
async function takeTimerPickerRequest(): Promise<void> {
  if (await invoke<boolean>("take_timer_picker_request")) {
    showView("main");
    showUntil(true);
  }
}
await listen("show-timer-picker", () => void takeTimerPickerRequest());
await takeTimerPickerRequest();

await refreshStatus();
setInterval(() => {
  if (pending !== null) return;
  // ON: the elapsed time. OFF with a change planned: "tomorrow" becomes today.
  if (status?.on) void refreshStatus();
  else if (status?.nextChange) renderDetail(status);
}, 1000);
