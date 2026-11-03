import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";

import { strings } from "./strings";

interface Status {
  on: boolean;
  elapsedSecs: number;
  powerAssertion: boolean;
  inputNudge: boolean;
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
}

async function refresh(): Promise<void> {
  render(await invoke<Status>("get_status"));
}

toggle.addEventListener("click", async () => {
  const on = toggle.getAttribute("aria-checked") !== "true";
  render(await invoke<Status>("set_active", { on }));
});

element("app-name").textContent = strings.appName;
element("schedule").textContent = strings.scheduleSoon;
toggle.setAttribute("aria-label", strings.toggleLabel);
element("version").textContent = `v${await getVersion()}`;

await refresh();
setInterval(() => {
  if (toggle.getAttribute("aria-checked") === "true") void refresh();
}, 1000);
