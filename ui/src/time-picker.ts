// A time field: the native input for typing, plus a popover to pick from. The
// OS picker can't follow the design, and WebKit on macOS and Linux has none.
// The input stays the source of truth, so callers only listen for "change".

import { strings } from "./strings";

const MINUTE_STEP = 5;
const GAP = 4;
const pad = (value: number) => String(value).padStart(2, "0");

/** "09" or "9 AM", as the OS locale writes hours. */
function hourLabel(hour: number): string {
  return new Date(2000, 0, 1, hour).toLocaleTimeString([], { hour: "numeric" });
}

let closeOpen: ((commit: boolean) => void) | null = null;

function column(
  label: string,
  values: readonly number[],
  name: (value: number) => string,
  selected: number,
  pick: (value: number) => void,
): HTMLElement {
  const list = document.createElement("div");
  list.className = "time-column";
  list.setAttribute("role", "listbox");
  list.setAttribute("aria-label", label);
  const select = (chosen: HTMLButtonElement) => {
    for (const other of list.children) {
      const on = other === chosen;
      other.setAttribute("aria-selected", String(on));
      (other as HTMLElement).tabIndex = on ? 0 : -1;
    }
  };
  for (const value of values) {
    const option = document.createElement("button");
    option.type = "button";
    option.className = "time-option";
    option.setAttribute("role", "option");
    option.textContent = name(value);
    option.addEventListener("click", () => {
      select(option);
      pick(value);
    });
    list.append(option);
    if (value === selected) select(option);
  }
  list.addEventListener("keydown", (event) => {
    const step = { ArrowDown: 1, ArrowUp: -1 }[event.key];
    if (step === undefined) return;
    event.preventDefault();
    const current = document.activeElement as HTMLElement;
    const next = (step > 0 ? current.nextElementSibling : current.previousElementSibling) as HTMLButtonElement | null;
    if (!next) return;
    next.focus();
    select(next);
    pick(values[[...list.children].indexOf(next)]);
  });
  return list;
}

/** Wraps `input`, which must be a time input, and returns the field to insert. */
export function timeField(input: HTMLInputElement): HTMLElement {
  const field = document.createElement("span");
  field.className = "time-field";
  const toggle = document.createElement("button");
  toggle.type = "button";
  toggle.className = "time-toggle";
  toggle.setAttribute("aria-label", strings.pickTime);
  toggle.setAttribute("aria-haspopup", "dialog");
  toggle.setAttribute("aria-expanded", "false");
  toggle.title = strings.pickTime;
  toggle.innerHTML =
    '<svg width="14" height="14" viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/></svg>';
  field.append(input, toggle);

  function open(): void {
    closeOpen?.(true);
    const original = input.value;
    const [hour, minute] = (original || "09:00").split(":").map(Number);
    let picked = { hour, minute };
    const apply = () => {
      input.value = `${pad(picked.hour)}:${pad(picked.minute)}`;
    };

    const panel = document.createElement("div");
    panel.className = "time-popover";
    panel.setAttribute("role", "dialog");
    panel.setAttribute("aria-label", input.getAttribute("aria-label") ?? strings.pickTime);

    const hours = column(strings.hours, [...Array(24).keys()], hourLabel, hour, (value) => {
      picked = { ...picked, hour: value };
      apply();
    });
    const steps = Array.from({ length: 60 / MINUTE_STEP }, (_, index) => index * MINUTE_STEP);
    // A typed 09:07 stays selectable instead of snapping to a step.
    const minutes = [...new Set([...steps, minute])].sort((a, b) => a - b);
    const minutesColumn = column(strings.minutes, minutes, pad, minute, (value) => {
      picked = { ...picked, minute: value };
      apply();
    });
    // Choosing a minute with the pointer is the last step.
    minutesColumn.addEventListener("click", (event) => {
      if ((event.target as HTMLElement).closest(".time-option")) close(true);
    });
    panel.append(hours, minutesColumn);
    document.body.append(panel);

    // Fixed, so the settings list's scrolling can't clip it. Above when there
    // is no room below.
    const anchor = field.getBoundingClientRect();
    const size = panel.getBoundingClientRect();
    const below = anchor.bottom + GAP + size.height <= window.innerHeight;
    panel.style.top = `${Math.max(GAP, below ? anchor.bottom + GAP : anchor.top - GAP - size.height)}px`;
    panel.style.left = `${Math.max(GAP, Math.min(anchor.left, window.innerWidth - size.width - GAP))}px`;
    for (const list of [hours, minutesColumn]) {
      const chosen = list.querySelector<HTMLElement>('[aria-selected="true"]');
      if (chosen) list.scrollTop = chosen.offsetTop - (list.clientHeight - chosen.offsetHeight) / 2;
    }
    hours.querySelector<HTMLElement>('[aria-selected="true"]')?.focus();
    toggle.setAttribute("aria-expanded", "true");

    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        // Only the popover closes, not the form or view around it.
        event.preventDefault();
        event.stopPropagation();
        close(false);
      } else if (event.key === "Enter") {
        event.preventDefault();
        event.stopPropagation();
        close(true);
      } else if (event.key === "ArrowLeft" || event.key === "ArrowRight") {
        event.preventDefault();
        (event.key === "ArrowLeft" ? hours : minutesColumn).querySelector<HTMLElement>('[tabindex="0"]')?.focus();
      }
    };
    const onPointer = (event: PointerEvent) => {
      const target = event.target as Node;
      if (!panel.contains(target) && !toggle.contains(target)) close(true);
    };
    const onScroll = (event: Event) => {
      if (!panel.contains(event.target as Node)) close(true);
    };
    const onFocus = (event: FocusEvent) => {
      if (event.relatedTarget && !panel.contains(event.relatedTarget as Node)) close(true);
    };
    const onResize = () => close(true);
    document.addEventListener("keydown", onKey, true);
    document.addEventListener("pointerdown", onPointer, true);
    window.addEventListener("scroll", onScroll, true);
    window.addEventListener("resize", onResize);
    panel.addEventListener("focusout", onFocus);

    function close(commit: boolean): void {
      closeOpen = null;
      document.removeEventListener("keydown", onKey, true);
      document.removeEventListener("pointerdown", onPointer, true);
      window.removeEventListener("scroll", onScroll, true);
      window.removeEventListener("resize", onResize);
      panel.removeEventListener("focusout", onFocus);
      // Focus inside the panel would fall to the body when it goes.
      if (panel.contains(document.activeElement)) toggle.focus();
      panel.remove();
      toggle.setAttribute("aria-expanded", "false");
      if (!commit) input.value = original;
      // Last: a schedule row redraws itself on "change".
      else if (input.value !== original) input.dispatchEvent(new Event("change", { bubbles: true }));
    }
    closeOpen = close;
  }

  toggle.addEventListener("click", () => {
    if (toggle.getAttribute("aria-expanded") === "true") closeOpen?.(true);
    else open();
  });
  return field;
}
