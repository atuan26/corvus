use std::io::{IsTerminal, Read};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use corvus_core::{lang, Config, Router, RouterError, TranslateRequest};

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
    let (router, warnings) = Router::from_config(&config);
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
