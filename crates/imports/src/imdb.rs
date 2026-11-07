//! IMDb's ratings export: `Const` (tt…), `Your Rating` (1–10), `Date Rated`, `Title`,
//! `Year`, `Title Type`. A rating means it was watched, on the day it was rated. The id is
//! IMDb's own, so these match exactly. Episodes are left out: the catalogue keeps series.

use crate::csv::Table;
use crate::{Entry, Kind};

fn kind(title_type: Option<&str>) -> Option<Option<Kind>> {
    match title_type.map(str::to_ascii_lowercase).as_deref() {
        Some(t) if t.contains("episode") => None,
        Some(t) if t.contains("series") => Some(Some(Kind::Series)),
        Some(_) => Some(Some(Kind::Film)),
        None => Some(None),
    }
}

pub fn read(t: &Table) -> Vec<Entry> {
    t.rows
        .iter()
        .filter_map(|row| {
            let kind = kind(t.get(row, "title type"))?;
            let imdb = t.get(row, "const").filter(|c| c.starts_with("tt"))?;
            let rated = t
                .get(row, "date rated")
                .filter(|d| d.len() >= 10)
                .map(|d| d[..10].to_string());
            Some(Entry {
                title: t.get(row, "title").unwrap_or_default().to_string(),
                year: t.get(row, "year").and_then(|y| y.parse().ok()),
                imdb: Some(imdb.to_string()),
                kind,
                rating: t.get(row, "your rating").and_then(|r| r.parse().ok()),
                watches: vec![rated],
                ..Default::default()
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rating_is_a_dated_watch_and_episodes_are_left_out() {
        let t = Table::parse(
            "Const,Your Rating,Date Rated,Title,Original Title,URL,Title Type,IMDb Rating,Runtime (mins),Year\n\
             tt0113277,9,2024-03-01,Heat,Heat,,Movie,8.3,170,1995\n\
             tt0903747,10,2023-01-02,Breaking Bad,Breaking Bad,,TV Series,9.5,49,2008\n\
             tt0959621,8,2023-01-03,Pilot,Pilot,,TV Episode,8.2,58,2008\n",
        )
        .unwrap();
        let e = read(&t);
        assert_eq!(e.len(), 2);
        assert_eq!(e[0].imdb.as_deref(), Some("tt0113277"));
        assert_eq!(
            (e[0].rating, e[0].year, e[0].kind),
            (Some(9.0), Some(1995), Some(Kind::Film))
        );
        assert_eq!(e[0].watches, vec![Some("2024-03-01".to_string())]);
        assert_eq!(e[1].kind, Some(Kind::Series));
    }
}
