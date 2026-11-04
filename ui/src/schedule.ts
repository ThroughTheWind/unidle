// The Schedule group of the settings screen (SPEC SC-1, SC-2): weekly time
// windows and days off. Every change sends the whole list; the Rust side
// validates it and the screen redraws from what was kept.

import { strings } from "./strings";
import { timeField } from "./time-picker";

/** Mirrors `Weekdays::NAMES` in crates/core/src/schedule.rs. */
export const WEEKDAYS = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"] as const;
export type Weekday = (typeof WEEKDAYS)[number];

export interface TimeWindow {
  days: Weekday[];
  /** "HH:MM". An end at or before the start ends the next day. */
  start: string;
  end: string;
}

export interface ScheduleSettings {
  scheduleEnabled: boolean;
  scheduleWindows: TimeWindow[];
  /** "YYYY-MM-DD", sorted. */
  skipDates: string[];
}

/** What a new schedule starts with: office hours. */
const FIRST_WINDOW: TimeWindow = { days: ["mon", "tue", "wed", "thu", "fri"], start: "09:00", end: "18:00" };

type Update = (patch: Partial<ScheduleSettings>) => Promise<void>;
type ShowError = (line: string | null) => void;

function element<T extends HTMLElement>(id: string): T {
  const found = document.getElementById(id);
  if (!found) throw new Error(`Missing element #${id}`);
  return found as T;
}

/** "Thu, Dec 25, 2026", from "2026-12-25", in local time. */
function formatDate(date: string): string {
  const [year, month, day] = date.split("-").map(Number);
  return new Date(year, month - 1, day).toLocaleDateString([], {
    weekday: "short",
    month: "short",
    day: "numeric",
    year: "numeric",
  });
}

function iconButton(label: string, onClick: () => void): HTMLButtonElement {
  const button = document.createElement("button");
  button.type = "button";
  button.className = "as-ibtn remove";
  button.setAttribute("aria-label", label);
  button.title = label;
  button.innerHTML =
    '<span><svg width="16" height="16" viewBox="0 0 24 24" aria-hidden="true"><path d="M6 6l12 12M18 6L6 18"/></svg></span>';
  button.addEventListener("click", onClick);
  return button;
}

/**
 * Wires the Schedule group. Returns the function that redraws it from the
 * current settings.
 */
