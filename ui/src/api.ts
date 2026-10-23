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

export const api = {
  translate: (text: string, source: string, target: string, provider: string | null) =>
    invoke<Translation>("translate", { request: { text, source, target }, provider }),
  providers: () => invoke<ProviderInfo[]>("providers"),
  languages: () => invoke<Language[]>("languages"),
  appInfo: () => invoke<AppInfo>("app_info"),
};

export function asUiError(e: unknown): UiError {
  if (e && typeof e === "object" && "message" in e) {
    const err = e as Partial<UiError>;
    return { message: String(err.message), attempts: err.attempts ?? [] };
  }
  return { message: String(e), attempts: [] };
}
