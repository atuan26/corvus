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
  shortcuts: ShortcutKeys;
  warnings: string[];
}

export interface Prefs {
  source: string;
  target: string;
  provider: string | null;
}

export type ProviderKind = "google-free" | "deepl" | "openai-compatible";
export type KeySource = "config" | "keychain" | "env" | "none";
export type KeyStore = "keychain" | "config";

export interface ProviderConfig {
  id: string;
  kind: ProviderKind;
  enabled: boolean;
  name?: string | null;
  base_url?: string | null;
  model?: string | null;
  api_key_env?: string | null;
}

export interface ProviderView extends ProviderConfig {
  key_source: KeySource;
  requires_key: boolean;
}

export interface ShortcutKeys {
  show_window: string;
  translate_selection: string;
  replace_selection: string;
}

export interface SettingsView {
  default_source: string;
  default_target: string;
  timeout_secs: number;
  shortcuts: ShortcutKeys;
  providers: ProviderView[];
  config_path: string | null;
  keychain_error: string | null;
}

export interface SettingsInput {
  default_source: string;
  default_target: string;
  timeout_secs: number;
  shortcuts: ShortcutKeys;
  providers: ProviderConfig[];
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
  getSettings: () => invoke<SettingsView>("get_settings"),
  /** Resolves with warnings; rejects with a list of validation errors. */
  saveSettings: (input: SettingsInput) => invoke<string[]>("save_settings", { input }),
  setApiKey: (providerId: string, key: string | null, store: KeyStore) =>
    invoke<KeySource>("set_api_key", { providerId, key, store }),
  testProvider: (provider: ProviderConfig) => invoke<Translation>("test_provider", { provider }),
  validateShortcut: (keys: string) => invoke<void>("validate_shortcut", { keys }),
  pauseShortcuts: () => invoke<void>("pause_shortcuts"),
  resumeShortcuts: () => invoke<void>("resume_shortcuts"),
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
