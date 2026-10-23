# Corvus

A keyboard-first desktop translator and AI assistant, built as a modern replacement for
[Crow Translate](https://github.com/crow-translate/crow-translate).

- **Rust core** (`crates/corvus-core`): provider trait, ordered fallback router with per-attempt
  timeouts, TOML config.
- **CLI** (`crates/corvus-cli`): `corvus translate -t vi "Hello"`, or pipe text via stdin.
- **Desktop app** (`src-tauri` + `ui`): Tauri 2, React 19, React Aria Components.

## Providers

| id (default config) | Kind | Needs | Enabled by default |
|---|---|---|---|
| `google` | `google-free` | nothing (unofficial endpoint) | yes |
| `deepl` | `deepl` | API key (keychain or `DEEPL_API_KEY`) | no |
| `ollama` | `openai-compatible` | a running Ollama, or any OpenAI-compatible `base_url` + `model` | no |

Everything below can be changed in **Settings** (Ctrl+, or the ⚙ button): default languages,
timeout, providers (add, reorder, enable, test), API keys and global shortcuts (press the new
keys to record them). Changes apply immediately, without a restart.

API keys are stored in the OS keychain (Secret Service / KWallet / GNOME Keyring, Windows
Credential Manager, macOS Keychain) under service `corvus`, account = provider id:

```sh
corvus key set deepl      # paste the key; read from stdin, not saved in shell history
corvus key delete deepl
```

Lookup order: `api_key` in the config file (fallback when no keychain is available), then the
keychain, then the environment variable named by `api_key_env`.

The config is created at `~/.config/corvus/config.toml` on first run. Providers are tried in
file order; when one fails or times out, the next is used.

```toml
[[providers]]
id = "openrouter"
kind = "openai-compatible"
name = "OpenRouter"
base_url = "https://openrouter.ai/api/v1"
model = "anthropic/claude-sonnet-5.5"
api_key_env = "OPENROUTER_API_KEY"
```

## Keyboard

Global shortcuts are set in the `[shortcuts]` section of the config; an empty value disables one.
If a key is taken by another app, Corvus shows a warning instead of failing.

| Keys | Action |
|---|---|
| Ctrl+Alt+C | Show Corvus (global) |
| Ctrl+Alt+E | Translate the selected text in a popup (global) |
| Ctrl+Alt+R | Replace the selected text with its translation (global) |
| Ctrl+Enter | Translate now |
| Ctrl+R | Swap languages |
| Ctrl+Shift+C | Copy translation |
| Ctrl+Shift+X | Clear text |
| Ctrl+L | Focus source text |
| Alt+S / Alt+T | Choose source / target language |
| Alt+1 … Alt+9 | Switch provider |
| Ctrl+/ | Show all shortcuts |
| Ctrl+, | Settings (Ctrl+S saves, Esc closes) |

In the popup: Enter copies and closes, Esc closes, Ctrl+O opens the text in the main window,
Alt+T changes the target language.

### Desktops without global hotkeys (Wayland)

Bind these commands in your desktop's own shortcut settings; they act on the running instance:

```sh
corvus-app --translate-selection
corvus-app --replace-selection
corvus-app --hidden          # start in the tray
```

## Development

```sh
pnpm install
pnpm tauri dev        # desktop app with hot reload
cargo test --workspace
cargo run -p corvus-cli -- translate -t vi "Hello"
pnpm tauri build      # installers in target/release/bundle
```

Linux build deps: `webkit2gtk-4.1`, `libayatana-appindicator`, `librsvg`.
