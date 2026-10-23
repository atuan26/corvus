import { useId, useState } from "react";
import { Button, Input, Label, Switch, TextField } from "react-aria-components";
import { api, type KeySource, type ProviderView } from "../api";

const KIND_LABEL: Record<ProviderView["kind"], string> = {
  "google-free": "Google (free, no key)",
  deepl: "DeepL API",
  "openai-compatible": "OpenAI-compatible AI",
};

const KEY_STATUS: Record<KeySource, string> = {
  keychain: "Key saved in the system keychain",
  config: "Key saved in the config file (plain text)",
  env: "Key read from an environment variable",
  none: "No key saved",
};

function isLocal(url?: string | null) {
  return /^https?:\/\/(localhost|127\.0\.0\.1|\[::1\])(:|\/|$)/.test(url ?? "");
}

interface Props {
  provider: ProviderView;
  index: number;
  count: number;
  /** Whether this id exists in the saved config (keys for unsaved providers go to the keychain only). */
  saved: boolean;
  keychainError: string | null;
  onChange: (patch: Partial<ProviderView>) => void;
  onMove: (delta: -1 | 1) => void;
  onRemove: () => void;
}

export function ProviderCard({ provider: p, index, count, saved, keychainError, onChange, onMove, onRemove }: Props) {
  const [key, setKey] = useState("");
  const [keyStatus, setKeyStatus] = useState<string | null>(null);
  const [test, setTest] = useState<{ ok: boolean; text: string } | null>(null);
  const [testing, setTesting] = useState(false);
  const headingId = useId();
  const title = p.name || KIND_LABEL[p.kind];
  const usesKey = p.kind !== "google-free";
  const store = keychainError ? "config" : "keychain";

  const saveKey = async (value: string | null) => {
    setKeyStatus(null);
    try {
      const source = await api.setApiKey(p.id, value, store);
      onChange({ key_source: source });
      setKey("");
      setKeyStatus(value ? "Key saved" : "Key removed");
    } catch (e) {
      setKeyStatus(`Could not save the key: ${String(e)}`);
    }
  };

  const runTest = async () => {
    setTesting(true);
    setTest(null);
    try {
      const t = await api.testProvider(p);
      setTest({ ok: true, text: `Works: "Hello, world!" → "${t.text}"` });
    } catch (e) {
      setTest({ ok: false, text: `Failed: ${String(e)}` });
    } finally {
      setTesting(false);
    }
  };

  return (
    <section className="provider-card" aria-labelledby={headingId} data-disabled={!p.enabled || undefined}>
      <div className="provider-head">
        <span className="provider-rank" aria-hidden>
          {index + 1}
        </span>
        <h3 id={headingId} className="provider-title">
          {title} <span className="provider-id">({p.id})</span>
        </h3>
        <Switch className="switch" isSelected={p.enabled} onChange={(enabled) => onChange({ enabled })}>
          <span className="switch-track" aria-hidden />
          Enabled
        </Switch>
      </div>

      <div className="provider-fields">
        <TextField className="text-field" value={p.name ?? ""} onChange={(name) => onChange({ name: name || null })}>
          <Label>Display name</Label>
          <Input placeholder={KIND_LABEL[p.kind]} />
        </TextField>
        {p.kind === "openai-compatible" && (
          <>
            <TextField
              className="text-field"
              value={p.base_url ?? ""}
              onChange={(base_url) => onChange({ base_url })}
              isRequired
            >
              <Label>Base URL</Label>
              <Input placeholder="https://api.example.com/v1" inputMode="url" />
            </TextField>
            <TextField className="text-field" value={p.model ?? ""} onChange={(model) => onChange({ model })} isRequired>
              <Label>Model</Label>
              <Input placeholder="Model name, as the service lists it" />
            </TextField>
          </>
        )}
      </div>

      {usesKey && (
        <div className="provider-key">
          <p className="setting-hint">
            {KEY_STATUS[p.key_source]}
            {p.key_source === "env" && p.api_key_env ? ` (${p.api_key_env})` : ""}
            {p.key_source === "none" && isLocal(p.base_url) ? ". Not needed for local servers." : ""}
          </p>
          <form
            className="key-row"
            onSubmit={(e) => {
              e.preventDefault();
              if (key.trim()) void saveKey(key);
            }}
          >
            <TextField className="text-field grow" value={key} onChange={setKey} type="password">
              <Label>{p.key_source === "none" ? "API key" : "Replace API key"}</Label>
              <Input autoComplete="off" spellCheck={false} />
            </TextField>
            <Button type="submit" className="btn" isDisabled={!key.trim() || (store === "config" && !saved)}>
              {store === "keychain" ? "Save key" : "Save in config file"}
            </Button>
            {(p.key_source === "keychain" || p.key_source === "config") && (
              <Button className="btn" onPress={() => void saveKey(null)}>
                Remove key
              </Button>
            )}
          </form>
          {store === "config" && !saved && (
            <p className="setting-hint">Save settings first, then add the key.</p>
          )}
          {keyStatus && (
            <p className="setting-hint" role="status">
              {keyStatus}
            </p>
          )}
        </div>
      )}

      <div className="provider-actions">
        <Button className="btn" onPress={() => void runTest()} isDisabled={testing}>
          {testing ? "Testing…" : "Test"}
        </Button>
        <Button className="btn" onPress={() => onMove(-1)} isDisabled={index === 0} aria-label={`Move ${title} up`}>
          ↑ Up
        </Button>
        <Button
          className="btn"
          onPress={() => onMove(1)}
          isDisabled={index === count - 1}
          aria-label={`Move ${title} down`}
        >
          ↓ Down
        </Button>
        <Button className="btn btn-danger" onPress={onRemove} aria-label={`Remove ${title}`}>
          Remove
        </Button>
      </div>
      <p role="status" className={test ? (test.ok ? "test-ok" : "field-error") : "sr-only"}>
        {test?.text ?? ""}
      </p>
    </section>
  );
}
