import { Button } from "react-aria-components";
import { Copy } from "lucide-react";
import { api } from "../api";
import { Markdown } from "./Markdown";
import type { Turn } from "./useAsk";

function hostname(url: string) {
  try {
    return new URL(url).hostname;
  } catch {
    return url;
  }
}

export function TurnView({ turn, onSignIn }: { turn: Turn; onSignIn?: () => void }) {
  if (turn.message.role === "user") {
    return (
      <article className="turn turn-user">
        <h3 className="sr-only">You asked</h3>
        <p className="turn-user-text">{turn.message.text}</p>
      </article>
    );
  }
  const text = turn.message.text;
  return (
    <article className="turn turn-assistant" aria-busy={turn.streaming || undefined}>
      <h3 className="sr-only">Answer</h3>
      {turn.searches.length > 0 && (
        <p className="turn-searches">Searched: {turn.searches.map((q) => `“${q}”`).join(", ")}</p>
      )}
      {text ? <Markdown text={text} /> : turn.streaming && <p className="setting-hint">Thinking…</p>}
      {turn.stopped && <p className="setting-hint">Stopped.</p>}
      {turn.error && (
        <div role="alert" className="error">
          {turn.error}
          {turn.needsSignIn && onSignIn && (
            <Button className="btn" onPress={onSignIn}>
              Fix in Settings
            </Button>
          )}
        </div>
      )}
      {turn.sources.length > 0 && (
        <nav aria-label="Sources" className="sources">
          <ol>
            {turn.sources.map((s) => (
              <li key={s.url}>
                <a
                  href={s.url}
                  onClick={(e) => {
                    e.preventDefault();
                    void api.openUrl(s.url);
                  }}
                >
                  {s.title || hostname(s.url)}
                </a>{" "}
                <span className="source-host">{hostname(s.url)}</span>
              </li>
            ))}
          </ol>
        </nav>
      )}
      {!turn.streaming && text && (
        <div className="turn-actions">
          <Button className="btn" onPress={() => void navigator.clipboard.writeText(text)}>
            <Copy size={15} aria-hidden /> Copy
          </Button>
        </div>
      )}
    </article>
  );
}
