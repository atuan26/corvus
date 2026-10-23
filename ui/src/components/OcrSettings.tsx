import { useEffect, useState } from "react";
import { Checkbox, CheckboxGroup, Label } from "react-aria-components";
import { api, type OcrConfig } from "../api";

/** Tesseract pack names → readable names for the common ones. */
const NAMES: Record<string, string> = {
  eng: "English", vie: "Vietnamese", jpn: "Japanese", kor: "Korean", chi_sim: "Chinese (Simplified)",
  chi_tra: "Chinese (Traditional)", fra: "French", deu: "German", spa: "Spanish", rus: "Russian",
  tha: "Thai", ita: "Italian", por: "Portuguese", ara: "Arabic", hin: "Hindi", ind: "Indonesian",
};

interface Props {
  value: OcrConfig;
  onChange: (ocr: OcrConfig) => void;
}

export function OcrSettings({ value, onChange }: Props) {
  const [installed, setInstalled] = useState<string[] | null>(null);
  const [error, setError] = useState("");
  useEffect(() => {
    api.ocrLanguages().then(setInstalled).catch((e) => setError(String(e)));
  }, []);

  if (error) {
    return (
      <p className="notice" role="note">
        {error}
      </p>
    );
  }
  if (!installed) return <p className="setting-hint">Checking Tesseract…</p>;

  // An empty list in the config means "all installed"; show that as everything checked.
  const selected = value.languages.length ? value.languages : installed;
  return (
    <>
      <CheckboxGroup
        className="checkbox-group"
        value={selected}
        onChange={(langs) => onChange({ ...value, languages: langs.length === installed.length ? [] : langs })}
      >
        <Label className="setting-label">Languages to recognize</Label>
        {installed.map((code) => (
          <Checkbox key={code} value={code} className="checkbox">
            <span className="checkbox-box" aria-hidden />
            {NAMES[code] ?? code} <span className="provider-id">({code})</span>
          </Checkbox>
        ))}
      </CheckboxGroup>
      <p className="setting-hint">
        Recognition runs locally with Tesseract. Fewer languages is faster and more accurate.
        {!installed.includes("vie") && " For Vietnamese, install the tesseract-data-vie package (other languages: tesseract-data-<code>)."}
      </p>
    </>
  );
}
