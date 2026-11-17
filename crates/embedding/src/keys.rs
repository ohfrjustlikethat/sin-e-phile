//! Which title each vector of a published artefact is, by IMDb id (ADR-0051).
//!
//! An artefact is positional: vector *n* is the nth core title of the catalogue it was built
//! from, in that catalogue's id order. Ids are row numbers, assigned as a catalogue is
//! loaded, so **no two installs agree on them**, and an install's own catalogue is never
//! the producer's. The published artefact could therefore only ever be indexed on the
//! machine that made it; everywhere else the prefix check refused it (correctly), and search
//! by meaning never worked (Phase 16, found on a copy of the author's install).
//!
//! A key file names each position by its IMDb id instead, which every install has. It lists
//! only the positions worth indexing (the producer's judgement, made where the synopses are:
//! an install has none, so it cannot make that judgement itself), and it is paired with its
//! artefact by the artefact's own `id_digest` and count, so it can never be applied to a
//! different one.
//!
//! Layout, little-endian: `SINEKEY\0`, version (u32), the artefact's `id_digest` (32 bytes),
//! the artefact's count (u64), the number of pairs (u64), then each pair as position (u32)
//! and IMDb number (u32: `tt0111161` is 111161).

use std::io::Write;

use crate::artefact::Header;

pub const MAGIC: &[u8; 8] = b"SINEKEY\0";
pub const VERSION: u32 = 1;
const FIXED: usize = 8 + 4 + 32 + 8 + 8;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum KeysError {
    #[error("not a key file (bad magic)")]
    NotKeys,
    #[error("key file version {0}, this build understands {VERSION}")]
    Version(u32),
    #[error("key file is truncated")]
    Truncated,
    #[error("key file names position {position}, past the artefact's {count} vectors")]
    PastTheEnd { position: u32, count: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keys {
    /// The `id_digest` of the artefact these keys name.
    pub artefact_digest: [u8; 32],
    /// That artefact's vector count.
    pub count: u64,
    /// (position in the artefact, IMDb number), ascending by position.
    pub pairs: Vec<(u32, u32)>,
}

impl Keys {
    /// Whether these keys name this artefact's vectors.
    pub fn pairs_with(&self, header: &Header) -> bool {
        self.artefact_digest == header.id_digest && self.count == header.count
    }

    pub fn write<W: Write>(&self, out: &mut W) -> std::io::Result<()> {
        out.write_all(MAGIC)?;
        out.write_all(&VERSION.to_le_bytes())?;
        out.write_all(&self.artefact_digest)?;
        out.write_all(&self.count.to_le_bytes())?;
        out.write_all(&(self.pairs.len() as u64).to_le_bytes())?;
        for (position, imdb) in &self.pairs {
            out.write_all(&position.to_le_bytes())?;
            out.write_all(&imdb.to_le_bytes())?;
        }
        Ok(())
    }

    pub fn read(bytes: &[u8]) -> Result<Self, KeysError> {
        if bytes.len() < FIXED {
            return Err(if bytes.starts_with(MAGIC) || bytes.len() < 8 {
                KeysError::Truncated
            } else {
                KeysError::NotKeys
            });
        }
        if &bytes[..8] != MAGIC {
            return Err(KeysError::NotKeys);
        }
        let u32_at = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().expect("4"));
        let u64_at = |at: usize| u64::from_le_bytes(bytes[at..at + 8].try_into().expect("8"));
        let version = u32_at(8);
        if version != VERSION {
            return Err(KeysError::Version(version));
        }
        let mut artefact_digest = [0u8; 32];
        artefact_digest.copy_from_slice(&bytes[12..44]);
        let count = u64_at(44);
        let n = u64_at(52) as usize;
        let body = &bytes[FIXED..];
        if body.len() != n * 8 {
            return Err(KeysError::Truncated);
        }
        let mut pairs = Vec::with_capacity(n);
        for chunk in body.as_chunks::<8>().0 {
            let position = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
            let imdb = u32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]);
            if u64::from(position) >= count {
                return Err(KeysError::PastTheEnd { position, count });
            }
            pairs.push((position, imdb));
        }
        Ok(Self {
            artefact_digest,
            count,
            pairs,
        })
    }
}

/// `tt0111161` → 111161; `None` for anything else.
pub fn imdb_number(tconst: &str) -> Option<u32> {
    let digits = tconst.strip_prefix("tt")?;
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

/// 111161 → `tt0111161`: at least seven digits, as IMDb writes them.
pub fn tconst(number: u32) -> String {
    format!("tt{number:07}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_round_trip_and_refuse_what_is_not_theirs() {
        let keys = Keys {
            artefact_digest: [7; 32],
            count: 10,
            pairs: vec![(0, 111_161), (4, 10_001_008), (9, 1)],
        };
        let mut bytes = Vec::new();
        keys.write(&mut bytes).unwrap();
        assert_eq!(Keys::read(&bytes).unwrap(), keys);

        assert_eq!(
            Keys::read(
                b"NOTKEYS\0and more bytes than the fixed part needs to be here...................."
            )
            .unwrap_err(),
            KeysError::NotKeys
        );
        assert_eq!(
            Keys::read(&bytes[..bytes.len() - 3]).unwrap_err(),
            KeysError::Truncated
        );
        let mut past = keys.clone();
        past.pairs.push((10, 5));
        let mut bytes = Vec::new();
        past.write(&mut bytes).unwrap();
        assert_eq!(
            Keys::read(&bytes).unwrap_err(),
            KeysError::PastTheEnd {
                position: 10,
                count: 10
            }
        );
    }

    #[test]
    fn imdb_ids_read_and_write_as_imdb_writes_them() {
        assert_eq!(imdb_number("tt0111161"), Some(111_161));
        assert_eq!(imdb_number("tt10001008"), Some(10_001_008));
        assert_eq!(imdb_number("nm0000001"), None);
        assert_eq!(imdb_number("tt"), None);
        assert_eq!(tconst(111_161), "tt0111161");
        assert_eq!(tconst(10_001_008), "tt10001008");
    }
}
