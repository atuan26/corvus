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

/// Secrets in a JSON file readable only by the user (0600), for systems without a keychain.
pub struct FileStore {
    path: std::path::PathBuf,
    lock: Mutex<()>,
}

impl FileStore {
    pub fn new(path: std::path::PathBuf) -> Self {
        Self { path, lock: Mutex::new(()) }
    }

    pub fn default_path() -> Option<std::path::PathBuf> {
        dirs::data_dir().map(|d| d.join("corvus").join("secrets.json"))
    }

    fn read(&self) -> Result<HashMap<String, String>, String> {
        match std::fs::read_to_string(&self.path) {
            Ok(text) => serde_json::from_str(&text).map_err(|e| format!("{}: {e}", self.path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(HashMap::new()),
            Err(e) => Err(format!("{}: {e}", self.path.display())),
        }
    }

    fn write(&self, map: &HashMap<String, String>) -> Result<(), String> {
        let err = |e: std::io::Error| format!("{}: {e}", self.path.display());
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir).map_err(err)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
        let mut file = options.open(&tmp).map_err(err)?;
        std::io::Write::write_all(&mut file, serde_json::to_string(map).map_err(|e| e.to_string())?.as_bytes()).map_err(err)?;
        std::fs::rename(&tmp, &self.path).map_err(err)
    }
}

impl SecretStore for FileStore {
    fn get(&self, provider_id: &str) -> Result<Option<String>, String> {
        let _guard = self.lock.lock().unwrap();
        Ok(self.read()?.get(provider_id).cloned())
    }

    fn set(&self, provider_id: &str, secret: &str) -> Result<(), String> {
        let _guard = self.lock.lock().unwrap();
        let mut map = self.read()?;
        map.insert(provider_id.into(), secret.into());
        self.write(&map)
    }

    fn delete(&self, provider_id: &str) -> Result<(), String> {
        let _guard = self.lock.lock().unwrap();
        let mut map = self.read()?;
        if map.remove(provider_id).is_some() {
            self.write(&map)?;
        }
        Ok(())
    }
}

/// The keychain when it works, otherwise the private file. Reads check both so a secret saved
/// while the keychain was down is still found later.
pub struct KeychainOrFile {
    pub file: FileStore,
}

impl KeychainOrFile {
    /// Where new secrets go: "keychain" or "file".
    pub fn location(&self) -> &'static str {
        if Keychain::probe().is_ok() { "keychain" } else { "file" }
    }
}

impl SecretStore for KeychainOrFile {
    fn get(&self, id: &str) -> Result<Option<String>, String> {
        match Keychain.get(id) {
            Ok(Some(s)) => Ok(Some(s)),
            _ => self.file.get(id),
        }
    }

    fn set(&self, id: &str, secret: &str) -> Result<(), String> {
        match Keychain.set(id, secret) {
            Ok(()) => {
                let _ = self.file.delete(id);
                Ok(())
            }
            Err(e) => {
                tracing::info!("{e}; saving to {}", self.file.path.display());
                self.file.set(id, secret)
            }
        }
    }

    fn delete(&self, id: &str) -> Result<(), String> {
        let keychain = Keychain.delete(id);
        self.file.delete(id)?;
        keychain.or(Ok(()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_store_round_trip_is_private() {
        let dir = std::env::temp_dir().join(format!("corvus-secrets-{}", std::process::id()));
        let store = FileStore::new(dir.join("secrets.json"));
        assert_eq!(store.get("a").unwrap(), None);
        store.set("a", "one").unwrap();
        store.set("b", "two").unwrap();
        store.delete("a").unwrap();
        assert_eq!(store.get("a").unwrap(), None);
        assert_eq!(store.get("b").unwrap().as_deref(), Some("two"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(dir.join("secrets.json")).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
}
