import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  Button,
  Label,
  ListBox,
  ListBoxItem,
  Popover,
  Select,
  SelectValue,
  Switch,
  TextArea,
  TextField,
} from "react-aria-components";
import { ArrowUp, Globe, ScanText, Square, SquarePen } from "lucide-react";
import { api, type AssistantStatus } from "../api";
import { IconButton } from "../components/IconButton";
import { ChatGptSignIn } from "./ChatGptSignIn";
import { TurnView } from "./Transcript";
import { useAsk } from "./useAsk";

const ASSISTANT_KEY = "corvus.ask.assistant";

function stored(key: string) {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

function store(key: string, value: string) {
  try {
    localStorage.setItem(key, value);
  } catch {
    // Remembering the choice is a convenience only.
  }
}

interface Props {
  /** Whether the Ask tab is showing; focus moves to the question box when it opens. */
  active: boolean;
  onOpenSettings: () => void;
  /** Text handed over from the Ask popup or elsewhere. */
  initialText?: string;
}

export default function AskView({ active, onOpenSettings, initialText }: Props) {
  const [assistants, setAssistants] = useState<AssistantStatus[]>([]);
  const [assistantId, setAssistantId] = useState<string | null>(null);
  const [webSearch, setWebSearch] = useState(true);
  const [draft, setDraft] = useState(initialText ?? "");
  const [status, setStatus] = useState("");
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const endRef = useRef<HTMLDivElement>(null);
  const { turns, busy, send, stop, reset } = useAsk(assistantId, setStatus);

  const load = useCallback(async () => {
    const list = await api.listAssistants();
    setAssistants(list);
    setAssistantId((current) => {
      const keep = list.find((a) => a.id === (current ?? stored(ASSISTANT_KEY)));
      return (keep ?? list.find((a) => !a.problem) ?? list[0])?.id ?? null;
    });
  }, []);

  useEffect(() => {
    void load();
    inputRef.current?.focus();
    const un = listen("settings-changed", () => void load());
    return () => void un.then((f) => f());
  }, [load]);

  useEffect(() => {
    if (active) requestAnimationFrame(() => inputRef.current?.focus());
  }, [active]);

  useEffect(() => {
    if (initialText) {
      setDraft(initialText);
      inputRef.current?.focus();
    }
  }, [initialText]);

  // Keep the newest text in view while it streams. Braces matter: Chromium's scrollIntoView
  // returns a Promise, which React would take as the cleanup function and crash on unmount.
  useEffect(() => {
    endRef.current?.scrollIntoView({ block: "end" });
  }, [turns]);

  const current = assistants.find((a) => a.id === assistantId) ?? null;
  const ready = !!current && !current.problem;

  const submit = () => {
    if (!draft.trim() || busy || !ready) return;
    void send(draft, { webSearch: webSearch && !!current?.web_search });
    setDraft("");
  };

  useEffect(() => {
    if (!active) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && busy) {
        e.preventDefault();
        stop();
      } else if ((e.ctrlKey || e.metaKey) && e.code === "KeyN") {
        e.preventDefault();
        reset();
        setStatus("New conversation");
        inputRef.current?.focus();
      } else if ((e.ctrlKey || e.metaKey) && e.code === "KeyL") {
        e.preventDefault();
        inputRef.current?.focus();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [active, busy, stop, reset]);

  return (
    <div className="ask">
      <div className="ask-toolbar">
        <Select
          className="provider-select"
          selectedKey={assistantId}
          onSelectionChange={(k) => {
            if (k == null) return;
            setAssistantId(String(k));
            store(ASSISTANT_KEY, String(k));
          }}
          isDisabled={assistants.length === 0}
          placeholder="No assistants"
        >
          <Label className="sr-only">Assistant</Label>
          <Button className="select-button">
            <SelectValue />
            <span aria-hidden>▾</span>
          </Button>
          <Popover className="popover">
            <ListBox className="listbox" items={assistants}>
              {(a) => (
                <ListBoxItem className="listbox-item" textValue={a.name}>
                  <span>{a.name}</span>
                  <span className="listbox-native">{a.problem ? "needs setup" : a.model}</span>
                </ListBoxItem>
              )}
            </ListBox>
          </Popover>
        </Select>
        <Switch className="switch" isSelected={webSearch && !!current?.web_search} isDisabled={!current?.web_search} onChange={setWebSearch}>
          <span className="switch-track" aria-hidden />
          <Globe size={15} aria-hidden /> Search the web
        </Switch>
        <span className="toolbar-spacer" />
        <IconButton
          showLabel
          label="New chat"
          shortcut="Ctrl+N"
          icon={<SquarePen />}
          isDisabled={turns.length === 0}
          onPress={() => {
            reset();
            inputRef.current?.focus();
          }}
        />
      </div>

      {current?.problem && (
        <div className="notice ask-setup">
          <p>
            <strong>{current.name}</strong>: {current.problem}.
          </p>
          {current.kind === "chatgpt" ? (
            <ChatGptSignIn onChange={() => void load()} compact />
          ) : (
            <Button className="btn" onPress={onOpenSettings}>
              Open Settings
            </Button>
          )}
        </div>
      )}
      {assistants.length === 0 && (
        <div className="notice ask-setup">
          <p>No AI assistant is set up yet.</p>
          <Button className="btn" onPress={onOpenSettings}>
            Open Settings
          </Button>
        </div>
      )}

      <section className="transcript" aria-label="Conversation">
        {turns.length === 0 && ready && (
          <p className="setting-hint ask-empty">
            Ask anything, or read a question from the screen. Enter sends, Shift+Enter adds a line, Esc stops an answer.
          </p>
        )}
        {turns.map((t, i) => (
          <TurnView key={i} turn={t} onSignIn={onOpenSettings} />
        ))}
        <div ref={endRef} />
      </section>

      <div className="composer">
        <TextField className="composer-field" value={draft} onChange={setDraft} isDisabled={!ready}>
          <Label className="sr-only">Ask a question</Label>
          <TextArea
            ref={inputRef}
            className="composer-input"
            rows={2}
            placeholder={ready ? "Ask a question…" : "Set up an assistant first"}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) {
                e.preventDefault();
                submit();
              }
            }}
          />
        </TextField>
        <IconButton
          label="Read text from screen"
          icon={<ScanText />}
          isDisabled={!ready}
          onPress={() => void api.ocrStart("ask")}
        />
        {busy ? (
          <Button className="btn" onPress={stop}>
            <Square size={14} aria-hidden /> Stop <kbd aria-hidden>Esc</kbd>
          </Button>
        ) : (
          <Button className="btn btn-primary" onPress={submit} isDisabled={!draft.trim() || !ready}>
            <ArrowUp size={16} aria-hidden /> Send <kbd aria-hidden>Enter</kbd>
          </Button>
        )}
      </div>
      <p role="status" className="status">
        {status}
      </p>
    </div>
  );
}
