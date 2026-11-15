//! PIN-based encryption for expansion.json.

use super::store::{ExpansionFile, HiddenSecret, Snippet};
pub use crate::vault_crypto::{VaultKey, check_pin};

pub fn seal(snippets: &[Snippet], pin: &str) -> Result<(String, VaultKey), String> {
    let plain = serde_json::to_vec(&ExpansionFile {
        snippets: snippets.to_vec(),
    })
    .map_err(|error| format!("Couldn't encrypt expansions: {error}"))?;
    crate::vault_crypto::seal_bytes(&plain, pin)
}

pub fn seal_with_key(snippets: &[Snippet], vault_key: &VaultKey) -> Result<String, String> {
    let plain = serde_json::to_vec(&ExpansionFile {
        snippets: snippets.to_vec(),
    })
    .map_err(|error| format!("Couldn't encrypt expansions: {error}"))?;
    crate::vault_crypto::seal_bytes_with_key(&plain, vault_key)
}

pub fn open(raw: &str, pin: &str) -> Result<(ExpansionFile, VaultKey), String> {
    let (plain, key) = crate::vault_crypto::open_bytes(raw, pin)?;
    let file = serde_json::from_slice(&plain)
        .map_err(|_| "Couldn't read the encrypted vault.".to_string())?;
    Ok((file, key))
}

pub fn open_with_key(raw: &str, vault_key: &VaultKey) -> Result<ExpansionFile, String> {
    let plain = crate::vault_crypto::open_bytes_with_key(raw, vault_key)?;
    serde_json::from_slice(&plain).map_err(|_| "Couldn't read the encrypted vault.".to_string())
}

pub fn seal_secrets(secrets: &[HiddenSecret], pin: &str) -> Result<(String, VaultKey), String> {
    let plain = serde_json::to_vec(secrets)
        .map_err(|error| format!("Couldn't encrypt hidden snippets: {error}"))?;
    crate::vault_crypto::seal_bytes(&plain, pin)
}

pub fn seal_secrets_with_key(
    secrets: &[HiddenSecret],
    vault_key: &VaultKey,
) -> Result<String, String> {
    let plain = serde_json::to_vec(secrets)
        .map_err(|error| format!("Couldn't encrypt hidden snippets: {error}"))?;
    crate::vault_crypto::seal_bytes_with_key(&plain, vault_key)
}

pub fn open_secrets(raw: &str, pin: &str) -> Result<(Vec<HiddenSecret>, VaultKey), String> {
    let (plain, key) = crate::vault_crypto::open_bytes(raw, pin)?;
    let secrets = serde_json::from_slice(&plain)
        .map_err(|_| "Couldn't read the encrypted vault.".to_string())?;
    Ok((secrets, key))
}

pub fn open_secrets_with_key(raw: &str, vault_key: &VaultKey) -> Result<Vec<HiddenSecret>, String> {
    let plain = crate::vault_crypto::open_bytes_with_key(raw, vault_key)?;
    serde_json::from_slice(&plain).map_err(|_| "Couldn't read the encrypted vault.".to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        VaultKey, check_pin, open, open_secrets_with_key, open_with_key, seal,
        seal_secrets_with_key, seal_with_key,
    };
    use crate::expansion::store::Snippet;

    fn sample() -> Snippet {
        Snippet {
            id: "a".into(),
            trigger: ";sig".into(),
            text: "Thanks".into(),
            enabled: true,
            hidden: false,
        }
    }

    #[test]
    fn a_pin_round_trips_the_snippets_and_a_wrong_pin_fails() {
        let (raw, key) = seal(&[sample()], "1234").unwrap();
        assert!(raw.contains("\"encrypted\": true"));
        assert!(!raw.contains("Thanks"));
        assert!(!raw.contains(";sig"));
        let (opened, _) = open(&raw, "1234").unwrap();
        assert_eq!(opened.snippets[0].text, "Thanks");
        assert!(open(&raw, "9999").is_err());
        let again = open_with_key(&seal_with_key(&opened.snippets, &key).unwrap(), &key).unwrap();
        assert_eq!(again.snippets[0].trigger, ";sig");
    }

    #[test]
    fn hidden_secrets_round_trip_without_the_public_text() {
        let secrets = vec![crate::expansion::store::HiddenSecret {
            id: "b".into(),
            text: "secret-value".into(),
        }];
        let key = VaultKey {
            key: [7u8; 32],
            salt: vec![1, 2, 3, 4],
        };
        let sealed = seal_secrets_with_key(&secrets, &key).unwrap();
        assert!(!sealed.contains("secret-value"));
        let opened = open_secrets_with_key(&sealed, &key).unwrap();
        assert_eq!(opened, secrets);
    }

    #[test]
    fn a_pin_must_be_digits() {
        assert!(check_pin("12").is_err());
        assert!(check_pin("12ab").is_ok());
        assert!(check_pin("1234").is_ok());
        assert!(check_pin("Ab!2").is_ok());
    }
}
