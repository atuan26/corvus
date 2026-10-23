export type ActionId =
  | "translate"
  | "swap"
  | "copy"
  | "clear"
  | "focusSource"
  | "focusSourceLang"
  | "focusTargetLang"
  | "help";

export interface Shortcut {
  action: ActionId;
  keys: string;
  label: string;
}

/** In-window shortcuts. Shown in the help dialog and matched by `matchShortcut`. */
export const SHORTCUTS: Shortcut[] = [
  { action: "translate", keys: "Ctrl+Enter", label: "Translate now" },
  { action: "swap", keys: "Ctrl+R", label: "Swap languages" },
  { action: "copy", keys: "Ctrl+Shift+C", label: "Copy translation" },
  { action: "clear", keys: "Ctrl+Shift+X", label: "Clear text" },
  { action: "focusSource", keys: "Ctrl+L", label: "Focus source text" },
  { action: "focusSourceLang", keys: "Alt+S", label: "Choose source language" },
  { action: "focusTargetLang", keys: "Alt+T", label: "Choose target language" },
  { action: "help", keys: "Ctrl+/", label: "Show keyboard shortcuts" },
];

export const GLOBAL_SHORTCUTS: { keys: string; label: string }[] = [
  { keys: "Alt+1 … Alt+9", label: "Use provider 1–9 for this translation" },
  { keys: "Ctrl+Alt+C", label: "Show Corvus from anywhere" },
  { keys: "Ctrl+Alt+E", label: "Translate selected text in any app" },
  { keys: "Ctrl+Alt+R", label: "Replace selected text with its translation" },
];

function eventKeys(e: KeyboardEvent): string {
  const parts: string[] = [];
  if (e.ctrlKey || e.metaKey) parts.push("Ctrl");
  if (e.altKey) parts.push("Alt");
  if (e.shiftKey) parts.push("Shift");
  // Use the physical key for letters so Alt/Shift combos work on every layout.
  const key = e.code.startsWith("Key") ? e.code.slice(3) : e.key === "Enter" ? "Enter" : e.key;
  parts.push(key.length === 1 ? key.toUpperCase() : key);
  return parts.join("+");
}

export function matchShortcut(e: KeyboardEvent): ActionId | null {
  const keys = eventKeys(e);
  return SHORTCUTS.find((s) => s.keys === keys)?.action ?? null;
}

/** Alt+1..9 → provider index 0..8. */
export function matchProviderDigit(e: KeyboardEvent): number | null {
  if (!e.altKey || e.ctrlKey || e.metaKey || e.shiftKey) return null;
  const m = /^Digit([1-9])$/.exec(e.code);
  return m ? Number(m[1]) - 1 : null;
}