export function scheduleEditor(
  getSettings: () => ScheduleSettings | null,
  update: Update,
  showError: ShowError,
): () => void {
  const enabled = element<HTMLInputElement>("schedule-enabled");
  const body = element("schedule-body");
  const list = element<HTMLUListElement>("schedule-windows");
  const add = element<HTMLButtonElement>("add-window");
  const skipList = element<HTMLUListElement>("skip-dates");
  const skipForm = element<HTMLFormElement>("skip-form");
  const skipDate = element<HTMLInputElement>("skip-date");

  element("h-schedule").textContent = strings.schedule;
  element("l-schedule").textContent = strings.scheduleEnabled;
  element("d-schedule").textContent = strings.scheduleEnabledHint;
  element("l-skip").textContent = strings.skipDates;
  element("d-skip").textContent = strings.skipDatesHint;
  element("skip-add").textContent = strings.addSkipDate;
  add.textContent = `+ ${strings.addWindow}`;

  const windows = () => getSettings()?.scheduleWindows ?? [];
  const setWindows = (next: TimeWindow[]) => update({ scheduleWindows: next });

  enabled.addEventListener("change", () => {
    // Turning it on with nothing in it starts from office hours.
    const patch: Partial<ScheduleSettings> = { scheduleEnabled: enabled.checked };
    if (enabled.checked && windows().length === 0) patch.scheduleWindows = [FIRST_WINDOW];
    void update(patch);
  });

  add.addEventListener("click", async () => {
    const last = windows().at(-1);
    await setWindows([...windows(), last ? { ...last, days: [...last.days] } : FIRST_WINDOW]);
    list.querySelector<HTMLElement>("li:last-child .day")?.focus();
  });

  skipForm.addEventListener("submit", (event) => {
    event.preventDefault();
    const date = skipDate.value;
    if (!date) return;
    const dates = getSettings()?.skipDates ?? [];
    skipDate.value = "";
    if (!dates.includes(date)) void update({ skipDates: [...dates, date].sort() });
  });

  function windowRow(window: TimeWindow, index: number): HTMLLIElement {
    const row = document.createElement("li");
    row.className = "window";

    const days = document.createElement("div");
    days.className = "days";
    days.setAttribute("role", "group");
    WEEKDAYS.forEach((day, dayIndex) => {
      const button = document.createElement("button");
      button.type = "button";
      button.className = "day";
      const { short, name } = strings.weekdays[dayIndex];
      button.textContent = short;
      button.setAttribute("aria-label", name);
      button.title = name;
      const on = window.days.includes(day);
      button.setAttribute("aria-pressed", String(on));
      button.addEventListener("click", () => {
        const next = on ? window.days.filter((d) => d !== day) : WEEKDAYS.filter((d) => d === day || window.days.includes(d));
        if (next.length === 0) {
          showError(strings.windowNeedsDay);
          return;
        }
        const all = windows().map((w, i) => (i === index ? { ...w, days: next } : w));
        void setWindows(all).then(() => list.querySelectorAll<HTMLElement>(`li:nth-child(${index + 1}) .day`)[dayIndex]?.focus());
      });
      days.append(button);
    });

    const times = document.createElement("div");
    times.className = "times";
    const time = (value: string, label: string, field: "start" | "end") => {
      const input = document.createElement("input");
      input.type = "time";
      input.className = "input window-time";
      input.value = value;
      input.required = true;
      input.setAttribute("aria-label", label);
      input.addEventListener("change", () => {
        if (!input.value) return;
        const all = windows().map((w, i) => (i === index ? { ...w, [field]: input.value.slice(0, 5) } : w));
        void setWindows(all).then(() => {
          // The redraw replaced this row; keep the keyboard where it was.
          if (document.activeElement !== document.body) return;
          list.querySelector<HTMLElement>(`li:nth-child(${index + 1}) .time-toggle`)?.focus();
        });
      });
      return timeField(input);
    };
    const dash = document.createElement("span");
    dash.className = "unit";
    dash.textContent = "–";
    dash.setAttribute("aria-hidden", "true");
    times.append(time(window.start, strings.windowFrom, "start"), dash, time(window.end, strings.windowTo, "end"));
    times.append(
      iconButton(strings.removeWindow, () => void setWindows(windows().filter((_, i) => i !== index))),
    );

    row.append(days, times);
    if (window.end <= window.start) {
      const hint = document.createElement("span");
      hint.className = "row-hint";
      hint.textContent = strings.windowOvernight;
      row.append(hint);
    }
    return row;
  }

  return () => {
    const settings = getSettings();
    if (!settings) return;
    enabled.checked = settings.scheduleEnabled;
    body.classList.toggle("is-disabled", !settings.scheduleEnabled);

    list.replaceChildren(...settings.scheduleWindows.map(windowRow));
    if (settings.scheduleWindows.length === 0) {
      const empty = document.createElement("li");
      empty.className = "row-hint empty";
      empty.textContent = strings.noWindows;
      list.append(empty);
    }

    skipList.replaceChildren(
      ...settings.skipDates.map((date) => {
        const item = document.createElement("li");
        item.className = "skip-date";
        const label = document.createElement("span");
        label.textContent = formatDate(date);
        item.append(
          label,
          iconButton(strings.removeSkipDate(formatDate(date)), () => {
            void update({ skipDates: settings.skipDates.filter((d) => d !== date) });
          }),
        );
        return item;
      }),
    );

    // After the rebuild, so new rows follow the switch too.
    for (const control of body.querySelectorAll<HTMLInputElement | HTMLButtonElement>("input, button")) {
      control.disabled = !settings.scheduleEnabled;
    }
  };
}
