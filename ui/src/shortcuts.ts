export type ActionId =
  | "translate"
  | "swap"
  | "copy"
  | "clear"
  | "focusSource"
  | "focusSourceLang"
  | "focusTargetLang"
  | "help"
  | "settings"
  | "tabTranslate"
  | "tabAsk";

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
  { action: "settings", keys: "Ctrl+,", label: "Open settings" },
  { action: "tabTranslate", keys: "Ctrl+1", label: "Translate tab" },
  { action: "tabAsk", keys: "Ctrl+2", label: "Ask AI tab (Enter sends, Esc stops, Ctrl+N new chat)" },
];

export const PROVIDER_SHORTCUT = { keys: "Alt+1 … Alt+9", label: "Use provider 1–9 for this translation" };

/** Labels for the configurable global shortcuts, keyed like the backend's `[shortcuts]`. */
export const GLOBAL_LABELS = {
  show_window: "Show Corvus from anywhere",
  translate_selection: "Translate selected text in any app",
  replace_selection: "Replace selected text with its translation",
  ask_selection: "Ask AI about selected text (1–6 quick actions, Ctrl+R replaces)",
  ocr_translate: "Translate text in a screen area",
  ocr_copy: "Copy text from a screen area",
} as const;

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

const takenEscapes = new WeakSet<Event>();

// Capture phase runs before React Aria handles the key, i.e. while the dropdown is still open.
window.addEventListener(
  "keydown",
  (e) => {
    if (e.key === "Escape" && document.querySelector(".popover, [data-recording]")) takenEscapes.add(e);
  },
  true,
);

/**
 * Whether this Esc press belongs to an open dropdown or the shortcut recorder rather than the
 * window. React Aria closes popovers on Esc without marking the event handled, so window-level
 * Esc handlers must check this or one press would close both.
 */
export function isEscapeTaken(e: KeyboardEvent): boolean {
  return takenEscapes.has(e);
}
