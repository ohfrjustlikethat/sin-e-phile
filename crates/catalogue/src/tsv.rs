//! Streaming reader for IMDb's gzipped TSV datasets.
//!
//! # Never in memory
//!
//! `title.principals.tsv.gz` is several hundred megabytes compressed and several
//! gigabytes raw. Nothing here holds more than one row at a time, and the row is a
//! borrowed slice of a reused line buffer rather than a fresh allocation per field.
//!
//! # How resumption works, and what it costs
//!
//! The cursor is the **last id processed**, not a byte offset. Gzip cannot be seeked
//! — decoding position N requires decoding everything before it — so a byte offset
//! into the decompressed stream would have to be reached by decompressing anyway.
//!
//! IMDb's files are sorted by their id column, so resuming means decompressing from
//! the start and skipping rows until the id is greater than the cursor. That is a
//! real cost: a resume near the end re-reads the whole file. It is paid in CPU only,
//! never in re-inserted rows, and decompression is roughly two orders of magnitude
//! cheaper per row than the insert it replaces.
//!
//! The alternative — decompressing once to a plain file and seeking within it —
//! would trade that for several gigabytes of disk in a directory the user is
//! expected to be able to copy around (§2.4). Not worth it.
//!
//! # `\N`
//!
//! IMDb writes a literal backslash-N for null. Treating it as a string is the
//! classic mistake with these files: it produces titles with a runtime of `\N` and
//! people born in year `\N`, and nothing complains.

use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::Path;

use crate::job::JobError;

const NULL: &str = r"\N";

/// One row, as borrowed slices into the reader's line buffer.
pub struct Row<'a> {
    fields: Vec<&'a str>,
    columns: &'a HashMap<String, usize>,
}

impl<'a> Row<'a> {
    /// A field by column name, with IMDb's `\N` mapped to `None`.
    pub fn get(&self, column: &str) -> Option<&'a str> {
        let index = *self.columns.get(column)?;
        match self.fields.get(index) {
            Some(&value) if value != NULL && !value.is_empty() => Some(value),
            _ => None,
        }
    }

    pub fn parse<T: std::str::FromStr>(&self, column: &str) -> Option<T> {
        self.get(column)?.parse().ok()
    }

    /// A comma-separated field, which IMDb uses for genres and professions.
    pub fn list(&self, column: &str) -> Vec<&'a str> {
        self.get(column)
            .map(|v| v.split(',').filter(|s| !s.is_empty()).collect())
            .unwrap_or_default()
    }

    pub fn field_count(&self) -> usize {
        self.fields.len()
    }
}

/// A gzipped TSV with a header row.
pub struct TsvReader {
    reader: BufReader<flate2::read::MultiGzDecoder<BufReader<std::fs::File>>>,
    columns: HashMap<String, usize>,
    header_len: usize,
    line: String,
    /// Rows read past the header, including any skipped while seeking a cursor.
    pub rows_scanned: u64,
}

impl TsvReader {
    pub fn open(path: &Path) -> Result<Self, JobError> {
        let file = std::fs::File::open(path)?;
        // Two buffers on purpose: one under the decoder so it reads the file in
        // large blocks, one over it so line splitting is not a syscall per line.
        let decoder = flate2::read::MultiGzDecoder::new(BufReader::with_capacity(1 << 20, file));
        let mut reader = BufReader::with_capacity(1 << 20, decoder);

        let mut header = String::new();
        reader.read_line(&mut header).map_err(|e| {
            JobError::step(
                "tsv",
                format!("{}: cannot read the header: {e}", path.display()),
            )
        })?;
        if header.is_empty() {
            return Err(JobError::step(
                "tsv",
                format!("{} is empty", path.display()),
            ));
        }

        let names: Vec<&str> = header.trim_end_matches(['\r', '\n']).split('\t').collect();
        let columns = names
            .iter()
            .enumerate()
            .map(|(i, name)| ((*name).to_string(), i))
            .collect::<HashMap<_, _>>();

        Ok(Self {
            reader,
            header_len: names.len(),
            columns,
            line: String::new(),
            rows_scanned: 0,
        })
    }

