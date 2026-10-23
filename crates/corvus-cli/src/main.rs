use std::io::{IsTerminal, Read};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use corvus_core::{lang, Config, Keychain, Router, RouterError, SecretStore, TranslateRequest};

#[derive(Parser)]
#[command(name = "corvus", version, about = "Translate text from the command line")]
struct Cli {
    /// Config file (default: ~/.config/corvus/config.toml)
    #[arg(long, global = true)]
    config: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Translate text given as arguments, or read from stdin
    Translate {
        /// Source language code, or "auto"
        #[arg(short, long)]
        source: Option<String>,
        /// Target language code
        #[arg(short, long)]
        target: Option<String>,
        /// Provider id to try first
        #[arg(short, long)]
        provider: Option<String>,
        /// Print the full result as JSON
        #[arg(long)]
        json: bool,
        text: Vec<String>,
    },
    /// List enabled providers
    Providers,
    /// List language codes
    Languages,
    /// Manage provider API keys in the OS keychain
    Key {
        #[command(subcommand)]
        action: KeyAction,
    },
}

#[derive(Subcommand)]
enum KeyAction {
    /// Store a key, read from stdin so it stays out of shell history
    Set { provider: String },
    /// Remove a stored key
    Delete { provider: String },
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    let Some(path) = cli.config.or_else(Config::default_path) else {
        eprintln!("error: cannot determine config directory; pass --config");
        return ExitCode::FAILURE;
    };
    let config = match Config::load_or_init(&path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };
    if let Command::Key { action } = &cli.command {
        return manage_key(&config, action);
    }
    let (router, warnings) = Router::from_config(&config, &Keychain);
    for w in &warnings {
        eprintln!("warning: {w}");
    }

    match cli.command {
        Command::Providers => {
            for p in router.providers() {
                println!("{:<12} {:<20} {}", p.id, p.kind, p.name);
            }
        }
        Command::Languages => {
            for l in lang::LANGUAGES {
                println!("{:<6} {}", l.code, l.name);
            }
        }
        Command::Key { .. } => unreachable!("handled above"),
        Command::Translate { source, target, provider, json, text } => {
            let text = if text.is_empty() {
                if std::io::stdin().is_terminal() {
                    eprintln!("error: no text given (pass it as arguments or pipe it in)");
                    return ExitCode::FAILURE;
                }
                let mut buf = String::new();
                if let Err(e) = std::io::stdin().read_to_string(&mut buf) {
                    eprintln!("error: reading stdin: {e}");
                    return ExitCode::FAILURE;
                }
                buf
            } else {
                text.join(" ")
            };
            let req = TranslateRequest::new(
                text,
                source.unwrap_or(config.default_source),
                target.unwrap_or(config.default_target),
            );
            match router.translate(&req, provider.as_deref()).await {
                Ok(t) if json => println!("{}", serde_json::to_string_pretty(&t).unwrap()),
                Ok(t) => println!("{}", t.text),
                Err(RouterError::AllFailed(attempts)) => {
                    eprintln!("error: all providers failed");
                    for a in attempts {
                        eprintln!("  {}: {}", a.provider, a.error);
                    }
                    return ExitCode::FAILURE;
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    return ExitCode::FAILURE;
                }
            }
        }
    }
    ExitCode::SUCCESS
}

fn manage_key(config: &Config, action: &KeyAction) -> ExitCode {
    let (KeyAction::Set { provider } | KeyAction::Delete { provider }) = action;
    if !config.providers.iter().any(|p| &p.id == provider) {
        eprintln!("error: no provider with id \"{provider}\" (see `corvus providers`)");
        return ExitCode::FAILURE;
    }
    let result = match action {
        KeyAction::Set { .. } => {
            if std::io::stdin().is_terminal() {
                eprintln!("Paste the API key, then press Enter:");
            }
            let mut key = String::new();
            match std::io::stdin().read_line(&mut key) {
                Ok(_) if !key.trim().is_empty() => Keychain.set(provider, key.trim()),
                Ok(_) => Err("empty key".into()),
                Err(e) => Err(e.to_string()),
            }
        }
        KeyAction::Delete { .. } => Keychain.delete(provider),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
