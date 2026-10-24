import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  Button,
  Label,
  SelectionIndicator,
  Tab,
  TabList,
  TabPanel,
  Tabs,
  TextArea,
  TextField,
} from "react-aria-components";
import { ArrowLeftRight, Check, Copy, Keyboard, Languages, MessageSquareText, ScanText, Settings2, X } from "lucide-react";
import { IconButton } from "./components/IconButton";
import { Logo } from "./components/Logo";
import AskView from "./ask/AskView";
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
import { WordDetails } from "./components/WordDetails";
import Settings from "./Settings";
import { matchProviderDigit, matchShortcut } from "./shortcuts";

const AUTO_TRANSLATE_DELAY_MS = 600;

type Tab = "translate" | "ask";
type View = Tab | "settings";

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
  // The input `result` belongs to; `text` moves on while the next translation is pending.
  const [resultFor, setResultFor] = useState("");
  const [error, setError] = useState<UiError | null>(null);
  const [busy, setBusy] = useState(false);
  const [status, setStatusText] = useState("");
  // Quiet messages are for screen readers only, when the screen already shows the same thing.
  const [statusQuiet, setStatusQuiet] = useState(false);
  const setStatus = useCallback((text: string) => {
    setStatusQuiet(false);
    setStatusText(text);
  }, []);
  const announce = useCallback((text: string) => {
    setStatusQuiet(true);
    setStatusText(text);
  }, []);
  const [helpOpen, setHelpOpen] = useState(false);
  const [ready, setReady] = useState(false);
  const [view, setView] = useState<View>("translate");
  // Where Settings returns to, and text handed to the Ask tab from a popup.
  const [lastTab, setLastTab] = useState<Tab>("translate");
  const [askText, setAskText] = useState<string | undefined>(undefined);
  const [copied, setCopied] = useState(false);
  const resultRef = useRef<HTMLTextAreaElement>(null);
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
        if (e.payload) setStatus("Text recognized and copied");
        setText(e.payload);
        requestAnimationFrame(() => sourceTextRef.current?.focus());
      }),
      listen("open-settings", () => setView("settings")),
      listen<string>("status-note", (e) => setStatus(e.payload)),
      listen<string>("load-ask", (e) => {
        setView("ask");
        // A fresh string each time so the Ask tab picks it up even if unchanged.
        setAskText(e.payload ? `${e.payload}\n\n` : " ");
      }),
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
    setView(lastTab);
    if (lastTab === "translate") requestAnimationFrame(() => sourceTextRef.current?.focus());
  }, [lastTab]);

  const openSettings = useCallback(() => {
    setView((v) => {
      if (v !== "settings") setLastTab(v);
      return "settings";
    });
  }, []);

  // Share the current choice with the popup and replace-selection, and remember it.
  useEffect(() => {
    if (!ready) return;
    const prefs = { source, target, provider };
    storePrefs(prefs);
    void api.setPrefs(prefs);
  }, [ready, source, target, provider]);

  // A new translation fades in, so the change is noticeable without moving anything.
  useEffect(() => {
    if (!result?.text || window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    resultRef.current?.animate([{ opacity: 0.25 }, { opacity: 1 }], { duration: 220, easing: "ease-out" });
  }, [result?.text]);

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
        setResultFor(text);
        setError(null);
        const by = providers.find((p) => p.id === t.provider)?.name ?? t.provider;
        const from = source === "auto" && t.detected_source ? `from ${languageName(t.detected_source)} ` : "";
        const fellBack = (providerOverride ?? provider) !== t.provider ? " (fallback)" : "";
        announce(`Translated ${from}by ${by}${fellBack}`);
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
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1500);
    } catch {
      setStatus("Could not copy to clipboard");
    }
  }, [result]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const global = matchShortcut(e);
      if (global === "help") {
        e.preventDefault();
        setHelpOpen(true);
        return;
      }
      if (view === "settings") return;
      if (global === "tabTranslate" || global === "tabAsk" || global === "settings") {
        e.preventDefault();
        if (global === "settings") openSettings();
        else setView(global === "tabAsk" ? "ask" : "translate");
        return;
      }
      if (view !== "translate") return;
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
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [view, providers, translate, swap, copy, openSettings]);

  const warningList = warnings.length > 0 && (
    <ul className="warnings" aria-label="Configuration warnings">
      {warnings.map((w) => (
        <li key={w}>{w}</li>
      ))}
    </ul>
  );
  const helpButton = (
    <IconButton label="Keyboard shortcuts" shortcut="Ctrl+/" icon={<Keyboard />} onPress={() => setHelpOpen(true)} />
  );
  const providerName = providers.find((p) => p.id === result?.provider)?.name;

  return (
    <div className="app">
      {view === "settings" ? (
        <>
          <header className="toolbar">
            <h1 className="brand">
              <Logo />
            </h1>
            <span className="toolbar-spacer" />
            {helpButton}
          </header>
          <Settings languages={languages} onClose={closeSettings} />
        </>
      ) : (
        <Tabs className="tabs" selectedKey={view} onSelectionChange={(k) => setView(k as Tab)}>
          <header className="toolbar">
            <h1 className="brand">
              <Logo />
            </h1>
            <TabList aria-label="Mode" className="tablist">
              <Tab id="translate" className="tab">
                <SelectionIndicator className="tab-indicator" />
                <span className="tab-label">
                  <Languages aria-hidden /> Translate
                </span>
                <kbd aria-hidden>Ctrl+1</kbd>
              </Tab>
              <Tab id="ask" className="tab">
                <SelectionIndicator className="tab-indicator" />
                <span className="tab-label">
                  <MessageSquareText aria-hidden /> Ask AI
                </span>
                <kbd aria-hidden>Ctrl+2</kbd>
              </Tab>
            </TabList>
            <span className="toolbar-spacer" />
            {view === "translate" && <ProviderSelect providers={providers} value={provider} onChange={setProvider} />}
            {helpButton}
            <IconButton label="Settings" shortcut="Ctrl+," icon={<Settings2 />} onPress={openSettings} />
          </header>
          {/* Settings shows the same messages in its own footer. */}
          {warningList}

          <TabPanel id="translate" className="tabpanel">
            <div className="lang-bar">
              <LanguagePicker
                label="Source language"
                languages={languages}
                value={source}
                onChange={setSource}
                inputRef={sourceLangRef}
              />
              <IconButton label="Swap languages" shortcut="Ctrl+R" icon={<ArrowLeftRight />} onPress={swap} />
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
                <TextArea ref={sourceTextRef} className="pane-text" placeholder="Type, paste, or read text from the screen…" spellCheck />
                <div className="pane-footer">
                  <div className="pane-footer-start">
                    <span>{text.length.toLocaleString()} characters</span>
                  </div>
                  <div className="pane-footer-end">
                    <IconButton
                      className="quiet"
                      label="Read text from screen"
                      icon={<ScanText />}
                      onPress={() => void api.ocrStart("translate")}
                    />
                    <IconButton
                      className="quiet"
                      label="Clear"
                      shortcut="Ctrl+Shift+X"
                      icon={<X />}
                      isDisabled={!text}
                      onPress={() => {
                        setText("");
                        sourceTextRef.current?.focus();
                      }}
                    />
                    <Button className="btn btn-primary" onPress={() => void translate()} isDisabled={!text.trim()}>
                      Translate <kbd aria-hidden>Ctrl+Enter</kbd>
                    </Button>
                  </div>
                </div>
              </TextField>

              <TextField className="pane pane-result" value={result?.text ?? ""} isReadOnly aria-busy={busy}>
                <Label className="pane-label">Translation</Label>
                <TextArea ref={resultRef} className="pane-text" placeholder={busy ? "Translating…" : "The translation appears here"} />
                <div className="pane-footer">
                  <div className="pane-footer-start">
                    {result && source === "auto" && result.detected_source && <span className="chip">From {languageName(result.detected_source)}</span>}
                    {providerName && <span>{providerName}</span>}
                  </div>
                  <div className="pane-footer-end">
                    <Button className={`btn ${copied ? "copied" : ""}`} onPress={() => void copy()} isDisabled={!result}>
                      {copied ? <Check size={16} aria-hidden /> : <Copy size={16} aria-hidden />}
                      {copied ? "Copied" : "Copy"} <kbd aria-hidden>Ctrl+Shift+C</kbd>
                    </Button>
                  </div>
                </div>
              </TextField>
            </main>

            {result && (
              <WordDetails
                text={resultFor}
                source={source === "auto" ? result.detected_source : source}
                target={target}
                result={result}
                onUseCorrection={(t) => {
                  setText(t);
                  sourceTextRef.current?.focus();
                }}
              />
            )}

            <footer className="statusbar">
              <p role="status" aria-live="polite" className={statusQuiet ? "status sr-only" : "status"}>
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
          </TabPanel>
          {/* Kept mounted so the conversation survives switching tabs. */}
          <TabPanel id="ask" className="tabpanel" shouldForceMount>
            <AskView active={view === "ask"} onOpenSettings={openSettings} initialText={askText} />
          </TabPanel>
        </Tabs>
      )}

      <ShortcutsDialog isOpen={helpOpen} onOpenChange={setHelpOpen} globalKeys={globalKeys} />
    </div>
  );
}
