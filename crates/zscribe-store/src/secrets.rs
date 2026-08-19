use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{AeadCore, Aes256Gcm, Key, Nonce};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use thiserror::Error;
use ts_rs::TS;

const SERVICE: &str = "dev.theholyonez.zscribe";
const NONCE_LEN: usize = 12;
const KEY_LEN: usize = 32;

#[derive(Debug, Error)]
pub enum SecretError {
    #[error("keychain error: {0}")]
    Keyring(#[from] keyring::Error),
    #[error("could not access {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("the encrypted secret store is corrupt or was written with a different key")]
    Corrupt,
    #[error("stored secrets are not valid JSON: {0}")]
    Parse(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SecretBackend {
    Keychain,

    EncryptedFile,
}

impl SecretBackend {
    pub const fn description(self) -> &'static str {
        match self {
            SecretBackend::Keychain => "Stored in your system keychain.",
            SecretBackend::EncryptedFile => {
                "No system keychain was found, so keys are stored in an encrypted file in \
                 ZScribe's data directory. This protects against casual reading and against \
                 backups, but not against other software running as your user."
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct SecretStore {
    backend: SecretBackend,
    key_file: PathBuf,
    secrets_file: PathBuf,
}

impl SecretStore {
    pub fn new(key_file: PathBuf, secrets_file: PathBuf) -> Self {
        Self {
            backend: if keychain_is_usable() {
                SecretBackend::Keychain
            } else {
                tracing::warn!("no OS keychain available; falling back to encrypted file storage");
                SecretBackend::EncryptedFile
            },
            key_file,
            secrets_file,
        }
    }

    pub fn file_backed(key_file: PathBuf, secrets_file: PathBuf) -> Self {
        Self {
            backend: SecretBackend::EncryptedFile,
            key_file,
            secrets_file,
        }
    }

    pub fn backend(&self) -> SecretBackend {
        self.backend
    }

    pub fn set(&self, account: &str, secret: &str) -> Result<(), SecretError> {
        if secret.is_empty() {
            return self.delete(account);
        }
        match self.backend {
            SecretBackend::Keychain => {
                keyring::Entry::new(SERVICE, account)?.set_password(secret)?;
                Ok(())
            }
            SecretBackend::EncryptedFile => self.update_file(|map| {
                map.insert(account.to_owned(), secret.to_owned());
            }),
        }
    }

    pub fn get(&self, account: &str) -> Result<Option<String>, SecretError> {
        match self.backend {
            SecretBackend::Keychain => {
                match keyring::Entry::new(SERVICE, account)?.get_password() {
                    Ok(secret) => Ok(Some(secret)),
                    Err(keyring::Error::NoEntry) => Ok(None),
                    Err(err) => Err(err.into()),
                }
            }
            SecretBackend::EncryptedFile => Ok(self.read_file()?.remove(account)),
        }
    }

    pub fn has(&self, account: &str) -> bool {
        matches!(self.get(account), Ok(Some(_)))
    }

    pub fn delete(&self, account: &str) -> Result<(), SecretError> {
        match self.backend {
            SecretBackend::Keychain => {
                match keyring::Entry::new(SERVICE, account)?.delete_credential() {
                    Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
                    Err(err) => Err(err.into()),
                }
            }
            SecretBackend::EncryptedFile => self.update_file(|map| {
                map.remove(account);
            }),
        }
    }

    fn update_file(
        &self,
        mutate: impl FnOnce(&mut BTreeMap<String, String>),
    ) -> Result<(), SecretError> {
        let mut map = self.read_file()?;
        mutate(&mut map);
        self.write_file(&map)
    }

    fn read_file(&self) -> Result<BTreeMap<String, String>, SecretError> {
        let blob = match std::fs::read(&self.secrets_file) {
            Ok(blob) => blob,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
            Err(source) => {
                return Err(SecretError::Io {
                    path: self.secrets_file.display().to_string(),
                    source,
                })
            }
        };

        if blob.len() <= NONCE_LEN {
            return Err(SecretError::Corrupt);
        }
        let (nonce, ciphertext) = blob.split_at(NONCE_LEN);

        let key = self.load_or_create_key()?;
        let plaintext = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key))
            .decrypt(Nonce::from_slice(nonce), ciphertext)
            .map_err(|_| SecretError::Corrupt)?;

        Ok(serde_json::from_slice(&plaintext)?)
    }

    fn write_file(&self, map: &BTreeMap<String, String>) -> Result<(), SecretError> {
        let key = self.load_or_create_key()?;
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let plaintext = serde_json::to_vec(map)?;

        let ciphertext = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key))
            .encrypt(&nonce, plaintext.as_slice())
            .map_err(|_| SecretError::Corrupt)?;

        let mut blob = nonce.to_vec();
        blob.extend_from_slice(&ciphertext);

        write_private(&self.secrets_file, &blob)
    }

    fn load_or_create_key(&self) -> Result<[u8; KEY_LEN], SecretError> {
        match std::fs::read(&self.key_file) {
            Ok(bytes) if bytes.len() == KEY_LEN => {
                let mut key = [0u8; KEY_LEN];
                key.copy_from_slice(&bytes);
                Ok(key)
            }

            Ok(_) => Err(SecretError::Corrupt),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                let key: [u8; KEY_LEN] = Aes256Gcm::generate_key(&mut OsRng).into();
                write_private(&self.key_file, &key)?;
                Ok(key)
            }
            Err(source) => Err(SecretError::Io {
                path: self.key_file.display().to_string(),
                source,
            }),
        }
    }
}

fn write_private(path: &Path, bytes: &[u8]) -> Result<(), SecretError> {
    let io_err = |source: std::io::Error| SecretError::Io {
        path: path.display().to_string(),
        source,
    };

    std::fs::write(path, bytes).map_err(io_err)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).map_err(io_err)?;
    }

    Ok(())
}

