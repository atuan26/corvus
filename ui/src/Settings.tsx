import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  Button,
  Group,
  Input,
  Label,
  ListBox,
  ListBoxItem,
  NumberField,
  Popover,
  Select,
  SelectValue,
} from "react-aria-components";
import {
  api,
  type AssistantKind,
  type AssistantView,
  type Language,
  type ProviderConfig,
  type ProviderKind,
  type ProviderView,
  type SettingsInput,
  type SettingsView,
} from "./api";
import { LanguagePicker } from "./components/LanguagePicker";
import { AssistantCard } from "./components/AssistantCard";
import { OcrSettings } from "./components/OcrSettings";
import { ProviderCard } from "./components/ProviderCard";
import { ShortcutRecorder } from "./components/ShortcutRecorder";
import { isEscapeTaken } from "./shortcuts";

interface Preset {
  id: string;
  label: string;
  kind: ProviderKind;
  name?: string;
  base_url?: string;
}

const PRESETS: Preset[] = [
  { id: "google", label: "Google (free)", kind: "google-free" },
  { id: "deepl", label: "DeepL", kind: "deepl" },
  { id: "openai", label: "OpenAI", kind: "openai-compatible", name: "OpenAI", base_url: "https://api.openai.com/v1" },
  {
    id: "openrouter",
    label: "OpenRouter",
    kind: "openai-compatible",
    name: "OpenRouter",
    base_url: "https://openrouter.ai/api/v1",
  },
  { id: "groq", label: "Groq", kind: "openai-compatible", name: "Groq", base_url: "https://api.groq.com/openai/v1" },
  {
    id: "ollama",
    label: "Ollama (local)",
    kind: "openai-compatible",
    name: "Ollama",
    base_url: "http://localhost:11434/v1",
  },
  {
    id: "lmstudio",
    label: "LM Studio (local)",
    kind: "openai-compatible",
    name: "LM Studio",
    base_url: "http://localhost:1234/v1",
  },
  { id: "custom", label: "Other OpenAI-compatible", kind: "openai-compatible", base_url: "" },
];

type Draft = Omit<SettingsView, "config_path" | "keychain_error" | "secrets_location">;

interface AssistantPreset {
  id: string;
  label: string;
  kind: AssistantKind;
  name?: string;
  base_url?: string;
}

const ASSISTANT_PRESETS: AssistantPreset[] = [
  { id: "chatgpt", label: "ChatGPT (sign in)", kind: "chatgpt" },
  { id: "claude", label: "Claude (API key)", kind: "anthropic" },
  { id: "openai", label: "OpenAI (API key)", kind: "openai" },
  { id: "ollama", label: "Ollama (local)", kind: "openai-compatible", name: "Ollama", base_url: "http://localhost:11434/v1" },
  { id: "lmstudio", label: "LM Studio (local)", kind: "openai-compatible", name: "LM Studio", base_url: "http://localhost:1234/v1" },
  { id: "groq", label: "Groq", kind: "openai-compatible", name: "Groq", base_url: "https://api.groq.com/openai/v1" },
  { id: "custom-ai", label: "Other OpenAI-compatible", kind: "openai-compatible", base_url: "" },
];

function toInput(d: Draft): SettingsInput {
  const providers: ProviderConfig[] = d.providers.map(({ key_source: _k, requires_key: _r, ...p }) => ({
    ...p,
    name: p.name?.trim() || null,
    base_url: p.base_url?.trim() || null,
    model: p.model?.trim() || null,
  }));
  const assistants = d.assistants.map(({ key_source: _k, ...a }) => ({
    ...a,
    name: a.name?.trim() || null,
    base_url: a.base_url?.trim() || null,
    model: a.model?.trim() || null,
  }));
  const { default_source, default_target, timeout_secs, shortcuts, ocr } = d;
  return { default_source, default_target, timeout_secs, shortcuts, ocr, providers, assistants };
}

function uniqueId(base: string, taken: Set<string>) {
  if (!taken.has(base)) return base;
  for (let n = 2; ; n++) if (!taken.has(`${base}-${n}`)) return `${base}-${n}`;
}

interface Props {
  languages: Language[];
  onClose: () => void;
}

