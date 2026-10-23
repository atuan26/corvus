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
| `deepl` | `deepl` | `DEEPL_API_KEY` | no |
| `ollama` | `openai-compatible` | a running Ollama, or any OpenAI-compatible `base_url` + `model` | no |

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

| Keys | Action |
|---|---|
| Ctrl+Alt+C | Show Corvus (global) |
| Ctrl+Enter | Translate now |
| Ctrl+R | Swap languages |
| Ctrl+Shift+C | Copy translation |
| Ctrl+Shift+X | Clear text |
| Ctrl+L | Focus source text |
| Alt+S / Alt+T | Choose source / target language |
| Alt+1 … Alt+9 | Switch provider |
| Ctrl+/ | Show all shortcuts |

## Development

```sh
pnpm install
pnpm tauri dev        # desktop app with hot reload
cargo test --workspace
cargo run -p corvus-cli -- translate -t vi "Hello"
pnpm tauri build      # installers in target/release/bundle
```

Linux build deps: `webkit2gtk-4.1`, `libayatana-appindicator`, `librsvg`.
