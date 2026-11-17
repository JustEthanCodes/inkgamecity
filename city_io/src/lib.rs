//! city_io — save/load for City Floorlines (§5.1).
//!
//! File layout (`*.cflsave`):
//!   b"CFLR" | schema_version u32 LE | chunk_count u32 LE
//!   per chunk: name_len u8 | name bytes | payload_crc32 u32 LE | payload_len u64 LE | zstd(payload)
//!
//! Entity → id remap discipline and incremental journaling are documented in the design
//! doc (§5.1); this crate is the durable container plus the descriptor loader.

#![forbid(unsafe_code)]

use std::path::Path;

use serde::Serialize;
use thiserror::Error;

pub mod descriptor;

pub const MAGIC: &[u8; 4] = b"CFLR";
/// Bump on incompatible format change; add a migration step in `migrations` when doing so.
pub const SCHEMA_VERSION: u32 = 1;
pub const MAX_CHUNK_PAYLOAD: u64 = 512 * 1024 * 1024;
const ZSTD_LEVEL: i32 = 3;

#[derive(Error, Debug)]
pub enum IoError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("bad magic bytes — not a City Floorlines save")]
    BadMagic,
    #[error("unsupported schema version {0} (this build reads schema ≤ {SCHEMA_VERSION})")]
    BadVersion(u32),
    #[error("chunk `{0}` failed crc32 verification")]
    CrcMismatch(String),
    #[error("chunk `{0}` is missing from this save")]
    MissingChunk(String),
    #[error("chunk `{name}` exceeds maximum payload size ({len} bytes)")]
    ChunkTooLarge { name: String, len: u64 },
    #[error("serialization error: {0}")]
    Serde(String),
}

/// Object-safe bridge so heterogeneous chunk values can be encoded in one call.
pub trait ErasedChunk: std::any::Any {
    fn encode_zstd(&self) -> Vec<u8>;
}
impl<T: Serialize + 'static> ErasedChunk for T {
    fn encode_zstd(&self) -> Vec<u8> {
        let raw = bincode::serde::encode_to_vec(self, bincode::config::standard())
            .expect("save chunk serialization must succeed for registered Serialize types");
        zstd::encode_all(raw.as_slice(), ZSTD_LEVEL)
            .expect("zstd encoding must not fail")
    }
}

pub struct LoadedSave {
    pub schema_version: u32,
    chunks: std::collections::HashMap<String, Vec<u8>>, // stored already-zstd
}

impl LoadedSave {
    pub fn chunk_names(&self) -> impl Iterator<Item = &str> {
        self.chunks.keys().map(String::as_str)
    }

    pub fn has_chunk(&self, name: &str) -> bool {
        self.chunks.contains_key(name)
    }

    pub fn read_chunk<T: serde::de::DeserializeOwned>(&self, name: &str) -> Result<T, IoError> {
        let compressed = self.chunks.get(name).ok_or_else(|| IoError::MissingChunk(name.to_string()))?;
        let raw = zstd::decode_all(compressed.as_slice())?;
        bincode::serde::decode_from_slice(&raw, bincode::config::standard())
            .map(|(v, _)| v)
            .map_err(|e| IoError::Serde(e.to_string()))
    }
}