    /// Fail early if the file does not have the columns the caller will ask for.
    ///
    /// IMDb has changed these before. Discovering it here names the missing column;
    /// discovering it in `Row::get` produces a silent `None` and a catalogue full of
    /// empty fields.
    pub fn require_columns(&self, required: &[&str]) -> Result<(), JobError> {
        let missing: Vec<&str> = required
            .iter()
            .copied()
            .filter(|c| !self.columns.contains_key(*c))
            .collect();
        if missing.is_empty() {
            return Ok(());
        }
        let mut present: Vec<&str> = self.columns.keys().map(String::as_str).collect();
        present.sort_unstable();
        Err(JobError::step(
            "tsv",
            format!(
                "the dataset is missing column(s) {missing:?}; it has {present:?}. \
                 IMDb has changed these before — the loader needs updating, not the data."
            ),
        ))
    }

    /// Read forward to the next usable row. `false` at end of file.
    ///
    /// SPLIT FROM PARSING ON PURPOSE, and not for style. A single `next_row` that
    /// looped and returned `Row<'_>` cannot compile: the returned row borrows the
    /// line buffer for as long as the caller holds it, and the next turn of the loop
    /// needs to clear that same buffer. The borrow checker cannot know the loop will
    /// not go round again while the row is alive — because in general it might.
    ///
    /// Splitting it means the mutable borrow ends when `advance` returns, and the
    /// immutable borrow starts when `current_row` is called. Same work, and the
    /// lifetimes no longer overlap.
    pub fn advance(&mut self) -> Result<bool, JobError> {
        loop {
            self.line.clear();
            let read = self.reader.read_line(&mut self.line).map_err(|e| {
                // A UTF-8 error here is a corrupt download far more often than a
                // genuinely non-UTF-8 dataset, so say so.
                JobError::step(
                    "tsv",
                    format!("read failed at row {}: {e}", self.rows_scanned),
                )
            })?;
            if read == 0 {
                self.line.clear();
                return Ok(false);
            }
            self.rows_scanned += 1;

            let trimmed = self.line.trim_end_matches(['\r', '\n']);
            if trimmed.is_empty() {
                continue;
            }

            // A short row is truncation or an embedded newline. Skipping it loudly
            // beats letting `get` return None for every column after the break.
            let count = trimmed.split('\t').count();
            if count != self.header_len {
                tracing::warn!(
                    "row {} has {count} fields, expected {} — skipped",
                    self.rows_scanned,
                    self.header_len
                );
                continue;
            }

            return Ok(true);
        }
    }

