//! Adding the user's own links: pasted text, read as an M3U playlist.
//!
//! One path for both a whole playlist and a single link, because a single link with a
//! title is a one-entry playlist: `#EXTINF:-1,Nosferatu (1922)` then the URL.

use sinephile_persistence::repositories::DirectLinkRepository;
use sinephile_persistence::{Db, DbError};

use crate::backends::direct::parse_m3u;

/// Pasted text past this is not something a person pasted.
pub const MAX_IMPORT_BYTES: usize = 5 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("that is {0} bytes; a playlist can be at most 5 MB")]
    TooLarge(usize),
    #[error("no web addresses found — each link needs to start with http:// or https://")]
    Empty,
    #[error("{0}")]
    Store(#[from] DbError),
}

pub async fn import(db: &Db, text: &str) -> Result<usize, ImportError> {
    if text.len() > MAX_IMPORT_BYTES {
        return Err(ImportError::TooLarge(text.len()));
    }
    let links = parse_m3u(text);
    if links.is_empty() {
        return Err(ImportError::Empty);
    }
    let rows: Vec<(String, Option<i64>, String)> = links
        .into_iter()
        .map(|l| (l.title, l.year.map(i64::from), l.url))
        .collect();
    Ok(DirectLinkRepository::new(db).add(&rows).await?)
}

/// A single link, as the form sends it, written as the one-entry playlist it is.
pub fn single(title: &str, year: Option<u16>, url: &str) -> String {
    let name = match year {
        Some(y) => format!("{} ({y})", title.trim()),
        None => title.trim().to_string(),
    };
    format!("#EXTM3U\n#EXTINF:-1,{name}\n{}\n", url.trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_single_link_and_a_playlist_take_the_same_path() {
        let db = Db::in_memory().await.unwrap();
        let n = import(
            &db,
            &single("Nosferatu", Some(1922), "https://films.example/n.mp4"),
        )
        .await
        .unwrap();
        assert_eq!(n, 1);
        let rows = DirectLinkRepository::new(&db).all().await.unwrap();
        assert_eq!(
            (rows[0].title.as_str(), rows[0].year),
            ("Nosferatu", Some(1922))
        );

        assert!(matches!(
            import(&db, "just some words").await,
            Err(ImportError::Empty)
        ));
        let huge = "a".repeat(MAX_IMPORT_BYTES + 1);
        assert!(matches!(
            import(&db, &huge).await,
            Err(ImportError::TooLarge(_))
        ));
    }
}
