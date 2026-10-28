import { useCallback, useEffect, useState } from "react";
import {
  Button,
  Checkbox,
  CheckboxGroup,
  ComboBox,
  Input,
  Label,
  ListBox,
  ListBoxItem,
  Popover,
} from "react-aria-components";
import { X } from "lucide-react";
import { api, type OcrConfig, type OcrLanguage, type TesseractStatus } from "../api";
import { fold } from "./languageFilter";
import { IconButton } from "./IconButton";

interface Props {
  value: OcrConfig;
  onChange: (ocr: OcrConfig) => void;
}

export function OcrSettings({ value, onChange }: Props) {
  const [status, setStatus] = useState<TesseractStatus | null>(null);
  const [languages, setLanguages] = useState<OcrLanguage[] | null>(null);
  // What is running now, shown as progress; null when idle.
  const [busy, setBusy] = useState<string | null>(null);
  const [note, setNote] = useState("");
  const [error, setError] = useState("");
  const [query, setQuery] = useState("");

  const refresh = useCallback(async () => {
    const s = await api.tesseractStatus();
    setStatus(s);
    setLanguages(s.installed ? await api.ocrLanguages() : null);
  }, []);

  useEffect(() => {
    refresh().catch((e) => setError(String(e)));
  }, [refresh]);

  const run = async (label: string, task: () => Promise<string | void>) => {
    setBusy(label);
    setError("");
    setNote("");
    try {
      const done = await task();
      if (done) setNote(done);
      await refresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(null);
    }
  };

  if (!status) return <p className="setting-hint">{error || "Checking Tesseract…"}</p>;

  const messages = (
    <>
      {busy && (
        <p role="status" className="setting-hint">
          {busy}
        </p>
      )}
      {!busy && note && (
        <p role="status" className="setting-hint">
          {note}
        </p>
      )}
      {error && (
        <p role="alert" className="error">
          {error}
        </p>
      )}
    </>
  );

  if (!status.installed) {
    const method = status.install;
    const install = () =>
      run(
        method.kind === "windows-installer" ? "Downloading the Tesseract installer…" : "Installing Tesseract…",
        async () => {
          const ready = await api.tesseractInstall();
          return ready ? "Tesseract is installed." : "Finish the Tesseract installer, then press Check again.";
        },
      );
    return (
      <div className="ocr-install">
        <p className="notice" role="note">
          Screen text needs Tesseract, a free text-recognition program that runs on this computer.
        </p>
        {method.kind === "manual" ? (
          <>
            <p className="setting-hint">Install it, then press Check again:</p>
            {method.command && <code className="ocr-command">{method.command}</code>}
            <Button className="link-btn" onPress={() => void api.openUrl(method.url)}>
              Installation instructions
            </Button>
          </>
        ) : (
          <p className="setting-hint">
            {method.kind === "windows-installer"
              ? "Downloads the official installer (UB Mannheim build, checked before it runs). Windows asks for permission to install."
              : method.program.endsWith("brew")
                ? "Installs it with Homebrew. This can take a few minutes."
                : `Runs: ${method.args.join(" ")}. Your system asks for your password.`}
          </p>
        )}
        <div className="ocr-actions">
          {method.kind !== "manual" && (
            <Button className="btn btn-primary" onPress={() => void install()} isDisabled={!!busy}>
              Install Tesseract
            </Button>
          )}
          <Button className="btn" onPress={() => void run("Checking…", async () => {})} isDisabled={!!busy}>
            Check again
          </Button>
        </div>
        {messages}
      </div>
    );
  }

  const installed = (languages ?? []).filter((l) => l.installed);
  const available = (languages ?? []).filter((l) => !l.installed);
  const codes = installed.map((l) => l.code);
  // An empty list in the config means "all installed"; show that as everything checked.
  const selected = value.languages.length ? value.languages.filter((c) => codes.includes(c)) : codes;

  const download = (lang: OcrLanguage) =>
    run(`Downloading ${lang.name}…`, async () => {
      await api.ocrDownloadLanguage(lang.code);
      // With an explicit list, choose the new language too; "all" already includes it.
      if (value.languages.length) onChange({ ...value, languages: [...value.languages, lang.code] });
      return value.languages.length ? `${lang.name} added. Save to use it.` : `${lang.name} added.`;
    });
  const remove = (lang: OcrLanguage) =>
    run(`Removing ${lang.name}…`, async () => {
      await api.ocrRemoveLanguage(lang.code);
      if (value.languages.includes(lang.code)) {
        onChange({ ...value, languages: value.languages.filter((c) => c !== lang.code) });
      }
      return `${lang.name} removed.`;
    });

  return (
    <>
      {installed.length > 0 ? (
        <CheckboxGroup
          className="checkbox-group"
          value={selected}
          onChange={(langs) => onChange({ ...value, languages: langs.length === codes.length ? [] : langs })}
        >
          <Label className="setting-label">Languages to recognize</Label>
          {installed.map((l) => (
            <span key={l.code} className="ocr-lang">
              <Checkbox value={l.code} className="checkbox">
                <span className="checkbox-box" aria-hidden />
                {l.name} <span className="provider-id">({l.code})</span>
              </Checkbox>
              {l.downloaded && (
                <IconButton
                  className="quiet"
                  label={`Remove ${l.name}`}
                  icon={<X />}
                  onPress={() => void remove(l)}
                  isDisabled={!!busy}
                />
              )}
            </span>
          ))}
        </CheckboxGroup>
      ) : (
        <p className="notice" role="note">
          Tesseract is installed but has no languages yet. Add one below.
        </p>
      )}
      <ComboBox
        className="lang-picker ocr-add"
        items={available.filter((l) => !query || fold(l.name).includes(fold(query)) || l.code.startsWith(query.toLowerCase()))}
        inputValue={query}
        onInputChange={setQuery}
        selectedKey={null}
        onSelectionChange={(key) => {
          const lang = available.find((l) => l.code === key);
          setQuery("");
          if (lang) void download(lang);
        }}
        // Not on focus: after a download the field would reopen the list.
        menuTrigger="input"
        isDisabled={!!busy}
        allowsEmptyCollection
      >
        <Label className="setting-label">Add a language</Label>
        <div className="field">
          <Input className="field-input" placeholder="Search languages to download…" />
          <Button className="field-button" aria-label="Show languages to download">
            <span aria-hidden>▾</span>
          </Button>
        </div>
        <Popover className="popover">
          <ListBox<OcrLanguage> className="listbox" renderEmptyState={() => <div className="listbox-empty">No match</div>}>
            {(l) => (
              <ListBoxItem id={l.code} className="listbox-item" textValue={l.name}>
                <span>{l.name}</span>
                <span className="listbox-native">{l.code}</span>
              </ListBoxItem>
            )}
          </ListBox>
        </Popover>
      </ComboBox>
      {messages}
      <p className="setting-hint">
        Recognition runs locally with Tesseract. Fewer languages is faster and more accurate. Added languages are
        downloaded from Tesseract's official data (usually 1–4 MB each).
      </p>
    </>
  );
}
