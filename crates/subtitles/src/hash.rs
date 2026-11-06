//! The OpenSubtitles movie hash: what identifies a file by its content, so a subtitle
//! timed for these exact bytes can be found, and a nudge follows the file when it is
//! renamed or moved.
//!
//! The file's size plus the sum of its first and last 64 KiB read as little-endian
//! 64-bit words, wrapping, as 16 hex digits. Cheap: 128 KiB read whatever the size.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

pub const CHUNK: u64 = 64 * 1024;

/// The hash from the size and the two ends. Either end may be short for a small file.
pub fn movie_hash(size: u64, head: &[u8], tail: &[u8]) -> String {
    let words = |b: &[u8]| -> u64 {
        b.chunks(8).fold(0u64, |acc, c| {
            let mut w = [0u8; 8];
            w[..c.len()].copy_from_slice(c);
            acc.wrapping_add(u64::from_le_bytes(w))
        })
    };
    format!(
        "{:016x}",
        size.wrapping_add(words(head)).wrapping_add(words(tail))
    )
}

/// The hash of a file on disk; `None` for one smaller than 128 KiB, which no video is.
pub fn of_file(path: &Path) -> std::io::Result<Option<String>> {
    let mut f = File::open(path)?;
    let size = f.metadata()?.len();
    if size < 2 * CHUNK {
        return Ok(None);
    }
    let mut head = vec![0u8; CHUNK as usize];
    f.read_exact(&mut head)?;
    let mut tail = vec![0u8; CHUNK as usize];
    f.seek(SeekFrom::Start(size - CHUNK))?;
    f.read_exact(&mut tail)?;
    Ok(Some(movie_hash(size, &head, &tail)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hash_is_size_plus_both_ends() {
        // All zeros: the hash is the size alone.
        let zeros = vec![0u8; CHUNK as usize];
        assert_eq!(movie_hash(131_072, &zeros, &zeros), "0000000000020000");
        // One word of 1 at each end adds 2.
        let mut one = zeros.clone();
        one[0] = 1;
        assert_eq!(movie_hash(131_072, &one, &one), "0000000000020002");
        // Wrapping, not overflow.
        let max = vec![0xFFu8; 8];
        assert_eq!(movie_hash(1, &max, &[]), "0000000000000000");
    }

    #[test]
    fn a_file_on_disk_hashes_like_its_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("film.mkv");
        let bytes: Vec<u8> = (0..300_000u32).map(|i| (i % 251) as u8).collect();
        std::fs::write(&path, &bytes).unwrap();
        let n = bytes.len();
        assert_eq!(
            of_file(&path).unwrap(),
            Some(movie_hash(n as u64, &bytes[..65_536], &bytes[n - 65_536..]))
        );
        std::fs::write(&path, [0u8; 10]).unwrap();
        assert_eq!(of_file(&path).unwrap(), None, "too small to be a video");
    }
}