fn keychain_is_usable() -> bool {
    match keyring::Entry::new(SERVICE, "__zscribe_probe__") {
        Ok(entry) => !matches!(
            entry.get_password(),
            Err(keyring::Error::PlatformFailure(_) | keyring::Error::NoStorageAccess(_))
        ),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (tempfile::TempDir, SecretStore) {
        let dir = tempfile::tempdir().expect("temp dir");
        let store = SecretStore::file_backed(
            dir.path().join("secrets.key"),
            dir.path().join("secrets.enc"),
        );
        (dir, store)
    }

    #[test]
    fn round_trips_a_secret() {
        let (_dir, store) = store();
        store.set("gemini", "AIzaSecret").expect("set");
        assert_eq!(
            store.get("gemini").expect("get").as_deref(),
            Some("AIzaSecret")
        );
        assert!(store.has("gemini"));
    }

    #[test]
    fn an_account_that_was_never_set_reads_as_none() {
        let (_dir, store) = store();
        assert_eq!(store.get("never-set").expect("get"), None);
        assert!(!store.has("never-set"));
    }

    #[test]
    fn accounts_do_not_leak_into_each_other() {
        let (_dir, store) = store();
        store.set("gemini", "one").expect("set");
        store.set("openai-compatible", "two").expect("set");

        assert_eq!(store.get("gemini").expect("get").as_deref(), Some("one"));
        assert_eq!(
            store.get("openai-compatible").expect("get").as_deref(),
            Some("two")
        );
    }

    #[test]
    fn setting_an_empty_key_clears_it_rather_than_storing_a_blank() {
        let (_dir, store) = store();
        store.set("gemini", "AIzaSecret").expect("set");
        store.set("gemini", "").expect("clear");
        assert_eq!(store.get("gemini").expect("get"), None);
    }

    #[test]
    fn deleting_something_absent_is_not_an_error() {
        let (_dir, store) = store();
        store.delete("never-set").expect("delete");
    }

    #[test]
    fn the_secrets_file_never_contains_the_key_in_the_clear() {
        let (dir, store) = store();
        store.set("gemini", "AIzaVerySecret").expect("set");

        let blob = std::fs::read(dir.path().join("secrets.enc")).expect("read");
        let haystack = String::from_utf8_lossy(&blob);
        assert!(!haystack.contains("AIzaVerySecret"));
        assert!(!haystack.contains("gemini"));
    }

    #[cfg(unix)]
    #[test]
    fn the_key_and_secrets_files_are_readable_only_by_their_owner() {
        use std::os::unix::fs::PermissionsExt;

        let (dir, store) = store();
        store.set("gemini", "AIzaSecret").expect("set");

        for name in ["secrets.key", "secrets.enc"] {
            let mode = std::fs::metadata(dir.path().join(name))
                .expect("stat")
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600, "{name}");
        }
    }

    #[test]
    fn a_truncated_store_is_reported_as_corrupt_rather_than_read_as_empty() {
        let (dir, store) = store();
        store.set("gemini", "AIzaSecret").expect("set");
        std::fs::write(dir.path().join("secrets.enc"), b"short").expect("truncate");

        assert!(matches!(store.get("gemini"), Err(SecretError::Corrupt)));
    }

    #[test]
    fn a_store_written_under_a_different_key_does_not_decrypt_to_nonsense() {
        let (dir, store) = store();
        store.set("gemini", "AIzaSecret").expect("set");

        std::fs::write(dir.path().join("secrets.key"), [7u8; KEY_LEN]).expect("swap key");

        assert!(matches!(store.get("gemini"), Err(SecretError::Corrupt)));
    }

    #[test]
    fn a_key_file_of_the_wrong_length_is_refused_rather_than_padded() {
        let (dir, store) = store();
        std::fs::write(dir.path().join("secrets.key"), b"tooshort").expect("write");
        assert!(matches!(
            store.set("gemini", "x"),
            Err(SecretError::Corrupt)
        ));
    }

    #[test]
    fn the_backend_is_reported_so_the_ui_can_explain_where_keys_live() {
        let (_dir, store) = store();
        assert_eq!(store.backend(), SecretBackend::EncryptedFile);
        assert!(store.backend().description().contains("encrypted file"));
    }
}
