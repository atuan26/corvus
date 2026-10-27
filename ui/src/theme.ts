import { listen } from "@tauri-apps/api/event";
import { api, type Appearance } from "./api";

/** Color themes; each has light and dark tokens in styles.css under `[data-theme]`. */
export const THEMES = [
  { id: "raven", label: "Raven", light: ["#edf0f5", "#4950c8", "#2fa49c"], dark: ["#0e1220", "#8f96ff", "#4fc1b8"] },
  { id: "moss", label: "Moss", light: ["#eef1ec", "#2f7d4f", "#c2a23a"], dark: ["#0f1510", "#6fcf97", "#d9bf5c"] },
  { id: "ocean", label: "Ocean", light: ["#ebf2f5", "#0e6f97", "#26b0a0"], dark: ["#0b141a", "#5cc3ec", "#4fd1c0"] },
  { id: "ember", label: "Ember", light: ["#f5f0ea", "#c2410c", "#d4a02a"], dark: ["#16110d", "#ff9a5c", "#e8c060"] },
  { id: "rose", label: "Rose", light: ["#f6eff2", "#be3a6e", "#8e6ad8"], dark: ["#170f13", "#ff86b2", "#b89cff"] },
  { id: "graphite", label: "Graphite", light: ["#f0f0f0", "#2b2b2b", "#8a8a8a"], dark: ["#111111", "#e6e6e6", "#7a7a7a"] },
] as const;

export const DEFAULT_APPEARANCE: Appearance = { mode: "system", theme: "raven" };

const CACHE_KEY = "corvus.appearance";
const darkQuery = window.matchMedia("(prefers-color-scheme: dark)");
let current = DEFAULT_APPEARANCE;

/** Whether dark colors are showing now. */
export function isDark(a: Appearance = current) {
  return a.mode === "dark" || (a.mode === "system" && darkQuery.matches);
}

export function applyAppearance(a: Appearance) {
  current = a;
  const root = document.documentElement;
  root.dataset.theme = THEMES.some((t) => t.id === a.theme) ? a.theme : DEFAULT_APPEARANCE.theme;
  root.dataset.scheme = isDark(a) ? "dark" : "light";
  try {
    localStorage.setItem(CACHE_KEY, JSON.stringify(a));
  } catch {
    // Only a startup-flash optimization.
  }
}

function cached(): Appearance {
  try {
    const a = JSON.parse(localStorage.getItem(CACHE_KEY) ?? "null");
    if (a && typeof a.mode === "string" && typeof a.theme === "string") return a;
  } catch {
    // Fall through to the default.
  }
  return DEFAULT_APPEARANCE;
}

/**
 * Called by every window before it renders: applies the last known colors at once, then the
 * saved ones, and follows later changes from Settings and from the desktop's dark mode.
 */
export function initTheme() {
  applyAppearance(cached());
  darkQuery.addEventListener("change", () => applyAppearance(current));
  void api.getAppearance().then(applyAppearance).catch(() => {});
  void listen<Appearance>("appearance-changed", (e) => applyAppearance(e.payload));
}
