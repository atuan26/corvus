import { useEffect, useId, useRef, useState, type KeyboardEvent } from "react";
import { Button } from "react-aria-components";
import { api } from "../api";

const MODIFIER_KEYS = new Set(["Control", "Alt", "Shift", "Meta", "AltGraph", "OS", "Super", "Hyper"]);

/** Build an accelerator like "Ctrl+Alt+E" from a key event, or null for a bare modifier press. */
export function toAccelerator(e: Pick<KeyboardEvent, "key" | "code" | "ctrlKey" | "altKey" | "shiftKey" | "metaKey">) {
  if (MODIFIER_KEYS.has(e.key)) return null;
  const parts: string[] = [];
  if (e.ctrlKey) parts.push("Ctrl");
  if (e.altKey) parts.push("Alt");
  if (e.shiftKey) parts.push("Shift");
  if (e.metaKey) parts.push("Super");
  // Physical key names work on any keyboard layout and are what the backend parser accepts.
  const key = e.code.startsWith("Key") ? e.code.slice(3) : e.code.startsWith("Digit") ? e.code.slice(5) : e.code;
  parts.push(key);
  return parts.join("+");
}

/** Global shortcuts need Ctrl, Alt or Super, otherwise they'd swallow normal typing. F-keys may stand alone. */
export function isUsableGlobally(accel: string) {
  const parts = accel.split("+");
  const key = parts[parts.length - 1];
  return /^F([1-9]|1\d|2[0-4])$/.test(key) || parts.some((p) => p === "Ctrl" || p === "Alt" || p === "Super");
}

interface Props {
  label: string;
  value: string;
  onChange: (keys: string) => void;
}

export function ShortcutRecorder({ label, value, onChange }: Props) {
  const [recording, setRecording] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const recordingRef = useRef(false);
  // A Space/Enter keyup after recording would "click" the button again and restart recording.
  const finishedAt = useRef(0);
  const labelId = useId();
  const hintId = useId();

  const setRec = (on: boolean) => {
    if (recordingRef.current === on) return;
    recordingRef.current = on;
    setRecording(on);
    void (on ? api.pauseShortcuts() : api.resumeShortcuts());
  };

  const finish = () => {
    finishedAt.current = Date.now();
    setRec(false);
  };

  // Never leave global shortcuts paused if the screen closes mid-recording.
  useEffect(() => () => void (recordingRef.current && api.resumeShortcuts()), []);

  const onKeyDown = (e: KeyboardEvent<HTMLButtonElement>) => {
    if (!recording) return;
    e.preventDefault();
    e.stopPropagation();
    const plain = !e.ctrlKey && !e.altKey && !e.shiftKey && !e.metaKey;
    if (plain && e.key === "Escape") {
      setError(null);
      return finish();
    }
    if (plain && (e.key === "Backspace" || e.key === "Delete")) {
      onChange("");
      return finish();
    }
    const accel = toAccelerator(e);
    if (!accel) return;
    if (!isUsableGlobally(accel)) {
      setError(`${accel} needs Ctrl, Alt or Super (F1–F24 can be used alone)`);
      return;
    }
    api
      .validateShortcut(accel)
      .then(() => {
        setError(null);
        onChange(accel);
        finish();
      })
      .catch((err) => setError(`${accel} can't be used: ${String(err)}`));
  };

  return (
    <div className="setting-row">
      <span id={labelId} className="setting-label">
        {label}
      </span>
      <div className="recorder">
        <button
          type="button"
          className="recorder-button btn"
          data-recording={recording || undefined}
          aria-labelledby={labelId}
          aria-describedby={hintId}
          aria-pressed={recording}
          onClick={() => {
            if (!recording && Date.now() - finishedAt.current > 400) {
              setError(null);
              setRec(true);
            }
          }}
          onKeyDown={onKeyDown}
          onKeyUp={(e) => recording && e.preventDefault()}
          onBlur={() => recording && finish()}
        >
          {recording ? "Press keys…" : value ? <kbd>{value}</kbd> : "Not set"}
        </button>
        <Button className="btn" onPress={() => onChange("")} isDisabled={!value} aria-label={`Clear ${label}`}>
          Clear
        </Button>
      </div>
      <p id={hintId} className="setting-hint" aria-live="polite">
        {recording ? "Press the new shortcut. Esc cancels, Backspace removes it." : "Press Enter to record a new shortcut."}
      </p>
      {error && (
        <p role="alert" className="field-error">
          {error}
        </p>
      )}
    </div>
  );
}