    /// The next row, or `None` at end of file.
    pub fn next_row(&mut self) -> Result<Option<Row<'_>>, JobError> {
        if self.advance()? {
            Ok(self.current_row())
        } else {
            Ok(None)
        }
    }

    /// Skip forward until the value in `id_column` is greater than `cursor`.
    ///
    /// Returns how many rows were skipped, so a resumed run can report honestly what
    /// it re-read rather than implying it started where it left off.
    pub fn seek_past(&mut self, id_column: &str, cursor: &str) -> Result<u64, JobError> {
        let index = *self
            .columns
            .get(id_column)
            .ok_or_else(|| JobError::step("tsv", format!("no column {id_column:?} to seek on")))?;

        let mut skipped = 0u64;
        loop {
            self.line.clear();
            let read = self.reader.read_line(&mut self.line)?;
            if read == 0 {
                return Ok(skipped);
            }
            self.rows_scanned += 1;

            let trimmed = self.line.trim_end_matches(['\r', '\n']);
            let Some(id) = trimmed.split('\t').nth(index) else {
                continue;
            };
            // IMDb ids are fixed-width and zero-padded (`tt0000001`), so byte order
            // is numeric order and a string comparison is correct. This would be a
            // bug on any id that is not.
            if id > cursor {
                // This row is the first unprocessed one. Rewinding is impossible on
                // a gzip stream, so the caller must handle it before reading on.
                return Ok(skipped);
            }
            skipped += 1;
        }
    }

    /// The row currently in the buffer, after `seek_past` stopped on it.
    pub fn current_row(&self) -> Option<Row<'_>> {
        let trimmed = self.line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            return None;
        }
        let fields: Vec<&str> = trimmed.split('\t').collect();
        if fields.len() != self.header_len {
            return None;
        }
        Some(Row {
            fields,
            columns: &self.columns,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn gz(contents: &str) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("data.tsv.gz");
        let mut encoder = flate2::write::GzEncoder::new(
            std::fs::File::create(&path).expect("create"),
            flate2::Compression::fast(),
        );
        encoder.write_all(contents.as_bytes()).expect("write");
        encoder.finish().expect("finish");
        (dir, path)
    }

    const SAMPLE: &str = "tconst\ttitleType\tprimaryTitle\tstartYear\tgenres\n\
        tt0000001\tshort\tCarmencita\t1894\tDocumentary,Short\n\
        tt0000002\tshort\tLe clown\t\\N\tAnimation\n\
        tt0000003\tmovie\tA Film\t1900\t\\N\n";

    #[test]
    fn null_is_none_not_the_string_backslash_n() {
        // The classic mistake with these files.
        let (_dir, path) = gz(SAMPLE);
        let mut reader = TsvReader::open(&path).expect("open");

        reader.next_row().expect("row").expect("first");
        let row = reader.next_row().expect("row").expect("second");
        assert_eq!(row.get("primaryTitle"), Some("Le clown"));
        assert_eq!(row.get("startYear"), None, "\\N must not become a year");
        assert_eq!(row.parse::<i64>("startYear"), None);
    }

    #[test]
    fn comma_lists_split_and_empty_lists_are_empty() {
        let (_dir, path) = gz(SAMPLE);
        let mut reader = TsvReader::open(&path).expect("open");
        let row = reader.next_row().expect("row").expect("first");
        assert_eq!(row.list("genres"), vec!["Documentary", "Short"]);

        reader.next_row().expect("row");
        let row = reader.next_row().expect("row").expect("third");
        assert!(
            row.list("genres").is_empty(),
            "\\N is not a genre named backslash-N"
        );
    }

    #[test]
    fn a_missing_column_is_named_rather_than_silently_absent() {
        let (_dir, path) = gz(SAMPLE);
        let reader = TsvReader::open(&path).expect("open");

        assert!(reader.require_columns(&["tconst", "primaryTitle"]).is_ok());

        let error = reader
            .require_columns(&["tconst", "runtimeMinutes"])
            .expect_err("should fail")
            .to_string();
        assert!(
            error.contains("runtimeMinutes"),
            "the error names it: {error}"
        );
        assert!(error.contains("tconst"), "and lists what IS there: {error}");
    }

    #[test]
    fn seek_past_stops_on_the_first_unprocessed_row() {
        let (_dir, path) = gz(SAMPLE);
        let mut reader = TsvReader::open(&path).expect("open");

        let skipped = reader.seek_past("tconst", "tt0000001").expect("seek");
        assert_eq!(skipped, 1, "only the first row was already done");

        // The row the seek stopped on must not be lost — it is the first one to do.
        let row = reader
            .current_row()
            .expect("the stopped-on row is available");
        assert_eq!(row.get("tconst"), Some("tt0000002"));
    }

    #[test]
    fn seeking_past_everything_reaches_the_end() {
        let (_dir, path) = gz(SAMPLE);
        let mut reader = TsvReader::open(&path).expect("open");
        reader.seek_past("tconst", "tt9999999").expect("seek");
        assert!(reader.next_row().expect("row").is_none());
    }

    #[test]
    fn a_short_row_is_skipped_not_silently_misread() {
        let (_dir, path) = gz("tconst\ttitleType\tprimaryTitle\n\
             tt0000001\tshort\tCarmencita\n\
             tt0000002\tbroken\n\
             tt0000003\tmovie\tA Film\n");
        let mut reader = TsvReader::open(&path).expect("open");

        let mut ids = Vec::new();
        while let Some(row) = reader.next_row().expect("row") {
            ids.push(row.get("tconst").unwrap_or("?").to_string());
        }
        assert_eq!(
            ids,
            vec!["tt0000001", "tt0000003"],
            "the broken row was dropped"
        );
    }

    #[test]
    fn blank_lines_are_not_rows() {
        let (_dir, path) = gz("tconst\ttitleType\n\ntt0000001\tshort\n\n");
        let mut reader = TsvReader::open(&path).expect("open");
        assert!(reader.next_row().expect("row").is_some());
        assert!(reader.next_row().expect("row").is_none());
    }
}