/// Serialize `chunks` to `path`. Atomic: writes `<path>.tmp` then renames.
pub fn write_save(path: &Path, chunks: &[(&str, &dyn ErasedChunk)]) -> Result<(), IoError> {
    let mut buf = Vec::with_capacity(64 * 1024);
    buf.extend_from_slice(MAGIC);
    buf.extend_from_slice(&SCHEMA_VERSION.to_le_bytes());
    buf.extend_from_slice(&(chunks.len() as u32).to_le_bytes());
    for (name, value) in chunks {
        let name_bytes = name.as_bytes();
        if name_bytes.is_empty() || name_bytes.len() > 255 {
            return Err(IoError::Serde(format!("invalid chunk name: {name:?}")));
        }
        let compressed = value.encode_zstd();
        if compressed.len() as u64 > MAX_CHUNK_PAYLOAD {
            return Err(IoError::ChunkTooLarge { name: (*name).to_string(), len: compressed.len() as u64 });
        }
        let crc = crc32fast::hash(&compressed);
        buf.push(name_bytes.len() as u8);
        buf.extend_from_slice(name_bytes);
        buf.extend_from_slice(&crc.to_le_bytes());
        buf.extend_from_slice(&(compressed.len() as u64).to_le_bytes());
        buf.extend_from_slice(&compressed);
    }
    let tmp = path.with_extension("cflsave.tmp");
    std::fs::write(&tmp, &buf)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

pub fn read_save(path: &Path) -> Result<LoadedSave, IoError> {
    let data = std::fs::read(path)?;
    let mut cursor = data.as_slice();
    macro_rules! take {
        ($n:expr) => {{
            let n = $n;
            if cursor.len() < n {
                return Err(IoError::Io(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "truncated save",
                )));
            }
            let (a, b) = cursor.split_at(n);
            cursor = b;
            a
        }};
    }
    if take!(4) != MAGIC {
        return Err(IoError::BadMagic);
    }
    let schema_version = u32::from_le_bytes(take!(4).try_into().unwrap());
    if schema_version > SCHEMA_VERSION {
        return Err(IoError::BadVersion(schema_version));
    }
    let chunk_count = u32::from_le_bytes(take!(4).try_into().unwrap());
    let mut chunks = std::collections::HashMap::with_capacity(chunk_count as usize);
    for _ in 0..chunk_count {
        let name_len = take!(1)[0] as usize;
        let name_bytes = take!(name_len);
        let name = std::str::from_utf8(name_bytes)
            .map_err(|_| IoError::Serde("chunk name is not utf-8".into()))?
            .to_string();
        let crc = u32::from_le_bytes(take!(4).try_into().unwrap());
        let len = u64::from_le_bytes(take!(8).try_into().unwrap());
        if len > MAX_CHUNK_PAYLOAD {
            return Err(IoError::ChunkTooLarge { name, len });
        }
        let payload = take!(len as usize).to_vec();
        if crc32fast::hash(&payload) != crc {
            return Err(IoError::CrcMismatch(name));
        }
        chunks.insert(name, payload);
    }
    Ok(LoadedSave { schema_version, chunks })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("cfl_test_{}_{}", std::process::id(), tag));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("test.cflsave")
    }

    #[test]
    fn roundtrip_chunk_integrity() {
        let path = temp_path("roundtrip");
        let grid: Vec<f32> = (0..10_000).map(|i| i as f32 * 0.5).collect();
        let header = vec![1u8, 2, 3, 4];
        write_save(&path, &[("grids", &grid), ("header", &header)]).unwrap();

        let save = read_save(&path).unwrap();
        assert_eq!(save.schema_version, SCHEMA_VERSION);
        let g: Vec<f32> = save.read_chunk("grids").unwrap();
        assert_eq!(g, grid);
        let h: Vec<u8> = save.read_chunk("header").unwrap();
        assert_eq!(h, header);
        assert!(save.has_chunk("grids"));
        assert!(matches!(save.read_chunk::<Vec<u8>>("nope"), Err(IoError::MissingChunk(_))));

        std::fs::remove_dir_all(path.parent().unwrap()).ok();
    }

    #[test]
    fn corrupt_chunk_fails_crc() {
        let path = temp_path("corrupt");
        std::fs::write(&path, {
            let raw = bincode::serde::encode_to_vec(&vec![42u8; 100], bincode::config::standard()).unwrap();
            let compressed = zstd::encode_all(raw.as_slice(), ZSTD_LEVEL).unwrap();
            let mut buf = Vec::new();
            buf.extend_from_slice(MAGIC);
            buf.extend_from_slice(&SCHEMA_VERSION.to_le_bytes());
            buf.extend_from_slice(&1u32.to_le_bytes());
            buf.push(5);
            buf.extend_from_slice(b"grids");
            buf.extend_from_slice(&0xDEAD_BEEFu32.to_le_bytes()); // wrong crc on purpose
            buf.extend_from_slice(&(compressed.len() as u64).to_le_bytes());
            buf.extend_from_slice(&compressed);
            buf
        })
        .unwrap();
        assert!(matches!(read_save(&path), Err(IoError::CrcMismatch(_))));
        std::fs::remove_dir_all(path.parent().unwrap()).ok();
    }

    #[test]
    fn rejects_bad_magic() {
        let path = temp_path("magic");
        std::fs::write(&path, b"MZ....").unwrap();
        assert!(matches!(read_save(&path), Err(IoError::BadMagic)));
        std::fs::remove_dir_all(path.parent().unwrap()).ok();
    }

    #[test]
    fn rejects_future_schema() {
        let path = temp_path("future");
        let mut buf = Vec::new();
        buf.extend_from_slice(MAGIC);
        buf.extend_from_slice(&(SCHEMA_VERSION + 1).to_le_bytes());
        buf.extend_from_slice(&0u32.to_le_bytes());
        std::fs::write(&path, &buf).unwrap();
        assert!(matches!(read_save(&path), Err(IoError::BadVersion(_))));
        std::fs::remove_dir_all(path.parent().unwrap()).ok();
    }
}
