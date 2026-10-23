//! Provider-agnostic translation core shared by the desktop app and the CLI.

pub mod config;
pub mod error;
pub mod lang;
pub mod provider;
pub mod providers;
pub mod router;
pub mod secrets;

pub use config::{Config, KeySource, ProviderConfig, ProviderKind, Shortcuts};
pub use error::ProviderError;
pub use provider::{ProviderInfo, TranslateRequest, Translation, Translator};
pub use router::{Attempt, Router, RouterError};
pub use secrets::{Keychain, MemoryStore, SecretStore};
