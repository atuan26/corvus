# Changelog

## 0.1.0

First release.

### Translate

- Translate with Google (free), DeepL, or any OpenAI-compatible service (OpenAI, OpenRouter,
  Groq, Ollama, LM Studio), tried in order with automatic fallback when one fails or is slow.
- Dictionary details for words and short phrases: IPA from Wiktionary, pronunciation audio,
  meanings by part of speech, definitions, examples, synonyms and "Did you mean" spelling fixes.
- Translate the selected text in any app in a popup (Ctrl+Alt+E), or replace it with its
  translation (Ctrl+Alt+R).
- Screen text (OCR): pick a screen area to translate (Ctrl+Alt+O) or copy (Ctrl+Alt+I) its text,
  recognized locally with Tesseract. Settings installs Tesseract and downloads OCR languages.

### Ask AI

- Chat with streaming answers, optional web search with sources, and a popup for the selected
  text with quick actions (explain, summarize, fix grammar, reply, make formal).
- Assistants: ChatGPT through "Sign in with ChatGPT" (beta), Claude, OpenAI, and
  OpenAI-compatible services.

### App

- Keyboard-first: every action has a shortcut, and global shortcuts can be changed in Settings.
- Settings for languages, providers, API keys (kept in the system keychain), assistants, OCR and
  shortcuts, applied without a restart.
- Appearance: System, Light or Dark mode, and six color themes.
- Start at sign-in, hidden in the tray.
- Windows installer for one user or all users, with Start menu, desktop shortcut and start-at-sign-in
  options.
- `corvus` command-line tool for translating, OCR and managing keys.

### Known limitations

- Google translation and pronunciation audio use unofficial free endpoints that can be
  rate-limited or change; add another provider as a fallback.
- Global shortcuts don't work on Wayland; bind the `corvus-app --…` commands in your desktop's
  settings instead.
- The Windows installer and macOS app aren't signed yet, so the system warns on first run.
