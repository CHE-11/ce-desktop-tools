//! PIN-based encryption shared by vault files.
//!
//! The PIN is never stored. A slow key derivation turns it into an AES-256-GCM
//! key, and only the ciphertext is written to disk.

use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use argon2::{Algorithm, Argon2, Params, Version};
use base64::Engine;
use rand::RngCore;
use serde::{Deserialize, Serialize};

const KDF: &str = "argon2id";
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;
const KEY_LEN: usize = 32;

#[derive(Clone)]
pub struct VaultKey {
    pub key: [u8; KEY_LEN],
    pub salt: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SealedFile {
    version: u32,
    encrypted: bool,
    kdf: String,
    salt: String,
    nonce: String,
    ciphertext: String,
}

pub fn check_pin(pin: &str) -> Result<(), String> {
    let length = pin.chars().count();
    if (4..=64).contains(&length) && pin.chars().all(|ch| !ch.is_control()) {
        return Ok(());
    }
    Err("Use a PIN of 4 to 64 characters.".into())
}

pub fn seal_bytes(plain: &[u8], pin: &str) -> Result<(String, VaultKey), String> {
    check_pin(pin)?;
    let mut salt = vec![0u8; SALT_LEN];
    rand::rngs::OsRng.fill_bytes(&mut salt);
    let key = derive(pin, &salt)?;
    let vault_key = VaultKey { key, salt };
    let raw = seal_bytes_with_key(plain, &vault_key)?;
    Ok((raw, vault_key))
}

pub fn seal_bytes_with_key(plain: &[u8], vault_key: &VaultKey) -> Result<String, String> {
    let mut nonce_bytes = [0u8; NONCE_LEN];
    rand::rngs::OsRng.fill_bytes(&mut nonce_bytes);
    let cipher = Aes256Gcm::new_from_slice(&vault_key.key)
        .map_err(|_| "Couldn't encrypt the vault file.".to_string())?;
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce_bytes), plain)
        .map_err(|_| "Couldn't encrypt the vault file.".to_string())?;
    let sealed = SealedFile {
        version: 1,
        encrypted: true,
        kdf: KDF.to_string(),
        salt: encode(&vault_key.salt),
        nonce: encode(&nonce_bytes),
        ciphertext: encode(&ciphertext),
    };
    let mut raw = serde_json::to_vec_pretty(&sealed)
        .map_err(|error| format!("Couldn't encrypt the vault file: {error}"))?;
    raw.push(b'\n');
    String::from_utf8(raw).map_err(|_| "Couldn't encrypt the vault file.".to_string())
}

pub fn open_bytes(raw: &str, pin: &str) -> Result<(Vec<u8>, VaultKey), String> {
    check_pin(pin)?;
    let sealed = sealed_from(raw)?;
    if sealed.kdf != KDF {
        return Err("This vault uses an unsupported lock.".into());
    }
    let salt = decode(&sealed.salt)?;
    let key = derive(pin, &salt)?;
    let plain = decrypt(&sealed, &key)?;
    Ok((plain, VaultKey { key, salt }))
}

pub fn open_bytes_with_key(raw: &str, vault_key: &VaultKey) -> Result<Vec<u8>, String> {
    let sealed = sealed_from(raw)?;
    decrypt(&sealed, &vault_key.key)
}

fn sealed_from(raw: &str) -> Result<SealedFile, String> {
    serde_json::from_str(raw).map_err(|_| "Couldn't read the encrypted vault.".to_string())
}

fn decrypt(sealed: &SealedFile, key: &[u8; KEY_LEN]) -> Result<Vec<u8>, String> {
    let nonce = decode(&sealed.nonce)?;
    let ciphertext = decode(&sealed.ciphertext)?;
    if nonce.len() != NONCE_LEN {
        return Err("Couldn't read the encrypted vault.".into());
    }
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|_| "Couldn't read the encrypted vault.".to_string())?;
    cipher
        .decrypt(Nonce::from_slice(&nonce), ciphertext.as_ref())
        .map_err(|_| "That PIN doesn't unlock this vault.".to_string())
}

fn derive(pin: &str, salt: &[u8]) -> Result<[u8; KEY_LEN], String> {
    let params = Params::new(19_456, 2, 1, Some(KEY_LEN))
        .map_err(|_| "Couldn't unlock the vault.".to_string())?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = [0u8; KEY_LEN];
    argon
        .hash_password_into(pin.as_bytes(), salt, &mut key)
        .map_err(|_| "Couldn't unlock the vault.".to_string())?;
    Ok(key)
}

fn encode(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn decode(value: &str) -> Result<Vec<u8>, String> {
    base64::engine::general_purpose::STANDARD
        .decode(value)
        .map_err(|_| "Couldn't read the encrypted vault.".to_string())
}
