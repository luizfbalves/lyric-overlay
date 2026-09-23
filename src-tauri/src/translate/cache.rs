use crate::player::TrackKey;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CachedTranslation {
    pub source_lang: String,
    /// `None` = mesma língua do alvo, não precisa traduzir.
    pub lines: Option<Vec<String>>,
}

/// FNV-1a 64 bits: estável entre versões do Rust (ao contrário do DefaultHasher).
pub fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

pub struct DiskCache {
    dir: PathBuf,
}

impl DiskCache {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    pub fn path(&self, key: &TrackKey, target: &str) -> PathBuf {
        let h = fnv1a(format!("{}#{}", key.id(), target).as_bytes());
        self.dir.join(format!("{h:016x}.json"))
    }

    pub fn get(&self, key: &TrackKey, target: &str) -> Option<CachedTranslation> {
        let s = std::fs::read_to_string(self.path(key, target)).ok()?;
        serde_json::from_str(&s).ok()
    }

    pub fn put(&self, key: &TrackKey, target: &str, v: &CachedTranslation) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        let bytes = serde_json::to_vec(v).map_err(std::io::Error::other)?;
        std::fs::write(self.path(key, target), bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> TrackKey {
        TrackKey { artist: "Banda Fictícia".into(), title: "Canção Teste".into(), duration_s: 180 }
    }

    #[test]
    fn fnv_known_vectors() {
        assert_eq!(fnv1a(b""), 0xcbf29ce484222325);
        assert_eq!(fnv1a(b"a"), 0xaf63dc4c8601ec8c);
    }

    #[test]
    fn miss_then_hit() {
        let d = tempfile::tempdir().unwrap();
        let c = DiskCache::new(d.path().join("translations"));
        assert_eq!(c.get(&key(), "PT-BR"), None);
        let v = CachedTranslation { source_lang: "JA".into(), lines: Some(vec!["tr:um".into(), "".into()]) };
        c.put(&key(), "PT-BR", &v).unwrap();
        assert_eq!(c.get(&key(), "PT-BR"), Some(v));
    }

    #[test]
    fn target_is_part_of_key() {
        let d = tempfile::tempdir().unwrap();
        let c = DiskCache::new(d.path().to_path_buf());
        c.put(&key(), "PT-BR", &CachedTranslation { source_lang: "PT".into(), lines: None }).unwrap();
        assert_eq!(c.get(&key(), "ES"), None);
        assert_eq!(c.get(&key(), "PT-BR").unwrap().lines, None);
    }

    #[test]
    fn corrupt_file_is_miss() {
        let d = tempfile::tempdir().unwrap();
        let c = DiskCache::new(d.path().to_path_buf());
        std::fs::write(c.path(&key(), "PT-BR"), "lixo").unwrap();
        assert_eq!(c.get(&key(), "PT-BR"), None);
    }
}
