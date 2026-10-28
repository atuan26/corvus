# Corvus

A keyboard-first desktop translator and AI assistant, built as a modern replacement for
[Crow Translate](https://github.com/crow-translate/crow-translate).

- **Rust core** (`crates/corvus-core`): provider trait, ordered fallback router with per-attempt
  timeouts, TOML config.
- **CLI** (`crates/corvus-cli`): `corvus translate -t vi "Hello"`, or pipe text via stdin.
- **Desktop app** (`src-tauri` + `ui`): Tauri 2, React 19, React Aria Components.

## Install

Download the package for your system from the
[Releases page](https://github.com/atuan26/corvus/releases).

| System | File | How |
|---|---|---|
| Windows 10/11 | `Corvus_<version>_x64-setup.exe` | Run it. Windows warns because the installer isn't signed yet: choose **More info → Run anyway**. The `.msi` is for managed (IT) installs. |
| Debian, Ubuntu | `Corvus_<version>_amd64.deb` | `sudo apt install ./Corvus_<version>_amd64.deb` |
| Fedora, openSUSE | `Corvus-<version>-1.x86_64.rpm` | `sudo dnf install ./Corvus-<version>-1.x86_64.rpm` |
| Other Linux | `Corvus_<version>_amd64.AppImage` | `chmod +x Corvus_*.AppImage`, then run it |
| macOS | `Corvus_<version>_universal.dmg` | Open it and drag Corvus to Applications. The app isn't notarized yet: the first time, right-click it and choose **Open**. |

The Windows installer can install for just you (no admin rights) or for all users, and its last
page offers a desktop shortcut and **Start Corvus when I sign in**. Uninstall from Windows'
*Installed apps*.

Optional extras:

- **Screen text (OCR)** needs [Tesseract](#screen-text-ocr); Settings → Screen text installs it
  and downloads languages.
- **Listening to pronunciations** on Linux needs PulseAudio or PipeWire (with `pipewire-pulse`),
  which desktop distributions already run.
- On Wayland desktops global shortcuts don't work; [bind the commands](#desktops-without-global-hotkeys-wayland)
  in your desktop's settings instead.

Corvus starts in the tray. Its settings are in `~/.config/corvus/config.toml` on Linux,
`%APPDATA%\corvus\config.toml` on Windows, and
`~/Library/Application Support/corvus/config.toml` on macOS.

## Screen text (OCR)

**Ctrl+Alt+O** freezes the screen under the mouse and lets you choose an area: drag over it, or
use the keyboard (arrows move, Shift+arrows resize, Alt for 1 px steps, Ctrl+A whole screen,
Enter confirms, Esc or right-click cancels). The text is recognized locally with Tesseract and
translated in the popup. **Ctrl+Alt+I** does the same but copies the text and opens it in the main
window, keeping its line breaks.

Inside the app, the scan button in the Translate and Ask boxes does the same and puts the text
straight into that box.

Recognition uses [Tesseract](https://github.com/tesseract-ocr/tesseract). **Settings → Screen
text** installs it if it's missing:

- **Windows**: downloads the official [UB Mannheim installer](https://github.com/UB-Mannheim/tesseract/wiki),
  checks it against a pinned SHA-256, and opens it (Windows asks for permission).
- **macOS**: runs `brew install tesseract` when [Homebrew](https://brew.sh) is installed.
- **Linux**: runs the package manager (apt, dnf, pacman or zypper) through `pkexec`, which asks
  for your password; without `pkexec` it shows the command to run.

Languages are added in the same place: pick one under **Add a language** and Corvus downloads its
data from Tesseract's official `tessdata_fast` set into `~/.local/share/corvus/tessdata`
(`%APPDATA%\corvus\tessdata` on Windows), with no admin rights needed. Languages installed with
the system's packages are used too. From the command line: `corvus ocr shot.png [--translate]`.

A Tesseract outside the usual places can be set in the config (`[ocr] tesseract = "..."`).

## Ask AI

The **Ask AI** tab (Ctrl+2) is a chat with streaming answers, optional web search with source
links, Esc to stop and Ctrl+N for a new chat. **Ctrl+Alt+A** opens a popup for the selected text in
any app: press 1–6 for Explain, Summarize, Fix grammar, Translate & explain, Write a reply or Make
it formal, ask a follow-up, then Enter to copy, **Ctrl+R to replace the selection**, or Ctrl+O to
continue in the main window.

Assistants are set up in Settings → AI assistants:

| Kind | Sign-in | Web search |
|---|---|---|
| ChatGPT (beta) | **Sign in with ChatGPT** in the browser, or with a code | yes |
| Claude | Anthropic API key (default model `claude-opus-5-5`) | yes |
| OpenAI API | API key | yes |
| OpenAI-compatible (Ollama, LM Studio, Groq, …) | base URL, model, optional key | no |

"Sign in with ChatGPT" uses the same OAuth flow as OpenAI's Codex CLI so a ChatGPT plan can be
used instead of an API key. It is not a published OpenAI API and may change. Tokens are stored in
the system keychain, or without one in a private file (`~/.local/share/corvus/secrets.json`, mode
600).

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
| Ctrl+Alt+A | Ask AI about the selected text (global) |
| Ctrl+Alt+O / Ctrl+Alt+I | Translate / copy text in a screen area (global) |
| Ctrl+1 / Ctrl+2 | Translate / Ask AI tab |
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
corvus-app --ask-selection
corvus-app --ocr-translate
corvus-app --ocr-copy
corvus-app --hidden          # start in the tray
```

## Building from source

### Requirements

- [Rust](https://rustup.rs) (stable) and [Node.js](https://nodejs.org) 24 with
  [pnpm](https://pnpm.io) 11.
- **Linux** system libraries:
  - Debian/Ubuntu: `sudo apt install libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libasound2-dev libdbus-1-dev patchelf`
  - Arch: `sudo pacman -S webkit2gtk-4.1 libayatana-appindicator librsvg alsa-lib`
  - Fedora: `sudo dnf install webkit2gtk4.1-devel libappindicator-gtk3-devel librsvg2-devel alsa-lib-devel dbus-devel`
- **Windows**: the "Desktop development with C++" workload from Visual Studio Build Tools.
  WebView2 is already part of Windows 10/11.
- **macOS**: Xcode Command Line Tools (`xcode-select --install`).

### Run and test

```sh
pnpm install
pnpm tauri dev                                  # desktop app with hot reload
cargo test --workspace                          # Rust tests (OCR tests skip without Tesseract)
cargo clippy --workspace --all-targets
pnpm build                                      # type-check and build the UI
cargo run -p corvus-cli -- translate -t vi "Hello"
```

`pnpm tauri dev` rebuilds and restarts the app when Rust code changes; UI changes reload in
place.

### Packages

```sh
pnpm tauri build                                # every package format this system can make
pnpm tauri build --bundles deb,rpm,appimage     # Linux
pnpm tauri build --bundles nsis,msi             # Windows
pnpm tauri build --target universal-apple-darwin --bundles dmg   # macOS (Intel + Apple Silicon)
```

Packages land in `target/release/bundle/` (or `target/<target>/release/bundle/` with
`--target`). The CLI is a separate binary: `cargo build --release -p corvus-cli` gives
`target/release/corvus`.

The Windows setup `.exe` can also be cross-built on Linux with the `x86_64-pc-windows-gnu` Rust
target, MinGW (`mingw-w64-gcc`) and `makensis` on `PATH`:

```sh
rustup target add x86_64-pc-windows-gnu
pnpm tauri build --target x86_64-pc-windows-gnu --bundles nsis
```

The `.msi` needs WiX, which only runs on Windows.

The installer's finish page ("Start Corvus when I sign in") comes from
`src-tauri/windows/installer.nsi`, a copy of Tauri's NSIS template with marked additions;
re-copy it from the new `tauri-cli` version and re-apply those parts when upgrading
`@tauri-apps/cli`.

### Releases

The **Build** workflow (`.github/workflows/build.yml`) tests on Linux and builds the packages for
Linux, Windows and macOS on every pull request; download them from the run's artifacts. To
release:

1. Set the version in `Cargo.toml`, `package.json` and `src-tauri/tauri.conf.json`, and add the
   changes to `CHANGELOG.md`.
2. Tag and push: `git tag v0.1.0 && git push origin v0.1.0`.
3. The workflow attaches the packages to a **draft** release; check it and publish it.

## License

Licensed under either of [Apache License 2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT), at your
option.
