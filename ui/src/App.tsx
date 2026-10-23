import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { Button, Label, TextArea, TextField } from "react-aria-components";
import {
  api,
  asUiError,
  loadStoredPrefs,
  storePrefs,
  type Language,
  type ProviderInfo,
  type ShortcutKeys,
  type Translation,
  type UiError,
} from "./api";
import { LanguagePicker } from "./components/LanguagePicker";
import { ProviderSelect } from "./components/ProviderSelect";
import { ShortcutsDialog } from "./components/ShortcutsDialog";
import Settings from "./Settings";
import { matchProviderDigit, matchShortcut } from "./shortcuts";

const AUTO_TRANSLATE_DELAY_MS = 600;

/** Keyboard jumps select the field's text so typing replaces it, as Tab does. Doing this in an
 * onFocus handler instead races with the picker opening and eats the first typed character. */
function focusAndSelect(input: HTMLInputElement | null) {
  input?.focus();
  input?.select();
}

export default function App() {
  const [languages, setLanguages] = useState<Language[]>([]);
  const [providers, setProviders] = useState<ProviderInfo[]>([]);
  const [provider, setProvider] = useState<string | null>(null);
  const [warnings, setWarnings] = useState<string[]>([]);
  const [source, setSource] = useState("auto");
  const [target, setTarget] = useState("en");
  const [text, setText] = useState("");
  const [result, setResult] = useState<Translation | null>(null);
  const [error, setError] = useState<UiError | null>(null);
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState("");
  const [helpOpen, setHelpOpen] = useState(false);
  const [ready, setReady] = useState(false);
  const [view, setView] = useState<"translate" | "settings">("translate");
  const [globalKeys, setGlobalKeys] = useState<ShortcutKeys | null>(null);

  const sourceTextRef = useRef<HTMLTextAreaElement>(null);
  const sourceLangRef = useRef<HTMLInputElement>(null);
  const targetLangRef = useRef<HTMLInputElement>(null);
  // Responses can arrive out of order while typing; only the latest one is shown.
  const requestSeq = useRef(0);

  useEffect(() => {
    Promise.all([api.languages(), api.providers(), api.appInfo()])
      .then(([langs, provs, info]) => {
        const stored = loadStoredPrefs();
        const known = (code?: string) => (code && langs.some((l) => l.code === code) ? code : undefined);
        setLanguages(langs);
        setProviders(provs);
        setProvider(provs.find((p) => p.id === stored.provider)?.id ?? provs[0]?.id ?? null);
        setSource(known(stored.source) ?? info.default_source);
        setTarget(known(stored.target) ?? info.default_target);
        setWarnings(info.warnings);
        setGlobalKeys(info.shortcuts);
        setReady(true);
      })
      .catch((e) => setError(asUiError(e)));
    sourceTextRef.current?.focus();
    const unlisteners = [
      listen<string>("load-text", (e) => {
        setView("translate");
        setText(e.payload);
        requestAnimationFrame(() => sourceTextRef.current?.focus());
      }),
      listen("open-settings", () => setView("settings")),
      // Saved settings change the provider list, warnings and global keys.
      listen("settings-changed", () => {
        void Promise.all([api.providers(), api.appInfo()]).then(([provs, info]) => {
          setProviders(provs);
          setProvider((current) => (provs.some((p) => p.id === current) ? current : (provs[0]?.id ?? null)));
          setWarnings(info.warnings);
          setGlobalKeys(info.shortcuts);
        });
      }),
    ];
    return () => unlisteners.forEach((u) => void u.then((f) => f()));
  }, []);

  const closeSettings = useCallback(() => {
    setView("translate");
    requestAnimationFrame(() => sourceTextRef.current?.focus());
  }, []);

  // Share the current choice with the popup and replace-selection, and remember it.
  useEffect(() => {
    if (!ready) return;
    const prefs = { source, target, provider };
    storePrefs(prefs);
    void api.setPrefs(prefs);
  }, [ready, source, target, provider]);

  const languageName = useCallback(
    (code: string | null | undefined) =>
      languages.find((l) => l.code.toLowerCase() === code?.toLowerCase())?.name ?? code ?? "",
    [languages],
  );

  const translate = useCallback(
    async (providerOverride?: string) => {
      const seq = ++requestSeq.current;
      if (!text.trim()) {
        setResult(null);
        setError(null);
        setBusy(false);
        return;
      }
      setBusy(true);
      setStatus("Translating…");
      try {
        const t = await api.translate(text, source, target, providerOverride ?? provider);
        if (seq !== requestSeq.current) return;
        setResult(t);
        setError(null);
        const by = providers.find((p) => p.id === t.provider)?.name ?? t.provider;
        const from = source === "auto" && t.detected_source ? `from ${languageName(t.detected_source)} ` : "";
        const fellBack = (providerOverride ?? provider) !== t.provider ? " (fallback)" : "";
        setStatus(`Translated ${from}by ${by}${fellBack}`);
      } catch (e) {
        if (seq !== requestSeq.current) return;
        setError(asUiError(e));
        setStatus("");
      } finally {
        if (seq === requestSeq.current) setBusy(false);
      }
    },
    [text, source, target, provider, providers, languageName],
  );

  useEffect(() => {
    const id = window.setTimeout(() => void translate(), AUTO_TRANSLATE_DELAY_MS);
    return () => window.clearTimeout(id);
    // Re-run only when the inputs change, not when `translate` is recreated for other reasons.
  }, [text, source, target, provider]);

  const swap = useCallback(() => {
    const from = source === "auto" ? (result?.detected_source ?? null) : source;
    if (!from) {
      setStatus("Cannot swap until the source language is detected");
      return;
    }
    setSource(target);
    setTarget(from);
    if (result) setText(result.text);
  }, [source, target, result]);

  const copy = useCallback(async () => {
    if (!result) return;
    try {
      await navigator.clipboard.writeText(result.text);
      setStatus("Translation copied");
    } catch {
      setStatus("Could not copy to clipboard");
    }
  }, [result]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (view !== "translate") {
        if (matchShortcut(e) === "help") {
          e.preventDefault();
          setHelpOpen(true);
        }
        return;
      }
      const digit = matchProviderDigit(e);
      if (digit !== null && providers[digit]) {
        e.preventDefault();
        setProvider(providers[digit].id);
        return;
      }
      const action = matchShortcut(e);
      if (!action) return;
      e.preventDefault();
      switch (action) {
        case "translate":
          void translate();
          break;
        case "swap":
          swap();
          break;
        case "copy":
          void copy();
          break;
        case "clear":
          setText("");
          sourceTextRef.current?.focus();
          break;
        case "focusSource":
          sourceTextRef.current?.focus();
          break;
        case "focusSourceLang":
          focusAndSelect(sourceLangRef.current);
          break;
        case "focusTargetLang":
          focusAndSelect(targetLangRef.current);
          break;
        case "help":
          setHelpOpen(true);
          break;
        case "settings":
          setView("settings");
          break;
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [view, providers, translate, swap, copy]);

  return (
    <div className="app">
      <header className="toolbar">
        <h1 className="brand">Corvus</h1>
        {view === "translate" && <ProviderSelect providers={providers} value={provider} onChange={setProvider} />}
        <Button className="icon-btn" onPress={() => setHelpOpen(true)} aria-label="Keyboard shortcuts (Ctrl+/)">
          <span aria-hidden>?</span>
        </Button>
        {view === "translate" && (
          <Button className="icon-btn" onPress={() => setView("settings")} aria-label="Settings (Ctrl+,)">
            <span aria-hidden>⚙</span>
          </Button>
        )}
      </header>

      {/* Settings shows the same messages in its own footer. */}
      {view === "translate" && warnings.length > 0 && (
        <ul className="warnings" aria-label="Configuration warnings">
          {warnings.map((w) => (
            <li key={w}>{w}</li>
          ))}
        </ul>
      )}

      {view === "settings" ? (
        <Settings languages={languages} onClose={closeSettings} />
      ) : (
        <>
          <div className="lang-bar">
            <LanguagePicker
              label="Source language"
              languages={languages}
              value={source}
              onChange={setSource}
              inputRef={sourceLangRef}
            />
            <Button className="icon-btn" onPress={swap} aria-label="Swap languages (Ctrl+R)">
              <span aria-hidden>⇄</span>
            </Button>
            <LanguagePicker
              label="Target language"
              languages={languages.filter((l) => l.code !== "auto")}
              value={target}
              onChange={setTarget}
              inputRef={targetLangRef}
            />
          </div>

          <main className="panes">
            <TextField className="pane" value={text} onChange={setText}>
              <Label className="pane-label">Text to translate</Label>
              <TextArea ref={sourceTextRef} className="pane-text" placeholder="Type or paste text…" spellCheck />
              <div className="pane-footer">
                <span>{text.length.toLocaleString()} characters</span>
                <Button className="btn btn-primary" onPress={() => void translate()} isDisabled={!text.trim()}>
                  Translate <kbd aria-hidden>Ctrl+Enter</kbd>
                </Button>
              </div>
            </TextField>

            <TextField className="pane pane-result" value={result?.text ?? ""} isReadOnly aria-busy={busy}>
              <Label className="pane-label">Translation</Label>
              <TextArea className="pane-text" placeholder={busy ? "Translating…" : "Translation appears here"} />
              <div className="pane-footer">
                <span />
                <Button className="btn" onPress={() => void copy()} isDisabled={!result}>
                  Copy <kbd aria-hidden>Ctrl+Shift+C</kbd>
                </Button>
              </div>
            </TextField>
          </main>

          <footer className="statusbar">
            <p role="status" aria-live="polite" className="status">
              {status}
            </p>
            {error && (
              <div role="alert" className="error">
                <strong>{error.message}</strong>
                {error.attempts.length > 0 && (
                  <ul>
                    {error.attempts.map((a) => (
                      <li key={a.provider}>
                        {providers.find((p) => p.id === a.provider)?.name ?? a.provider}: {a.error}
                      </li>
                    ))}
                  </ul>
                )}
              </div>
            )}
          </footer>

        </>
      )}

      <ShortcutsDialog isOpen={helpOpen} onOpenChange={setHelpOpen} globalKeys={globalKeys} />
    </div>
  );
}
