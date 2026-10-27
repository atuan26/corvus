import { useEffect, useState } from "react";
import { Button } from "react-aria-components";
import { Volume2 } from "lucide-react";
import { api, type Ipa, type Translation } from "../api";
import { IconButton } from "./IconButton";

/** Longest text the speech endpoint reads. */
const SPEAK_MAX = 200;

/** Single words and short phrases get dictionary treatment; Wiktionary has nothing for sentences. */
export function isLookupSized(text: string) {
  const t = text.trim();
  return t.length > 0 && t.length <= 40 && !t.includes("\n") && t.split(/\s+/).length <= 3;
}

interface Props {
  text: string;
  /** Resolved source language (never "auto"), if known. */
  source: string | null;
  target: string;
  result: Translation;
  /** Fewer items, for the popup. */
  compact?: boolean;
  onUseCorrection?: (text: string) => void;
}

/** Pronunciation, listen buttons, and meanings by part of speech for the current translation. */
export function WordDetails({ text, source, target, result, compact, onUseCorrection }: Props) {
  const [ipa, setIpa] = useState<Ipa[]>([]);
  const [expanded, setExpanded] = useState(false);
  const details = result.details;

  useEffect(() => {
    setIpa([]);
    setExpanded(false);
    if (!source || !isLookupSized(text)) return;
    let current = true;
    api
      .ipa(text, source)
      .then((found) => current && setIpa(found))
      // IPA is a nicety; Google's phonetic spelling still shows.
      .catch(() => {});
    return () => {
      current = false;
    };
  }, [text, source]);

  const speak = (words: string, lang: string) =>
    // Nothing useful to show on failure; the button simply does nothing.
    api.speak(words, lang).catch(() => {});

  const corrected =
    details?.corrected && details.corrected.toLowerCase() !== text.trim().toLowerCase() ? details.corrected : null;
  const canSpeakSource = !!source && text.trim().length <= SPEAK_MAX;
  const canSpeakTarget = result.text.trim().length > 0 && result.text.trim().length <= SPEAK_MAX;
  const sourcePhonetic = ipa.length > 0 ? null : details?.source_phonetic;
  const meanings = (details?.meanings ?? []).filter(
    (m) => m.translations.length > 0 || m.senses.length > 0 || (!compact && m.synonyms.length > 0),
  );
  const full = !compact && expanded;
  const limits = compact
    ? { meanings: 2, translations: 4, senses: 1, synonyms: 0, examples: 0 }
    : full
      ? { meanings: Infinity, translations: Infinity, senses: Infinity, synonyms: Infinity, examples: Infinity }
      : { meanings: 3, translations: 8, senses: 2, synonyms: 8, examples: 3 };
  const truncated =
    meanings.length > limits.meanings ||
    meanings.some(
      (m) =>
        m.translations.length > limits.translations ||
        m.senses.length > limits.senses ||
        m.synonyms.length > limits.synonyms,
    ) ||
    (details?.examples.length ?? 0) > limits.examples;

  const hasPronunciation = canSpeakSource || canSpeakTarget || ipa.length > 0 || !!sourcePhonetic;
  if (!hasPronunciation && !corrected && meanings.length === 0) return null;

  return (
    <section className={`word-details ${compact ? "compact" : ""}`} aria-label="Dictionary">
      {corrected && (
        <p className="wd-correction">
          Did you mean{" "}
          {onUseCorrection ? (
            <Button className="link-btn" onPress={() => onUseCorrection(corrected)}>
              {corrected}
            </Button>
          ) : (
            <strong>{corrected}</strong>
          )}
          ?
        </p>
      )}

      <div className="wd-pronounce">
        <span className="wd-side">
          {canSpeakSource && (
            <IconButton
              className="quiet"
              label="Listen to the original"
              icon={<Volume2 />}
              onPress={() => void speak(text, source!)}
            />
          )}
          {ipa.slice(0, compact ? 1 : 3).map((p) => (
            <span key={p.text} className="wd-ipa" title={p.accent ? `IPA, ${p.accent}` : "IPA"}>
              {p.text}
              {!compact && p.accent && <small>{p.accent}</small>}
            </span>
          ))}
          {sourcePhonetic && (
            <span className="wd-phonetic" title="Pronunciation">
              {sourcePhonetic}
            </span>
          )}
        </span>
        {(canSpeakTarget || details?.target_phonetic) && (
          <span className="wd-side">
            {canSpeakTarget && (
              <IconButton
                className="quiet"
                label="Listen to the translation"
                icon={<Volume2 />}
                onPress={() => void speak(result.text, target)}
              />
            )}
            {details?.target_phonetic && (
              <span className="wd-phonetic" title="Pronunciation of the translation">
                {details.target_phonetic}
              </span>
            )}
          </span>
        )}
      </div>

      {meanings.slice(0, limits.meanings).map((m) => (
        <div key={m.pos} className="wd-meaning">
          {m.pos && <h3 className="wd-pos">{m.pos}</h3>}
          {m.translations.length > 0 && (
            <ul className="wd-terms" aria-label={`Translations as ${m.pos || "other"}`}>
              {m.translations.slice(0, limits.translations).map((t) => (
                <li key={t.word} className="chip" title={t.back.join(", ")}>
                  {t.word}
                </li>
              ))}
            </ul>
          )}
          {m.senses.length > 0 && limits.senses > 0 && (
            <ol className="wd-senses">
              {m.senses.slice(0, limits.senses).map((s) => (
                <li key={s.gloss}>
                  {s.gloss}
                  {s.example && <q className="wd-example">{s.example}</q>}
                </li>
              ))}
            </ol>
          )}
          {m.synonyms.length > 0 && limits.synonyms > 0 && (
            <p className="wd-synonyms">
              <span className="wd-label">Synonyms</span> {m.synonyms.slice(0, limits.synonyms).join(", ")}
            </p>
          )}
        </div>
      ))}

      {details && details.examples.length > 0 && limits.examples > 0 && (
        <div className="wd-meaning">
          <h3 className="wd-pos">Examples</h3>
          <ul className="wd-examples">
            {details.examples.slice(0, limits.examples).map((e) => (
              <li key={e}>{e}</li>
            ))}
          </ul>
        </div>
      )}

      {!compact && (truncated || expanded) && (
        <Button className="link-btn" onPress={() => setExpanded((x) => !x)}>
          {expanded ? "Show less" : "Show all"}
        </Button>
      )}
    </section>
  );
}
