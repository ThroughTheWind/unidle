import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import { strings } from "./strings";

interface Status {
  on: boolean;
  elapsedSecs: number;
  powerAssertion: boolean;
  nudging: boolean;
  inputUnavailable: "needsPermission" | "wayland" | "noDisplay" | "unsupported" | null;
}

function element(id: string): HTMLElement {
  const found = document.getElementById(id);
  if (!found) throw new Error(`Missing element #${id}`);
  return found;
}

const toggle = element("toggle");
const state = element("state");
const statusDot = element("status-dot");
const statusText = element("status-text");
const statusTimer = element("status-timer");
const notice = element("notice");
const noticeText = element("notice-text");
const noticeAction = element("notice-action");

function formatElapsed(totalSecs: number): string {
  const pad = (value: number) => String(value).padStart(2, "0");
  const hours = Math.floor(totalSecs / 3600);
  const minutes = Math.floor((totalSecs % 3600) / 60);
  return `${pad(hours)}:${pad(minutes)}:${pad(totalSecs % 60)}`;
}

function render(status: Status): void {
  toggle.setAttribute("aria-checked", String(status.on));
  toggle.classList.toggle("is-on", status.on);
  state.textContent = status.on ? strings.on : strings.off;
  state.classList.toggle("is-on", status.on);

  // Never claim to keep the machine awake unless the adapter can actually do it.
  const warning = status.on && !status.powerAssertion;
  statusDot.className = `status-dot ${warning ? "is-warning" : status.on ? "is-active" : ""}`;
  statusText.textContent = !status.on
    ? strings.stopped
    : warning
      ? strings.notImplemented
      : strings.keepingAwake;
  statusTimer.textContent = status.on ? formatElapsed(status.elapsedSecs) : "";

  // Power assertion only: say so, without blocking anything (SPEC PB-2).
  const fallback = status.on && status.powerAssertion && status.inputUnavailable !== null;
  notice.hidden = !fallback;
  noticeText.textContent = fallback ? strings.inputUnavailable : "";
  noticeAction.hidden = status.inputUnavailable !== "needsPermission";
}

async function refresh(): Promise<void> {
  render(await invoke<Status>("get_status"));
}

toggle.addEventListener("click", async () => {
  const on = toggle.getAttribute("aria-checked") !== "true";
  render(await invoke<Status>("set_active", { on }));
});

noticeAction.textContent = strings.openAccessibility;
noticeAction.addEventListener("click", () => {
  void invoke("open_input_permission_settings");
});

element("app-name").textContent = strings.appName;
element("schedule").textContent = strings.scheduleSoon;
toggle.setAttribute("aria-label", strings.toggleLabel);
element("version").textContent = `v${await getVersion()}`;

// The tray can change the state while the window is open.
await listen<Status>("status-changed", (event) => render(event.payload));

await refresh();
setInterval(() => {
  if (toggle.getAttribute("aria-checked") === "true") void refresh();
}, 1000);
