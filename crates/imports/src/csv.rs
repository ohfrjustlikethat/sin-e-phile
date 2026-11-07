//! Just enough CSV (RFC 4180) for the exports: comma-separated, fields optionally in
//! double quotes, a doubled quote inside one, and line breaks inside a quoted field (a
//! Letterboxd review can hold them). A header row names the columns.

/// Every record, as fields. A byte-order mark is dropped.
pub fn records(text: &str) -> Vec<Vec<String>> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut out = Vec::new();
    let mut record = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match (quoted, c) {
            (true, '"') if chars.peek() == Some(&'"') => {
                chars.next();
                field.push('"');
            }
            (true, '"') => quoted = false,
            (true, c) => field.push(c),
            (false, '"') if field.is_empty() => quoted = true,
            (false, ',') => record.push(std::mem::take(&mut field)),
            (false, '\r') => {}
            (false, '\n') => {
                record.push(std::mem::take(&mut field));
                if !(record.len() == 1 && record[0].is_empty()) {
                    out.push(std::mem::take(&mut record));
                }
                record.clear();
            }
            (false, c) => field.push(c),
        }
    }
    if !field.is_empty() || !record.is_empty() {
        record.push(field);
        out.push(record);
    }
    out
}

/// Rows as (column name -> value) lookups, by the header row. Columns are matched without
/// regard to case or surrounding space.
pub struct Table {
    header: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

impl Table {
    pub fn parse(text: &str) -> Option<Self> {
        let mut all = records(text).into_iter();
        let header = all
            .next()?
            .into_iter()
            .map(|h| h.trim().to_ascii_lowercase())
            .collect();
        Some(Self {
            header,
            rows: all.collect(),
        })
    }

    pub fn has(&self, column: &str) -> bool {
        self.header
            .iter()
            .any(|h| h == &column.to_ascii_lowercase())
    }

    /// A row's value in `column`, trimmed; `None` if absent or empty.
    pub fn get<'a>(&self, row: &'a [String], column: &str) -> Option<&'a str> {
        let i = self
            .header
            .iter()
            .position(|h| h == &column.to_ascii_lowercase())?;
        row.get(i).map(|v| v.trim()).filter(|v| !v.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_commas_doubled_quotes_and_line_breaks() {
        let text = "\u{feff}Name,Year,Review\r\n\
                    \"Crouching Tiger, Hidden Dragon\",2000,\"He said \"\"no\"\".\nThen left.\"\r\n\
                    Heat,1995,\r\n\r\n";
        let r = records(text);
        assert_eq!(r.len(), 3);
        assert_eq!(
            r[1],
            [
                "Crouching Tiger, Hidden Dragon",
                "2000",
                "He said \"no\".\nThen left."
            ]
        );
        assert_eq!(r[2], ["Heat", "1995", ""]);
    }

    #[test]
    fn columns_by_name_whatever_their_case() {
        let t = Table::parse("Const,Your Rating\ntt0113277,9\n").unwrap();
        assert!(t.has("your rating"));
        assert_eq!(t.get(&t.rows[0], "YOUR RATING"), Some("9"));
        assert_eq!(t.get(&t.rows[0], "Title"), None);
    }
}
