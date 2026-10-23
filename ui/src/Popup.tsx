import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { Button, Label, TextArea, TextField } from "react-aria-components";
import { api, asUiError, type Language, type Prefs, type ProviderInfo, type Translation, type UiError } from "./api";
import { LanguagePicker } from "./components/LanguagePicker";

/** Compact translation of the current selection. Keyboard: Esc close, Enter copy+close, Ctrl+O open. */
export default function Popup() {
  const [languages, setLanguages] = useState<Language[]>([]);
  const [providers, setProviders] = useState<ProviderInfo[]>([]);
  const [prefs, setPrefs] = useState<Prefs | null>(null);
  const [text, setText] = useState("");
  const [draft, setDraft] = useState("");
  const [result, setResult] = useState<Translation | null>(null);
  const [error, setError] = useState<UiError | null>(null);
  const [busy, setBusy] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  const draftRef = useRef<HTMLTextAreaElement>(null);
  const targetRef = useRef<HTMLInputElement>(null);
  const seq = useRef(0);

  const receive = useCallback((t: string) => {
    setText(t);
    setDraft("");
    setResult(null);
    setError(null);
    // Reshown windows keep stale focus; move it here so Esc/Enter work immediately.
    requestAnimationFrame(() => (t ? rootRef.current?.focus() : draftRef.current?.focus()));
  }, []);

  useEffect(() => {
    Promise.all([api.languages(), api.providers(), api.getPrefs(), api.popupText()])
      .then(([langs, provs, p, pending]) => {
        setLanguages(langs);
        setProviders(provs);
        setPrefs(p);
        receive(pending ?? "");
      })
      .catch((e) => setError(asUiError(e)));
    // Each time the popup is reshown the backend sends the new selection; prefs may have changed too.
    const unlisten = listen<string>("popup-text", (e) => {
      void api.getPrefs().then(setPrefs);
      receive(e.payload);
    });
    return () => void unlisten.then((f) => f());
  }, [receive]);

  useEffect(() => {
    if (!prefs || !text.trim()) return;
    const id = ++seq.current;
    setBusy(true);
    api
      .translate(text, prefs.source, prefs.target, prefs.provider)
      .then((t) => id === seq.current && (setResult(t), setError(null)))
      .catch((e) => id === seq.current && setError(asUiError(e)))
      .finally(() => id === seq.current && setBusy(false));
  }, [text, prefs]);

  const copyAndClose = useCallback(async () => {
    if (!result) return;
    try {
      await navigator.clipboard.writeText(result.text);
    } finally {
      void api.hidePopup();
    }
  }, [result]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const inField = e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement;
      if (e.key === "Escape" && !e.defaultPrevented) {
        void api.hidePopup();
      } else if (e.key === "Enter" && !inField && !e.shiftKey) {
        e.preventDefault();
        void copyAndClose();
      } else if (e.ctrlKey && e.code === "KeyO") {
        e.preventDefault();
        void api.openInMain(text || draft);
      } else if (e.altKey && e.code === "KeyT") {
        e.preventDefault();
        targetRef.current?.focus();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [copyAndClose, text, draft]);

  const languageName = (code?: string | null) =>
    languages.find((l) => l.code.toLowerCase() === code?.toLowerCase())?.name ?? code ?? "";
  const providerName = providers.find((p) => p.id === result?.provider)?.name ?? result?.provider;
  const from = prefs?.source === "auto" ? result?.detected_source : prefs?.source;

  return (
    <div ref={rootRef} className="popup" tabIndex={-1} role="dialog" aria-label="Translation">
      <div className="popup-bar">
        <span className="popup-from">{from ? languageName(from) : "Auto"} →</span>
        {prefs && (
          <LanguagePicker
            label="Target language"
            languages={languages.filter((l) => l.code !== "auto")}
            value={prefs.target}
            onChange={(target) => setPrefs({ ...prefs, target })}
            inputRef={targetRef}
          />
        )}
      </div>

      {text ? (
        <>
          <p className="popup-source" title={text}>
            {text}
          </p>
          <div className="popup-result" aria-live="polite" aria-busy={busy}>
            {error ? (
              <p role="alert" className="error">
                {error.message}
                {error.attempts.map((a) => (
                  <span key={a.provider}>
                    <br />
                    {a.provider}: {a.error}
                  </span>
                ))}
              </p>
            ) : (
              <p className="popup-translation">{result?.text ?? "Translating…"}</p>
            )}
          </div>
        </>
      ) : (
        <TextField className="popup-draft" value={draft} onChange={setDraft}>
          <Label className="pane-label">No text selected. Type text to translate</Label>
          <TextArea
            ref={draftRef}
            className="pane-text"
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey && draft.trim()) {
                e.preventDefault();
                setText(draft);
                rootRef.current?.focus();
              }
            }}
          />
        </TextField>
      )}

      <div className="popup-footer">
        <span className="popup-provider">{providerName}</span>
        <Button className="btn" onPress={() => void api.openInMain(text || draft)}>
          Open <kbd aria-hidden>Ctrl+O</kbd>
        </Button>
        <Button className="btn btn-primary" onPress={() => void copyAndClose()} isDisabled={!result}>
          Copy <kbd aria-hidden>Enter</kbd>
        </Button>
      </div>
    </div>
  );
}
