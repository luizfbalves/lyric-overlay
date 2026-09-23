use keyring::Entry;

const SERVICE: &str = "dev.luizfbalves.lyricoverlay";
const USER: &str = "deepl-api-key";

pub fn load_key() -> Option<String> {
    let entry = Entry::new(SERVICE, USER).ok()?;
    match entry.get_password() {
        Ok(k) if !k.trim().is_empty() => Some(k),
        Ok(_) | Err(keyring::Error::NoEntry) => None,
        Err(e) => {
            eprintln!("keyring (leitura): {e}");
            None
        }
    }
}

/// String vazia apaga a chave.
pub fn store_key(key: &str) -> Result<(), String> {
    let entry = Entry::new(SERVICE, USER).map_err(|e| e.to_string())?;
    let key = key.trim();
    if key.is_empty() {
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    } else {
        entry.set_password(key).map_err(|e| e.to_string())
    }
}
