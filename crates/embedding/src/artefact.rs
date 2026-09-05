//! The embedding artefact: what gets published, and what refuses to load.
//!
//! ADR-0014 requires an artefact that is **deterministic, versioned, pinned and
//! checksummed**, and that the application *refuses to load* when its model identity
//! does not match the model actually present.
//!
//! # Why the refusal matters more than the checksum
//!
//! A corrupt download fails loudly — a truncated file, a bad hash, an obvious error. A
//! *mismatched* artefact fails silently: vectors produced by one model compared against
//! queries produced by another land in different regions of a space that has no idea
//! anything is wrong. Search simply gets worse, gradually, with nothing to point at.
//! [`Header::compatible_with`] is what makes that a startup error instead.
//!
//! # Determinism
//!
//! Same catalogue snapshot + same model + same document-builder version = byte-identical
//! file. So: no write timestamp, no map iteration, fixed-width little-endian
//! throughout, and the snapshot date is an **input** rather than `now()`. A format that
//! records when it was built cannot be diffed against a rebuild, and "deterministic"
//! becomes a claim nobody can check.

use std::io::{Read, Write};

use sha2::{Digest, Sha256};

/// `SINEEMB\0`. Identifies the file before anything is trusted about its contents.
pub const MAGIC: [u8; 8] = *b"SINEEMB\0";

/// The container format's own version, distinct from the model's and the document
/// builder's. Bumped only when the layout below changes.
pub const FORMAT_VERSION: u16 = 1;

/// Bytes of header before the vectors begin.
const HEADER_BYTES: usize = 256;

