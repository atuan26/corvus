import { invoke } from "@tauri-apps/api/core";

export interface Language {
  code: string;
  name: string;
  native: string;
}

export interface ProviderInfo {
  id: string;
  name: string;
  kind: string;
}

export interface Translation {
  text: string;
  detected_source: string | null;
  provider: string;
}

export interface UiError {
  message: string;
  attempts: { provider: string; error: string }[];
}

export interface AppInfo {
  default_source: string;
  default_target: string;
  config_path: string | null;
  warnings: string[];
}

export interface Prefs {
  source: string;
  target: string;
  provider: string | null;
}

export const api = {
  translate: (text: string, source: string, target: string, provider: string | null) =>
    invoke<Translation>("translate", { request: { text, source, target }, provider }),
  providers: () => invoke<ProviderInfo[]>("providers"),
  languages: () => invoke<Language[]>("languages"),
  appInfo: () => invoke<AppInfo>("app_info"),
  getPrefs: () => invoke<Prefs>("get_prefs"),
  setPrefs: (prefs: Prefs) => invoke<void>("set_prefs", { prefs }),
  popupText: () => invoke<string | null>("popup_text"),
  openInMain: (text: string) => invoke<void>("open_in_main", { text }),
  hidePopup: () => invoke<void>("hide_popup"),
};

const PREFS_KEY = "corvus.prefs";

/** Last-used languages/provider, remembered per machine. Storage may be unavailable. */
export function loadStoredPrefs(): Partial<Prefs> {
  try {
    return JSON.parse(localStorage.getItem(PREFS_KEY) ?? "{}") as Partial<Prefs>;
  } catch {
    return {};
  }
}

export function storePrefs(prefs: Prefs) {
  try {
    localStorage.setItem(PREFS_KEY, JSON.stringify(prefs));
  } catch {
    // Not persisting is fine; the backend still holds the session's prefs.
  }
}

export function asUiError(e: unknown): UiError {
  if (e && typeof e === "object" && "message" in e) {
    const err = e as Partial<UiError>;
    return { message: String(err.message), attempts: err.attempts ?? [] };
  }
  return { message: String(e), attempts: [] };
}
