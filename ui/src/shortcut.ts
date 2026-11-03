// Turns a key press into an accelerator string the global-shortcut plugin
// accepts, such as "Ctrl+Alt+A".

const NAMED: Record<string, string> = {
  Space: "Space",
  Enter: "Enter",
  Tab: "Tab",
  Backspace: "Backspace",
  Delete: "Delete",
  Insert: "Insert",
  Home: "Home",
  End: "End",
  PageUp: "PageUp",
  PageDown: "PageDown",
  ArrowUp: "Up",
  ArrowDown: "Down",
  ArrowLeft: "Left",
  ArrowRight: "Right",
  Minus: "-",
  Equal: "=",
  Comma: ",",
  Period: ".",
  Slash: "/",
  Semicolon: ";",
  Quote: "'",
  BracketLeft: "[",
  BracketRight: "]",
  Backslash: "\\",
  Backquote: "`",
};

const MODIFIER_CODES = /^(Shift|Control|Alt|Meta|OS)(Left|Right)?$/;

function keyName(code: string): string | null {
  const letter = /^Key([A-Z])$/.exec(code);
  if (letter) return letter[1];
  const digit = /^Digit([0-9])$/.exec(code);
  if (digit) return digit[1];
  if (/^F([1-9]|1[0-9]|2[0-4])$/.test(code)) return code;
  return NAMED[code] ?? null;
}

/**
 * The accelerator for this key press, `"modifier-only"` while only modifiers
 * are held, or null for a press that can't be a shortcut: no modifier, or a
 * key the plugin doesn't know. A modifier is required so the shortcut can't
 * swallow a key the user types.
 */
export function shortcutFromEvent(event: KeyboardEvent): string | "modifier-only" | null {
  if (MODIFIER_CODES.test(event.code)) return "modifier-only";
  const key = keyName(event.code);
  const modifiers = [
    event.ctrlKey && "Ctrl",
    event.altKey && "Alt",
    event.shiftKey && "Shift",
    event.metaKey && "Super",
  ].filter((part): part is string => typeof part === "string");
  if (key === null || modifiers.length === 0) return null;
  return [...modifiers, key].join("+");
}