#[derive(Debug, thiserror::Error)]
pub enum ArtefactError {
    #[error("not an embedding artefact (bad magic)")]
    NotAnArtefact,
    #[error("artefact format version {found}, this build understands {expected}")]
    FormatVersion { found: u16, expected: u16 },
    #[error("artefact is truncated: expected {expected} bytes of vectors, found {found}")]
    Truncated { expected: usize, found: usize },
    #[error("checksum mismatch: the artefact is corrupt or was modified")]
    Checksum,
    #[error("artefact was built for model {artefact:?}, this build has {present:?}")]
    ModelMismatch { artefact: String, present: String },
    #[error("artefact was built by document builder v{artefact}, this build is v{present}")]
    DocumentBuilderMismatch { artefact: u32, present: u32 },
    #[error("a field is too long for the header: {0}")]
    FieldTooLong(&'static str),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

/// What the artefact says about itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    /// The exact model, including its quantisation — e.g.
    /// `all-MiniLM-L6-v2-int8`. Compared verbatim, because "close enough" is exactly
    /// the judgement that produces a silently degraded search.
    pub model: String,
    pub dimension: u16,
    /// How the vectors are stored. Only `Int8` today; the field exists so a future
    /// float artefact is a new value rather than a new format.
    pub quantisation: Quantisation,
    pub document_builder_version: u32,
    /// The catalogue this was built from, as `YYYY-MM-DD`. An input, never `now()`.
    pub snapshot_date: String,
    pub count: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quantisation {
    Int8,
}

impl Quantisation {
    fn code(self) -> u8 {
        match self {
            Quantisation::Int8 => 1,
        }
    }

    fn from_code(code: u8) -> Option<Self> {
        match code {
            1 => Some(Quantisation::Int8),
            _ => None,
        }
    }

    pub fn bytes_per_dimension(self) -> usize {
        match self {
            Quantisation::Int8 => 1,
        }
    }
}

impl Header {
    /// Would loading this artefact produce meaningful results on this build?
    ///
    /// Both halves matter and they fail differently. A model mismatch means the vectors
    /// are in a different space entirely. A document-builder mismatch means they are in
    /// the same space but describe different sentences — subtler, and no less wrong.
    pub fn compatible_with(
        &self,
        model: &str,
        document_builder_version: u32,
    ) -> Result<(), ArtefactError> {
        if self.model != model {
            return Err(ArtefactError::ModelMismatch {
                artefact: self.model.clone(),
                present: model.to_string(),
            });
        }
        if self.document_builder_version != document_builder_version {
            return Err(ArtefactError::DocumentBuilderMismatch {
                artefact: self.document_builder_version,
                present: document_builder_version,
            });
        }
        Ok(())
    }

    /// Bytes one vector occupies.
    pub fn vector_bytes(&self) -> usize {
        self.dimension as usize * self.quantisation.bytes_per_dimension()
    }

    /// What the whole file should weigh — for showing a size before a download, which
    /// ADR-0014 requires to be consented to.
    pub fn expected_bytes(&self) -> u64 {
        HEADER_BYTES as u64 + self.count * self.vector_bytes() as u64 + 32
    }

    fn encode(&self) -> Result<[u8; HEADER_BYTES], ArtefactError> {
        let mut out = [0u8; HEADER_BYTES];
        out[0..8].copy_from_slice(&MAGIC);
        out[8..10].copy_from_slice(&FORMAT_VERSION.to_le_bytes());
        out[10..12].copy_from_slice(&self.dimension.to_le_bytes());
        out[12] = self.quantisation.code();
        out[13..17].copy_from_slice(&self.document_builder_version.to_le_bytes());
        out[17..25].copy_from_slice(&self.count.to_le_bytes());

        // Length-prefixed strings in fixed slots. Fixed width keeps the vectors at a
        // constant offset, which is what lets a reader memory-map the file later
        // without parsing anything first.
        write_string(&mut out[25..89], &self.model, "model")?;
        write_string(&mut out[89..105], &self.snapshot_date, "snapshot_date")?;
        // 105..256 is reserved and stays zero, so an older reader that ignores it and
        // a newer one that uses it produce the same bytes for the same content today.
        Ok(out)
    }

    fn decode(bytes: &[u8]) -> Result<Self, ArtefactError> {
        if bytes.len() < HEADER_BYTES || bytes[0..8] != MAGIC {
            return Err(ArtefactError::NotAnArtefact);
        }
        let format = u16::from_le_bytes([bytes[8], bytes[9]]);
        if format != FORMAT_VERSION {
            return Err(ArtefactError::FormatVersion {
                found: format,
                expected: FORMAT_VERSION,
            });
        }
        let dimension = u16::from_le_bytes([bytes[10], bytes[11]]);
        let quantisation =
            Quantisation::from_code(bytes[12]).ok_or(ArtefactError::NotAnArtefact)?;
        let document_builder_version =
            u32::from_le_bytes(bytes[13..17].try_into().expect("4 bytes"));
        let count = u64::from_le_bytes(bytes[17..25].try_into().expect("8 bytes"));

        Ok(Header {
            model: read_string(&bytes[25..89])?,
            dimension,
            quantisation,
            document_builder_version,
            snapshot_date: read_string(&bytes[89..105])?,
            count,
        })
    }
}

fn write_string(slot: &mut [u8], value: &str, field: &'static str) -> Result<(), ArtefactError> {
    let bytes = value.as_bytes();
    if bytes.len() + 1 > slot.len() {
        return Err(ArtefactError::FieldTooLong(field));
    }
    slot[0] = bytes.len() as u8;
    slot[1..1 + bytes.len()].copy_from_slice(bytes);
    Ok(())
}

fn read_string(slot: &[u8]) -> Result<String, ArtefactError> {
    let length = slot[0] as usize;
    if length + 1 > slot.len() {
        return Err(ArtefactError::NotAnArtefact);
    }
    String::from_utf8(slot[1..1 + length].to_vec()).map_err(|_| ArtefactError::NotAnArtefact)
}

/// Write an artefact: header, then vectors in order, then a SHA-256 of everything
/// before it.
///
/// Vectors are written in the caller's order and the caller must supply them sorted by
/// catalogue id — the order *is* the index, since a lookup is `offset = id_position *
/// vector_bytes`. An unsorted write produces a valid file that returns the wrong
/// vector for every item.
pub fn write<W: Write>(
    writer: &mut W,
    header: &Header,
    vectors: impl IntoIterator<Item = Vec<i8>>,
) -> Result<[u8; 32], ArtefactError> {
    let encoded = header.encode()?;
    let mut hasher = Sha256::new();
    hasher.update(encoded);
    writer.write_all(&encoded)?;

    let mut written = 0u64;
    for vector in vectors {
        // A short vector would silently shift every subsequent one, so this is checked
        // rather than trusted.
        if vector.len() != header.dimension as usize {
            return Err(ArtefactError::Truncated {
                expected: header.dimension as usize,
                found: vector.len(),
            });
        }
        let bytes: Vec<u8> = vector.iter().map(|v| *v as u8).collect();
        hasher.update(&bytes);
        writer.write_all(&bytes)?;
        written += 1;
    }

    if written != header.count {
        return Err(ArtefactError::Truncated {
            expected: header.count as usize,
            found: written as usize,
        });
    }

    let checksum: [u8; 32] = hasher.finalize().into();
    writer.write_all(&checksum)?;
    Ok(checksum)
}

/// A loaded artefact.
pub struct Artefact {
    pub header: Header,
    pub checksum: [u8; 32],
    vectors: Vec<u8>,
}

impl Artefact {
    /// Read and verify an artefact.
    ///
    /// The checksum is verified here rather than on demand: a corrupt file that is
    /// discovered on the four-hundred-thousandth lookup has already been trusted for a
    /// long time.
    pub fn read<R: Read>(reader: &mut R) -> Result<Self, ArtefactError> {
        let mut all = Vec::new();
        reader.read_to_end(&mut all)?;

        if all.len() < HEADER_BYTES + 32 {
            return Err(ArtefactError::NotAnArtefact);
        }
        let header = Header::decode(&all[..HEADER_BYTES])?;

        let body_end = all.len() - 32;
        let expected_vector_bytes = header.count as usize * header.vector_bytes();
        let actual_vector_bytes = body_end - HEADER_BYTES;
        if actual_vector_bytes != expected_vector_bytes {
            return Err(ArtefactError::Truncated {
                expected: expected_vector_bytes,
                found: actual_vector_bytes,
            });
        }

        let mut hasher = Sha256::new();
        hasher.update(&all[..body_end]);
        let computed: [u8; 32] = hasher.finalize().into();
        let stored: [u8; 32] = all[body_end..].try_into().expect("32 bytes");
        if computed != stored {
            return Err(ArtefactError::Checksum);
        }

        Ok(Artefact {
            header,
            checksum: stored,
            vectors: all[HEADER_BYTES..body_end].to_vec(),
        })
    }

    /// The nth vector, in the order it was written.
    pub fn vector(&self, index: u64) -> Option<&[i8]> {
        if index >= self.header.count {
            return None;
        }
        let width = self.header.vector_bytes();
        let start = index as usize * width;
        let bytes = self.vectors.get(start..start + width)?;
        // SAFETY: i8 and u8 have identical layout; this is a reinterpretation of the
        // same bytes, not a conversion.
        Some(unsafe { std::slice::from_raw_parts(bytes.as_ptr() as *const i8, bytes.len()) })
    }

    pub fn checksum_hex(&self) -> String {
        self.checksum.iter().map(|b| format!("{b:02x}")).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(count: u64) -> Header {
        Header {
            model: "all-MiniLM-L6-v2-int8".into(),
            dimension: 4,
            quantisation: Quantisation::Int8,
            document_builder_version: 1,
            snapshot_date: "2026-09-05".into(),
            count,
        }
    }

    fn vectors(count: u64) -> Vec<Vec<i8>> {
        (0..count)
            .map(|i| vec![i as i8, (i + 1) as i8, -(i as i8), 0])
            .collect()
    }

    fn built(count: u64) -> Vec<u8> {
        let mut out = Vec::new();
        write(&mut out, &header(count), vectors(count)).expect("write");
        out
    }

    #[test]
    fn an_artefact_round_trips() {
        let bytes = built(3);
        let artefact = Artefact::read(&mut bytes.as_slice()).expect("read");

        assert_eq!(artefact.header, header(3));
        assert_eq!(artefact.vector(0), Some([0i8, 1, 0, 0].as_slice()));
        assert_eq!(artefact.vector(2), Some([2i8, 3, -2, 0].as_slice()));
        assert_eq!(
            artefact.vector(3),
            None,
            "past the end is None, not a panic"
        );
    }

    #[test]
    fn the_same_input_produces_byte_identical_output() {
        // ADR-0014's determinism requirement, which is what makes a rebuild checkable
        // at all. A write timestamp anywhere in the header would break this.
        assert_eq!(built(10), built(10));
    }

    #[test]
    fn a_mismatched_model_is_refused() {
        // The failure this whole header exists to prevent: vectors from one model
        // compared against queries from another, landing in different regions of a
        // space that has no idea anything is wrong.
        let artefact = Artefact::read(&mut built(1).as_slice()).expect("read");
        assert!(artefact
            .header
            .compatible_with("all-MiniLM-L6-v2-int8", 1)
            .is_ok());

        let err = artefact
            .header
            .compatible_with("all-MiniLM-L12-v2-int8", 1)
            .expect_err("must refuse");
        assert!(matches!(err, ArtefactError::ModelMismatch { .. }));

        // Same model, different document builder: same space, different sentences.
        let err = artefact
            .header
            .compatible_with("all-MiniLM-L6-v2-int8", 2)
            .expect_err("must refuse");
        assert!(matches!(err, ArtefactError::DocumentBuilderMismatch { .. }));
    }

    #[test]
    fn a_corrupt_artefact_is_caught_at_load_rather_than_on_use() {
        let mut bytes = built(4);
        let middle = bytes.len() / 2;
        bytes[middle] ^= 0xff;
        assert!(matches!(
            Artefact::read(&mut bytes.as_slice()),
            Err(ArtefactError::Checksum)
        ));
    }

    #[test]
    fn a_truncated_artefact_is_caught() {
        let bytes = built(8);
        // Lose the last vector and the checksum stays where it was — the length check
        // catches this before the hash is even computed.
        let short = &bytes[..bytes.len() - 4];
        assert!(matches!(
            Artefact::read(&mut &short[..]),
            Err(ArtefactError::Truncated { .. }) | Err(ArtefactError::Checksum)
        ));

        assert!(matches!(
            Artefact::read(&mut &bytes[..10]),
            Err(ArtefactError::NotAnArtefact)
        ));
    }

    #[test]
    fn something_that_is_not_an_artefact_is_rejected_immediately() {
        let junk = vec![0u8; 512];
        assert!(matches!(
            Artefact::read(&mut junk.as_slice()),
            Err(ArtefactError::NotAnArtefact)
        ));
    }

    #[test]
    fn a_short_vector_is_refused_rather_than_shifting_every_later_one() {
        // A vector one element short would slide every subsequent lookup by a byte,
        // and the file would still be a perfectly valid artefact.
        let mut out = Vec::new();
        let err = write(&mut out, &header(2), vec![vec![1i8, 2, 3, 4], vec![1i8, 2]])
            .expect_err("must refuse");
        assert!(matches!(err, ArtefactError::Truncated { .. }));
    }

    #[test]
    fn a_count_that_disagrees_with_the_vectors_is_refused() {
        let mut out = Vec::new();
        let err = write(&mut out, &header(5), vectors(2)).expect_err("must refuse");
        assert!(matches!(
            err,
            ArtefactError::Truncated {
                expected: 5,
                found: 2
            }
        ));
    }

    #[test]
    fn an_over_long_field_is_an_error_not_a_silent_truncation() {
        // A truncated model identity would compare unequal to itself and refuse every
        // artefact, which is a baffling failure a long way from its cause.
        let long = Header {
            model: "m".repeat(200),
            ..header(0)
        };
        assert!(matches!(
            write(&mut Vec::new(), &long, vec![]),
            Err(ArtefactError::FieldTooLong("model"))
        ));
    }

    #[test]
    fn the_expected_size_can_be_shown_before_a_download() {
        // ADR-0014: the download is consented to WITH ITS SIZE SHOWN.
        let h = Header {
            dimension: 384,
            count: 855_703,
            ..header(0)
        };
        // 384 int8 dimensions per title, plus a 256-byte header and a 32-byte digest.
        assert_eq!(h.expected_bytes(), 256 + 855_703 * 384 + 32);
        assert_eq!(h.expected_bytes() / 1_048_576, 313, "313 MB");
    }
}
