import { useEffect, useId, useState } from "react";
import { Button, Input, Label, ListBox, ListBoxItem, Popover, Select, SelectValue, Switch, TextField } from "react-aria-components";
import { api, type AssistantView, type KeySource, type ModelEntry } from "../api";
import { ChatGptSignIn } from "../ask/ChatGptSignIn";

const KIND_LABEL: Record<AssistantView["kind"], string> = {
  chatgpt: "ChatGPT (sign in with your plan)",
  anthropic: "Claude (Anthropic API key)",
  openai: "OpenAI API key",
  "openai-compatible": "OpenAI-compatible server",
};

const DEFAULT_MODEL: Partial<Record<AssistantView["kind"], string>> = {
  anthropic: "claude-opus-5-5",
  openai: "gpt-5.5",
};

const KEY_STATUS: Record<KeySource, string> = {
  keychain: "API key saved",
  config: "API key saved in the config file (plain text)",
  env: "API key read from an environment variable",
  none: "No API key saved",
};

interface Props {
  assistant: AssistantView;
  index: number;
  count: number;
  secretsLocation: "keychain" | "file";
  onChange: (patch: Partial<AssistantView>) => void;
  onMove: (delta: -1 | 1) => void;
  onRemove: () => void;
  /** Sign-in or key changes apply immediately; the parent reloads statuses. */
  onApplied: () => void;
}

function ChatGptModelPicker({ value, onChange }: { value: string; onChange: (m: string) => void }) {
  const [models, setModels] = useState<ModelEntry[] | null>(null);
  const [error, setError] = useState("");
  useEffect(() => {
    api.chatgptModels().then(setModels).catch((e) => setError(String(e)));
  }, []);
  const items = [{ slug: "", name: "Best available (recommended)" }, ...(models ?? [])];
  if (value && !items.some((m) => m.slug === value)) items.push({ slug: value, name: value });
  return (
    <div className="setting-row">
      <Select className="provider-select" selectedKey={value} onSelectionChange={(k) => onChange(String(k ?? ""))}>
        <Label className="setting-label">Model</Label>
        <Button className="select-button">
          <SelectValue />
          <span aria-hidden>▾</span>
        </Button>
        <Popover className="popover">
          <ListBox className="listbox" items={items.map((m) => ({ id: m.slug, ...m }))}>
            {(m) => (
              <ListBoxItem className="listbox-item" textValue={m.name}>
                <span>{m.name}</span>
                {m.slug && <span className="listbox-native">{m.slug}</span>}
              </ListBoxItem>
            )}
          </ListBox>
        </Popover>
      </Select>
      {error && <p className="setting-hint">Model list unavailable: {error}</p>}
    </div>
  );
}

export function AssistantCard({ assistant: a, index, count, secretsLocation, onChange, onMove, onRemove, onApplied }: Props) {
  const [key, setKey] = useState("");
  const [keyStatus, setKeyStatus] = useState("");
  const [signedIn, setSignedIn] = useState(false);
  const headingId = useId();
  const title = a.name || KIND_LABEL[a.kind];

  useEffect(() => {
    if (a.kind === "chatgpt") void api.chatgptStatus().then((s) => setSignedIn(s.signed_in));
  }, [a.kind]);

  const saveKey = async (value: string | null) => {
    try {
      await api.setAssistantKey(a.id, value);
      onChange({ key_source: value ? "keychain" : "none" });
      setKey("");
      setKeyStatus(value ? "Key saved" : "Key removed");
      onApplied();
    } catch (e) {
      setKeyStatus(`Could not save the key: ${String(e)}`);
    }
  };

  return (
    <section className="provider-card" aria-labelledby={headingId} data-disabled={!a.enabled || undefined}>
      <div className="provider-head">
        <span className="provider-rank" aria-hidden>
          {index + 1}
        </span>
        <h3 id={headingId} className="provider-title">
          {title} <span className="provider-id">({a.id})</span>
        </h3>
        <Switch className="switch" isSelected={a.enabled} onChange={(enabled) => onChange({ enabled })}>
          <span className="switch-track" aria-hidden />
          Enabled
        </Switch>
      </div>

      {a.kind === "chatgpt" && (
        <>
          <ChatGptSignIn
            onChange={() => {
              void api.chatgptStatus().then((s) => setSignedIn(s.signed_in));
              onApplied();
            }}
          />
          <p className="setting-hint">
            Beta. Uses the same sign-in as OpenAI's Codex CLI; it is not a published OpenAI API and may change.
          </p>
          {signedIn && <ChatGptModelPicker value={a.model ?? ""} onChange={(model) => onChange({ model: model || null })} />}
        </>
      )}

      {a.kind !== "chatgpt" && (
        <div className="provider-fields">
          <TextField className="text-field" value={a.name ?? ""} onChange={(name) => onChange({ name: name || null })}>
            <Label>Display name</Label>
            <Input placeholder={KIND_LABEL[a.kind]} />
          </TextField>
          {a.kind === "openai-compatible" && (
            <TextField className="text-field" value={a.base_url ?? ""} onChange={(base_url) => onChange({ base_url })} isRequired>
              <Label>Base URL</Label>
              <Input placeholder="http://localhost:11434/v1" inputMode="url" />
            </TextField>
          )}
          <TextField
            className="text-field"
            value={a.model ?? ""}
            onChange={(model) => onChange({ model: model || null })}
            isRequired={a.kind === "openai-compatible"}
          >
            <Label>Model</Label>
            <Input placeholder={DEFAULT_MODEL[a.kind] ? `Default: ${DEFAULT_MODEL[a.kind]}` : "Model name"} />
          </TextField>
        </div>
      )}

      {a.kind !== "chatgpt" && (
        <div className="provider-key">
          <p className="setting-hint">
            {KEY_STATUS[a.key_source]}
            {a.key_source === "keychain" && secretsLocation === "file" ? " (private file, no keychain available)" : ""}
            {a.key_source === "env" && a.api_key_env ? ` (${a.api_key_env})` : ""}
          </p>
          <form
            className="key-row"
            onSubmit={(e) => {
              e.preventDefault();
              if (key.trim()) void saveKey(key);
            }}
          >
            <TextField className="text-field grow" value={key} onChange={setKey} type="password">
              <Label>{a.key_source === "none" ? "API key" : "Replace API key"}</Label>
              <Input autoComplete="off" spellCheck={false} />
            </TextField>
            <Button type="submit" className="btn" isDisabled={!key.trim()}>
              Save key
            </Button>
            {a.key_source === "keychain" && (
              <Button className="btn" onPress={() => void saveKey(null)}>
                Remove key
              </Button>
            )}
          </form>
          {keyStatus && (
            <p className="setting-hint" role="status">
              {keyStatus}
            </p>
          )}
        </div>
      )}

      {a.kind !== "openai-compatible" && (
        <Switch className="switch provider-switch" isSelected={a.web_search} onChange={(web_search) => onChange({ web_search })}>
          <span className="switch-track" aria-hidden />
          Allow web search
        </Switch>
      )}

      <div className="provider-actions">
        <Button className="btn" onPress={() => onMove(-1)} isDisabled={index === 0} aria-label={`Move ${title} up`}>
          ↑ Up
        </Button>
        <Button className="btn" onPress={() => onMove(1)} isDisabled={index === count - 1} aria-label={`Move ${title} down`}>
          ↓ Down
        </Button>
        <Button className="btn btn-danger" onPress={onRemove} aria-label={`Remove ${title}`}>
          Remove
        </Button>
      </div>
    </section>
  );
}
