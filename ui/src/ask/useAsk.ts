import { useCallback, useRef, useState } from "react";
import { Channel } from "@tauri-apps/api/core";
import { api, type ChatMessage, type Source, type StreamMessage } from "../api";

export interface Turn {
  message: ChatMessage;
  sources: Source[];
  searches: string[];
  streaming?: boolean;
  error?: string;
  needsSignIn?: boolean;
  stopped?: boolean;
}

interface SendOptions {
  system?: string;
  webSearch: boolean;
}

/** One conversation with an assistant, streamed turn by turn. */
export function useAsk(assistantId: string | null, onStatus?: (text: string) => void) {
  const [turns, setTurns] = useState<Turn[]>([]);
  const [busy, setBusy] = useState(false);
  const requestId = useRef<string | null>(null);

  const updateLast = (f: (t: Turn) => Turn) =>
    setTurns((ts) => (ts.length ? [...ts.slice(0, -1), f(ts[ts.length - 1])] : ts));

  const send = useCallback(
    async (text: string, opts: SendOptions) => {
      if (!assistantId || !text.trim() || requestId.current) return;
      // Failed or stopped answers are left out of the history sent back.
      const history = turns.filter((t) => !t.error && !t.streaming).map((t) => t.message);
      const user: ChatMessage = { role: "user", text };
      const id = crypto.randomUUID();
      requestId.current = id;
      setBusy(true);
      setTurns((ts) => [
        ...ts,
        { message: user, sources: [], searches: [] },
        { message: { role: "assistant", text: "" }, sources: [], searches: [], streaming: true },
      ]);

      const finish = () => {
        if (requestId.current === id) {
          requestId.current = null;
          setBusy(false);
        }
      };
      const channel = new Channel<StreamMessage>();
      channel.onmessage = (m) => {
        if (requestId.current !== id) return;
        if (m.type === "event") {
          const e = m.event;
          if (e.type === "text") updateLast((t) => ({ ...t, message: { ...t.message, text: t.message.text + e.delta } }));
          else if (e.type === "searching") {
            updateLast((t) => ({ ...t, searches: [...t.searches, e.query] }));
            onStatus?.(`Searching the web for “${e.query}”`);
          } else updateLast((t) => ({ ...t, sources: [...t.sources, e.source] }));
        } else if (m.type === "done") {
          updateLast((t) => ({ ...t, message: m.reply.message, sources: m.reply.sources, streaming: false }));
          onStatus?.("Answer ready");
          finish();
        } else {
          updateLast((t) => ({ ...t, streaming: false, error: m.message, needsSignIn: m.needs_sign_in }));
          onStatus?.("");
          finish();
        }
      };
      try {
        await api.askStart(id, assistantId, { system: opts.system, messages: [...history, user], web_search: opts.webSearch }, channel);
        onStatus?.("Thinking…");
      } catch (e) {
        updateLast((t) => ({ ...t, streaming: false, error: String(e) }));
        finish();
      }
    },
    [assistantId, turns, onStatus],
  );

  const stop = useCallback(() => {
    const id = requestId.current;
    if (!id) return;
    void api.askCancel(id);
    requestId.current = null;
    setBusy(false);
    updateLast((t) => ({ ...t, streaming: false, stopped: true }));
    onStatus?.("Stopped");
  }, [onStatus]);

  const reset = useCallback(() => {
    if (requestId.current) void api.askCancel(requestId.current);
    requestId.current = null;
    setBusy(false);
    setTurns([]);
  }, []);

  return { turns, busy, send, stop, reset };
}
