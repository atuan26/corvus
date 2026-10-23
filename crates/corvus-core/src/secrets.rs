//! API key storage. Keys live in the OS keychain (Secret Service, Windows Credential Manager,
//! macOS Keychain), keyed by provider id, so they never need to be written to the config file.

use std::collections::HashMap;
use std::sync::Mutex;

pub const SERVICE: &str = "corvus";

pub trait SecretStore: Send + Sync {
    fn get(&self, provider_id: &str) -> Result<Option<String>, String>;
    fn set(&self, provider_id: &str, secret: &str) -> Result<(), String>;
    fn delete(&self, provider_id: &str) -> Result<(), String>;
}

/// The platform keychain. Every call may block on IPC, so call it off the async runtime.
pub struct Keychain;

impl Keychain {
    /// Whether the keychain answers at all (e.g. a Secret Service is running on Linux).
    pub fn probe() -> Result<(), String> {
        Keychain.get("__corvus_probe__").map(|_| ())
    }

    fn entry(provider_id: &str) -> Result<keyring::Entry, String> {
        keyring::Entry::new(SERVICE, provider_id).map_err(|e| format!("keychain unavailable: {e}"))
    }
}

impl SecretStore for Keychain {
    fn get(&self, provider_id: &str) -> Result<Option<String>, String> {
        match Self::entry(provider_id)?.get_password() {
            Ok(s) => Ok(Some(s)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(format!("keychain read failed: {e}")),
        }
    }

    fn set(&self, provider_id: &str, secret: &str) -> Result<(), String> {
        Self::entry(provider_id)?.set_password(secret).map_err(|e| format!("keychain write failed: {e}"))
    }

    fn delete(&self, provider_id: &str) -> Result<(), String> {
        match Self::entry(provider_id)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(format!("keychain delete failed: {e}")),
        }
    }
}

/// In-memory store for tests and for running without a keychain.
#[derive(Default)]
pub struct MemoryStore(Mutex<HashMap<String, String>>);

impl SecretStore for MemoryStore {
    fn get(&self, provider_id: &str) -> Result<Option<String>, String> {
        Ok(self.0.lock().unwrap().get(provider_id).cloned())
    }

    fn set(&self, provider_id: &str, secret: &str) -> Result<(), String> {
        self.0.lock().unwrap().insert(provider_id.into(), secret.into());
        Ok(())
    }

    fn delete(&self, provider_id: &str) -> Result<(), String> {
        self.0.lock().unwrap().remove(provider_id);
        Ok(())
    }
}
