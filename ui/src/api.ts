import { Channel, invoke } from "@tauri-apps/api/core";

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
  details?: Details;
}

/** Dictionary extras for a word or short phrase. */
export interface Details {
  /** Google's respelling (English) or romanization (other scripts); not IPA. */
  source_phonetic?: string;
  target_phonetic?: string;
  /** Suggested spelling when the source looks mistyped. */
  corrected?: string;
  meanings: Meaning[];
  examples: string[];
}

export interface Meaning {
  pos: string;
  translations: { word: string; back: string[] }[];
  senses: { gloss: string; example?: string }[];
  synonyms: string[];
}

export interface Ipa {
  text: string;
  accent?: string;
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
  ask_selection: string;
  ocr_translate: string;
  ocr_copy: string;
}

export interface OcrConfig {
  languages: string[];
  tesseract?: string | null;
}

export type AssistantKind = "chatgpt" | "anthropic" | "openai" | "openai-compatible";

export interface AssistantConfig {
  id: string;
  kind: AssistantKind;
  enabled: boolean;
  name?: string | null;
  base_url?: string | null;
  model?: string | null;
  api_key_env?: string | null;
  web_search: boolean;
}

export interface AssistantView extends AssistantConfig {
  key_source: KeySource;
}

export interface AssistantStatus {
  id: string;
  name: string;
  kind: AssistantKind;
  model: string | null;
  web_search: boolean;
  /** Why it can't be used yet, e.g. "Not signed in to ChatGPT". */
  problem: string | null;
}

export interface ChatMessage {
  role: "user" | "assistant";
  text: string;
  raw?: unknown;
  provider?: string | null;
}

export interface Source {
  url: string;
  title: string | null;
}

export interface AskReply {
  message: ChatMessage;
  sources: Source[];
  usage: { input_tokens: number; output_tokens: number } | null;
}

export type AskEvent =
  | { type: "text"; delta: string }
  | { type: "searching"; query: string }
  | { type: "source"; source: Source };

export type StreamMessage =
  | { type: "event"; event: AskEvent }
  | { type: "done"; reply: AskReply }
  | { type: "error"; message: string; needs_sign_in: boolean };

export interface AskRequest {
  system?: string | null;
  messages: ChatMessage[];
  web_search: boolean;
}

export interface ChatGptStatus {
  signed_in: boolean;
  email: string | null;
  plan: string | null;
  storage: "keychain" | "file";
}

export interface DeviceCode {
  verification_url: string;
  user_code: string;
}

export interface ModelEntry {
  slug: string;
  name: string;
}

export interface SettingsView {
  default_source: string;
  default_target: string;
  timeout_secs: number;
  shortcuts: ShortcutKeys;
  ocr: OcrConfig;
  providers: ProviderView[];
  assistants: AssistantView[];
  secrets_location: "keychain" | "file";
  config_path: string | null;
  keychain_error: string | null;
}

export interface SettingsInput {
  default_source: string;
  default_target: string;
  timeout_secs: number;
  shortcuts: ShortcutKeys;
  ocr: OcrConfig;
  providers: ProviderConfig[];
  assistants: AssistantConfig[];
}

export const api = {
  translate: (text: string, source: string, target: string, provider: string | null) =>
    invoke<Translation>("translate", { request: { text, source, target }, provider }),
  ipa: (word: string, lang: string) => invoke<Ipa[]>("ipa", { word, lang }),
  /** MP3 bytes of `text` read aloud. */
  speak: (text: string, lang: string) => invoke<ArrayBuffer>("speak", { text, lang }),
  providers: () => invoke<ProviderInfo[]>("providers"),
  languages: () => invoke<Language[]>("languages"),
  appInfo: () => invoke<AppInfo>("app_info"),
  getPrefs: () => invoke<Prefs>("get_prefs"),
  setPrefs: (prefs: Prefs) => invoke<void>("set_prefs", { prefs }),
  popupText: () => invoke<string | null>("popup_text"),
  openInMain: (text: string, view?: "translate" | "ask") => invoke<void>("open_in_main", { text, view }),
  pasteText: (text: string) => invoke<void>("paste_text", { text }),
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
  listAssistants: () => invoke<AssistantStatus[]>("list_assistants"),
  askStart: (requestId: string, assistantId: string, request: AskRequest, channel: Channel<StreamMessage>) =>
    invoke<void>("ask_start", { requestId, assistantId, request, channel }),
  askCancel: (requestId: string) => invoke<void>("ask_cancel", { requestId }),
  chatgptStatus: () => invoke<ChatGptStatus>("chatgpt_status"),
  chatgptSignIn: () => invoke<ChatGptStatus>("chatgpt_sign_in"),
  chatgptDeviceStart: () => invoke<DeviceCode>("chatgpt_device_start"),
  chatgptDeviceFinish: () => invoke<ChatGptStatus>("chatgpt_device_finish"),
  chatgptCancelSignIn: () => invoke<void>("chatgpt_cancel_sign_in"),
  chatgptSignOut: () => invoke<void>("chatgpt_sign_out"),
  chatgptModels: () => invoke<ModelEntry[]>("chatgpt_models"),
  setAssistantKey: (assistantId: string, key: string | null) => invoke<void>("set_assistant_key", { assistantId, key }),
  openUrl: (url: string) => invoke<void>("open_url", { url }),
  ocrLanguages: () => invoke<string[]>("ocr_languages"),
  /** Pick a screen area; its text goes into the Translate or Ask input. */
  ocrStart: (tab: "translate" | "ask") => invoke<void>("ocr_start", { tab }),
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