export default function Settings({ languages, onClose }: Props) {
  const [saved, setSaved] = useState<SettingsView | null>(null);
  const [draft, setDraft] = useState<Draft | null>(null);
  const [errors, setErrors] = useState<string[]>([]);
  const [status, setStatus] = useState("");
  const [saving, setSaving] = useState(false);
  const [confirmLeave, setConfirmLeave] = useState(false);
  const headingRef = useRef<HTMLHeadingElement>(null);

  const load = useCallback(async () => {
    const s = await api.getSettings();
    setSaved(s);
    setDraft({
      default_source: s.default_source,
      default_target: s.default_target,
      timeout_secs: s.timeout_secs,
      shortcuts: s.shortcuts,
      ocr: s.ocr,
      providers: s.providers,
      assistants: s.assistants,
    });
  }, []);

  useEffect(() => {
    load().catch((e) => setErrors([String(e)]));
    headingRef.current?.focus();
  }, [load]);

  const dirty = useMemo(() => {
    if (!saved || !draft) return false;
    return JSON.stringify(toInput(draft)) !== JSON.stringify(toInput(saved));
  }, [saved, draft]);

  const save = useCallback(async () => {
    if (!draft || saving) return;
    setSaving(true);
    setErrors([]);
    setStatus("Saving…");
    try {
      const warnings = await api.saveSettings(toInput(draft));
      await load();
      setStatus(warnings.length ? `Saved, with ${warnings.length} warning(s) shown below` : "Settings saved");
      setErrors(warnings);
      setConfirmLeave(false);
    } catch (e) {
      setStatus("");
      setErrors(Array.isArray(e) ? e.map(String) : [String(e)]);
    } finally {
      setSaving(false);
    }
  }, [draft, saving, load]);

  const close = useCallback(() => (dirty ? setConfirmLeave(true) : onClose()), [dirty, onClose]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.code === "KeyS") {
        e.preventDefault();
        void save();
      } else if (e.key === "Escape" && !e.defaultPrevented && !isEscapeTaken(e)) {
        close();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [save, close]);

  if (!draft || !saved) {
    return (
      <div className="settings">
        <p role="status">{errors[0] ?? "Loading settings…"}</p>
      </div>
    );
  }

  const savedIds = new Set(saved.providers.map((p) => p.id));
  const set = (patch: Partial<Draft>) => setDraft({ ...draft, ...patch });
  const setProvider = (i: number, patch: Partial<ProviderView>) => {
    const providers = draft.providers.slice();
    providers[i] = { ...providers[i], ...patch };
    // Key changes are applied immediately, so mirror them into `saved` to avoid a false "unsaved",
    // and drop messages from the last save that may be about the old key.
    if ("key_source" in patch) {
      setErrors([]);
      setStatus("");
      setSaved({
        ...saved,
        providers: saved.providers.map((p) => (p.id === providers[i].id ? { ...p, key_source: patch.key_source! } : p)),
      });
    }
    set({ providers });
  };
  const move = (i: number, delta: -1 | 1) => {
    const providers = draft.providers.slice();
    [providers[i], providers[i + delta]] = [providers[i + delta], providers[i]];
    set({ providers });
    setStatus(`Moved ${providers[i + delta].name || providers[i + delta].id} to position ${i + delta + 1}`);
  };
  const remove = (i: number) => {
    const p = draft.providers[i];
    set({ providers: draft.providers.filter((_, j) => j !== i) });
    setStatus(`Removed ${p.name || p.id}. Discard changes to undo.`);
  };
  const add = (presetId: string) => {
    const preset = PRESETS.find((p) => p.id === presetId);
    if (!preset) return;
    const id = uniqueId(preset.id, new Set(draft.providers.map((p) => p.id)));
    const provider: ProviderView = {
      id,
      kind: preset.kind,
      enabled: true,
      name: preset.name ?? null,
      base_url: preset.base_url ?? null,
      model: preset.kind === "openai-compatible" ? "" : null,
      api_key_env: null,
      key_source: "none",
      requires_key: preset.kind === "deepl",
    };
    set({ providers: [...draft.providers, provider] });
    setStatus(`Added ${preset.label} as provider ${draft.providers.length + 1}`);
  };

  const setAssistant = (i: number, patch: Partial<AssistantView>) => {
    const assistants = draft.assistants.slice();
    assistants[i] = { ...assistants[i], ...patch };
    if ("key_source" in patch) {
      setSaved({
        ...saved,
        assistants: saved.assistants.map((a) => (a.id === assistants[i].id ? { ...a, key_source: patch.key_source! } : a)),
      });
    }
    set({ assistants });
  };
  const moveAssistant = (i: number, delta: -1 | 1) => {
    const assistants = draft.assistants.slice();
    [assistants[i], assistants[i + delta]] = [assistants[i + delta], assistants[i]];
    set({ assistants });
    setStatus(`Moved ${assistants[i + delta].name || assistants[i + delta].id} to position ${i + delta + 1}`);
  };
  const removeAssistant = (i: number) => {
    const a = draft.assistants[i];
    set({ assistants: draft.assistants.filter((_, j) => j !== i) });
    setStatus(`Removed ${a.name || a.id}. Discard changes to undo.`);
  };
  const addAssistant = (presetId: string) => {
    const preset = ASSISTANT_PRESETS.find((p) => p.id === presetId);
    if (!preset) return;
    const id = uniqueId(preset.id, new Set(draft.assistants.map((a) => a.id)));
    const assistant: AssistantView = {
      id,
      kind: preset.kind,
      enabled: true,
      name: preset.name ?? null,
      base_url: preset.base_url ?? null,
      model: null,
      api_key_env: null,
      web_search: preset.kind !== "openai-compatible",
      key_source: "none",
    };
    set({ assistants: [...draft.assistants, assistant] });
    setStatus(`Added ${preset.label}. Save to use it.`);
  };

  return (
    <div className="settings">
      <div className="settings-body">
        <h2 ref={headingRef} tabIndex={-1} className="settings-title">
          Settings
        </h2>

        <section className="settings-section" aria-labelledby="general-heading">
          <h3 id="general-heading">Translation</h3>
          <div className="settings-grid">
            <div className="setting-row">
              <span className="setting-label">Default source language</span>
              <LanguagePicker
                label="Default source language"
                languages={languages}
                value={draft.default_source}
                onChange={(default_source) => set({ default_source })}
              />
            </div>
            <div className="setting-row">
              <span className="setting-label">Default target language</span>
              <LanguagePicker
                label="Default target language"
                languages={languages.filter((l) => l.code !== "auto")}
                value={draft.default_target}
                onChange={(default_target) => set({ default_target })}
              />
            </div>
            <NumberField
              className="setting-row"
              value={draft.timeout_secs}
              onChange={(v) => set({ timeout_secs: Number.isNaN(v) ? 15 : v })}
              minValue={1}
              maxValue={120}
            >
              <Label className="setting-label">Seconds to wait before trying the next provider</Label>
              <Group className="field number-field">
                <Button slot="decrement" className="field-button">
                  −
                </Button>
                <Input className="field-input" />
                <Button slot="increment" className="field-button">
                  +
                </Button>
              </Group>
            </NumberField>
          </div>
        </section>

        <section className="settings-section" aria-labelledby="providers-heading">
          <h3 id="providers-heading">Providers</h3>
          <p className="setting-hint">
            Tried from top to bottom. If one fails or is too slow, the next one is used.
          </p>
          {saved.keychain_error && (
            <p className="notice" role="note">
              The system keychain isn't available ({saved.keychain_error}). Keys can be saved in the config file
              instead, which stores them as plain text. On KDE, enable KWallet's Secret Service to use the keychain.
            </p>
          )}
          {draft.providers.map((p, i) => (
            <ProviderCard
              key={p.id}
              provider={p}
              index={i}
              count={draft.providers.length}
              saved={savedIds.has(p.id)}
              keychainError={saved.keychain_error}
              onChange={(patch) => setProvider(i, patch)}
              onMove={(d) => move(i, d)}
              onRemove={() => remove(i)}
            />
          ))}
          <Select className="add-provider" placeholder="Add a provider…" selectedKey={null} onSelectionChange={(k) => k && add(String(k))}>
            <Label className="sr-only">Add a provider</Label>
            <Button className="select-button">
              <SelectValue />
              <span aria-hidden>▾</span>
            </Button>
            <Popover className="popover">
              <ListBox className="listbox" items={PRESETS}>
                {(p) => (
                  <ListBoxItem className="listbox-item" textValue={p.label}>
                    {p.label}
                  </ListBoxItem>
                )}
              </ListBox>
            </Popover>
          </Select>
        </section>

        <section className="settings-section" aria-labelledby="assistants-heading">
          <h3 id="assistants-heading">AI assistants</h3>
          <p className="setting-hint">Used by Ask AI and the Ask popup. Sign-in and keys apply immediately; other changes on Save.</p>
          {draft.assistants.map((a, i) => (
            <AssistantCard
              key={a.id}
              assistant={a}
              index={i}
              count={draft.assistants.length}
              secretsLocation={saved.secrets_location}
              onChange={(patch) => setAssistant(i, patch)}
              onMove={(d) => moveAssistant(i, d)}
              onRemove={() => removeAssistant(i)}
              onApplied={() => setStatus("Applied")}
            />
          ))}
          <Select className="add-provider" placeholder="Add an assistant…" selectedKey={null} onSelectionChange={(k) => k && addAssistant(String(k))}>
            <Label className="sr-only">Add an assistant</Label>
            <Button className="select-button">
              <SelectValue />
              <span aria-hidden>▾</span>
            </Button>
            <Popover className="popover">
              <ListBox className="listbox" items={ASSISTANT_PRESETS}>
                {(p) => (
                  <ListBoxItem className="listbox-item" textValue={p.label}>
                    {p.label}
                  </ListBoxItem>
                )}
              </ListBox>
            </Popover>
          </Select>
        </section>

        <section className="settings-section" aria-labelledby="ocr-heading">
          <h3 id="ocr-heading">Screen text (OCR)</h3>
          <OcrSettings value={draft.ocr} onChange={(ocr) => set({ ocr })} />
        </section>

        <section className="settings-section" aria-labelledby="shortcuts-heading">
          <h3 id="shortcuts-heading">Global shortcuts</h3>
          <p className="setting-hint">These work in any app. Clear one to turn it off.</p>
          <ShortcutRecorder
            label="Show Corvus"
            value={draft.shortcuts.show_window}
            onChange={(show_window) => set({ shortcuts: { ...draft.shortcuts, show_window } })}
          />
          <ShortcutRecorder
            label="Translate selected text"
            value={draft.shortcuts.translate_selection}
            onChange={(translate_selection) => set({ shortcuts: { ...draft.shortcuts, translate_selection } })}
          />
          <ShortcutRecorder
            label="Ask AI about selected text"
            value={draft.shortcuts.ask_selection}
            onChange={(ask_selection) => set({ shortcuts: { ...draft.shortcuts, ask_selection } })}
          />
          <ShortcutRecorder
            label="Translate text in a screen area"
            value={draft.shortcuts.ocr_translate}
            onChange={(ocr_translate) => set({ shortcuts: { ...draft.shortcuts, ocr_translate } })}
          />
          <ShortcutRecorder
            label="Copy text from a screen area"
            value={draft.shortcuts.ocr_copy}
            onChange={(ocr_copy) => set({ shortcuts: { ...draft.shortcuts, ocr_copy } })}
          />
          <ShortcutRecorder
            label="Replace selected text with its translation"
            value={draft.shortcuts.replace_selection}
            onChange={(replace_selection) => set({ shortcuts: { ...draft.shortcuts, replace_selection } })}
          />
        </section>

        {saved.config_path && (
          <p className="setting-hint">
            Settings file: <code>{saved.config_path}</code>
          </p>
        )}
      </div>

      <footer className="settings-footer">
        <div className="settings-messages">
          <p role="status" className="status">
            {confirmLeave ? "You have unsaved changes." : dirty ? "Unsaved changes" : status}
          </p>
          {errors.length > 0 && (
            <ul role="alert" className="error">
              {errors.map((e) => (
                <li key={e}>{e}</li>
              ))}
            </ul>
          )}
        </div>
        {confirmLeave ? (
          <>
            <Button className="btn" onPress={() => setConfirmLeave(false)} autoFocus>
              Keep editing
            </Button>
            <Button className="btn btn-danger" onPress={onClose}>
              Discard and close
            </Button>
          </>
        ) : (
          <>
            <Button className="btn" onPress={close}>
              Close <kbd aria-hidden>Esc</kbd>
            </Button>
            <Button
              className="btn"
              onPress={() => {
                void load();
                setErrors([]);
                setStatus("Changes discarded");
              }}
              isDisabled={!dirty}
            >
              Discard
            </Button>
          </>
        )}
        <Button className="btn btn-primary" onPress={() => void save()} isDisabled={!dirty || saving}>
          Save <kbd aria-hidden>Ctrl+S</kbd>
        </Button>
      </footer>
    </div>
  );
}
