// Formats the times of the source line (SPEC SC-4), the same way the tray does
// (src-tauri/src/strings.rs): 24-hour clock, and a day only when it isn't today.

import { strings } from "./strings";

const DAY_MS = 86_400_000;

function clock(date: Date): string {
  return date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", hourCycle: "h23" });
}

/** Whole local days from `now` to `date`: 0 today, 1 tomorrow. */
function daysFrom(now: Date, date: Date): number {
  const midnight = (day: Date) => new Date(day.getFullYear(), day.getMonth(), day.getDate()).getTime();
  return Math.round((midnight(date) - midnight(now)) / DAY_MS);
}

/** "at 18:00", "tomorrow at 09:00", "Mon at 09:00", "Nov 16 at 09:00". */
export function when(at: number, now = new Date()): string {
  const date = new Date(at);
  const time = clock(date);
  const days = daysFrom(now, date);
  if (days <= 0) return strings.today(time);
  if (days === 1) return strings.tomorrow(time);
  if (days <= 6) return strings.onDay(date.toLocaleDateString([], { weekday: "short" }), time);
  return strings.onDay(date.toLocaleDateString([], { month: "short", day: "numeric" }), time);
}

/** "18:00", or "tomorrow 18:00" when it isn't today. */
export function shortWhen(at: number, now = new Date()): string {
  const date = new Date(at);
  const days = daysFrom(now, date);
  if (days <= 0) return clock(date);
  if (days === 1) return strings.tomorrowShort(clock(date));
  return `${date.toLocaleDateString([], { weekday: "short" })} ${clock(date)}`;
}

/** The next full hour, as "HH:MM", to start the "until" picker from. */
export function nextHour(now = new Date()): string {
  return `${String((now.getHours() + 1) % 24).padStart(2, "0")}:00`;
}
