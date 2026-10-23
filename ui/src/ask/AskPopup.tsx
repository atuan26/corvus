import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { Button, Label, TextArea, TextField } from "react-aria-components";
import { api, type AssistantStatus } from "../api";
import { isEscapeTaken } from "../shortcuts";
import { TurnView } from "./Transcript";
import { useAsk } from "./useAsk";

const SYSTEM =
  "The user selected some text in another app and wants help with it. Answer directly and concisely. " +
  "When asked to rewrite or fix the text, reply with only the new text, in the same language unless asked otherwise.";

interface Preset {
  label: string;
  prompt: (target: string) => string;
}

const PRESETS: Preset[] = [
  { label: "Explain", prompt: () => "Explain this simply." },
  { label: "Summarize", prompt: () => "Summarize this in a few bullet points." },
  { label: "Fix grammar", prompt: () => "Fix the grammar and spelling. Reply with only the corrected text." },
  { label: "Translate & explain", prompt: (t) => `Translate this into ${t} and explain any tricky words or phrases.` },
  { label: "Write a reply", prompt: () => "Write a short, polite reply to this message." },
  { label: "Make it formal", prompt: () => "Rewrite this to sound more formal. Reply with only the rewritten text." },
];

function withSelection(instruction: string, selection: string) {
  return selection ? `${instruction}\n\n<selected_text>\n${selection}\n</selected_text>` : instruction;
}

/** Ask about the current selection: numbered presets, follow-ups, copy/replace/continue. */
export default function AskPopup() {
  const [selection, setSelection] = useState("");
  const [assistant, setAssistant] = useState<AssistantStatus | null>(null);
  const [target, setTarget] = useState("English");
  const [draft, setDraft] = useState("");
  const [status, setStatus] = useState("");
  const rootRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const { turns, busy, send, stop, reset } = useAsk(assistant?.id ?? null, setStatus);

  const load = useCallback(async (text: string) => {
    reset();
    setSelection(text);
    setDraft("");
    setStatus("");
    const [list, prefs, langs] = await Promise.all([api.listAssistants(), api.getPrefs(), api.languages()]);
    let chosen: string | null = null;
    try {
      chosen = localStorage.getItem("corvus.ask.assistant");
    } catch {
      // Use the first ready assistant.
    }
    setAssistant(list.find((a) => a.id === chosen && !a.problem) ?? list.find((a) => !a.problem) ?? list[0] ?? null);
    setTarget(langs.find((l) => l.code === prefs.target)?.name ?? "English");
    requestAnimationFrame(() => (text ? rootRef.current?.focus() : inputRef.current?.focus()));
  }, [reset]);

  useEffect(() => {
    void api.popupText().then((t) => load(t ?? ""));
    const un = listen<string>("popup-text", (e) => void load(e.payload));
    return () => void un.then((f) => f());
  }, [load]);

  const ready = !!assistant && !assistant.problem;
  const ask = (instruction: string) => {
    if (!ready || busy) return;
    // Only the first question carries the selection; follow-ups refer back to it.
    const text = turns.length === 0 ? withSelection(instruction, selection) : instruction;
    void send(text, { system: SYSTEM, webSearch: false });
    setDraft("");
    requestAnimationFrame(() => rootRef.current?.focus());
  };

  const answer = [...turns].reverse().find((t) => t.message.role === "assistant" && !t.streaming && !t.error)?.message.text ?? "";

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const inField = e.target instanceof HTMLTextAreaElement || e.target instanceof HTMLInputElement;
      if (e.key === "Escape" && !isEscapeTaken(e)) {
        e.preventDefault();
        if (busy) stop();
        else void api.hidePopup();
      } else if (!inField && !e.ctrlKey && !e.altKey && /^Digit[1-9]$/.test(e.code)) {
        const preset = PRESETS[Number(e.code.slice(5)) - 1];
        if (preset && turns.length === 0) {
          e.preventDefault();
          ask(preset.prompt(target));
        }
      } else if (e.key === "Enter" && !inField && answer) {
        e.preventDefault();
        void navigator.clipboard.writeText(answer).then(() => api.hidePopup());
      } else if ((e.ctrlKey || e.metaKey) && e.code === "KeyR" && answer && selection) {
        e.preventDefault();
        void api.pasteText(answer);
      } else if ((e.ctrlKey || e.metaKey) && e.code === "KeyO") {
        e.preventDefault();
        void api.openInMain(selection, "ask");
      } else if ((e.ctrlKey || e.metaKey) && e.code === "KeyL") {
        e.preventDefault();
        inputRef.current?.focus();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  return (
    <div ref={rootRef} className="popup ask-popup" tabIndex={-1} role="dialog" aria-label="Ask about the selection">
      <div className="popup-bar">
        <span className="popup-from">{assistant ? `${assistant.name}${assistant.model ? ` · ${assistant.model}` : ""}` : "No assistant"}</span>
      </div>
      {selection && (
        <p className="popup-source" title={selection}>
          {selection}
        </p>
      )}
      {!ready && (
        <div className="notice">
          {assistant?.problem ?? "No AI assistant is set up."}{" "}
          <Button className="btn" onPress={() => void api.openInMain("", "ask")}>
            Set up in Corvus
          </Button>
        </div>
      )}

      {turns.length === 0 && ready && (
        <div className="presets" role="group" aria-label="Quick actions">
          {PRESETS.map((p, i) => (
            <Button key={p.label} className="btn" onPress={() => ask(p.prompt(target))} isDisabled={!selection && i !== 0}>
              <kbd aria-hidden>{i + 1}</kbd> {p.label}
            </Button>
          ))}
        </div>
      )}

      <section className="transcript popup-transcript" aria-label="Answer">
        {turns
          .filter((t) => t.message.role === "assistant" || turns.indexOf(t) > 0)
          .map((t, i) => (
            <TurnView key={i} turn={t} />
          ))}
      </section>

      <TextField className="composer-field" value={draft} onChange={setDraft} isDisabled={!ready}>
        <Label className="sr-only">{turns.length ? "Follow-up question" : "Ask about it"}</Label>
        <TextArea
          ref={inputRef}
          className="composer-input"
          rows={1}
          placeholder={turns.length ? "Follow-up… (Ctrl+L)" : selection ? "Or ask anything about it… (Ctrl+L)" : "Ask anything…"}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing && draft.trim()) {
              e.preventDefault();
              ask(draft);
            }
          }}
        />
      </TextField>

      <div className="popup-footer">
        <span className="popup-provider" role="status">
          {status}
        </span>
        {busy && (
          <Button className="btn" onPress={stop}>
            Stop <kbd aria-hidden>Esc</kbd>
          </Button>
        )}
        <Button className="btn" onPress={() => void api.openInMain(selection, "ask")}>
          Continue <kbd aria-hidden>Ctrl+O</kbd>
        </Button>
        {selection && (
          <Button className="btn" onPress={() => void api.pasteText(answer)} isDisabled={!answer}>
            Replace <kbd aria-hidden>Ctrl+R</kbd>
          </Button>
        )}
        <Button
          className="btn btn-primary"
          onPress={() => void navigator.clipboard.writeText(answer).then(() => api.hidePopup())}
          isDisabled={!answer}
        >
          Copy <kbd aria-hidden>Enter</kbd>
        </Button>
      </div>
    </div>
  );
}
