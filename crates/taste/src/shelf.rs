//! The catalogue under Films and TV Shows (Phase 16, the author's brief item 9): "On this PC" first, then rails
//! from the whole catalogue, so neither tab is ever empty, whatever is or is not in the library. Most watched, new
//! this year, then the main genres, by votes. A kids profile sees only what suits children, as on Home.
//!
//! The genres are the ones whose rail can be read quickly from the votes index (0–90 ms each on 3.1 million
//! titles, measured 2026-11-17); a rarer genre (documentaries, westerns) walks far down it and is left to
//! Phase 19's browsing.

use sinephile_persistence::repositories::MediaRepository;
use sinephile_persistence::{Db, DbError};

use crate::home::Rail;

const PER_RAIL: i64 = 24;
/// New this year means people are watching it: this many votes.
const NEW_VOTES: i64 = 500;

const FILM_GENRES: [&str; 8] = [
    "Drama",
    "Comedy",
    "Action",
    "Thriller",
    "Crime",
    "Sci-Fi",
    "Animation",
    "Horror",
];
const SHOW_GENRES: [&str; 6] = ["Drama", "Comedy", "Crime", "Animation", "Sci-Fi", "Mystery"];

/// The calendar year now, as Home's rails reckon it.
pub fn this_year() -> i64 {
    let today = crate::model::day_of(&crate::home::now_iso()).unwrap_or(0.0);
    (today / 365.2425 + 1970.0).floor() as i64
}

/// The rails for Films (or, with `shows`, TV Shows), in order; empty ones left out.
pub async fn catalogue_rails(
    db: &Db,
    shows: bool,
    kids: bool,
    year: i64,
) -> Result<Vec<Rail>, DbError> {
    let media = MediaRepository::new(db);
    let suits = |ids: Vec<i64>| async move {
        if !kids {
            return Ok::<_, DbError>(ids);
        }
        let ok = MediaRepository::new(db).suitable_for_children(&ids).await?;
        Ok(ids.into_iter().filter(|id| ok.contains(id)).collect())
    };
    let mut out = Vec::new();
    let mut push = |key: &'static str, title: &'static str, items: Vec<i64>| {
        if !items.is_empty() {
            out.push(Rail {
                key,
                title: title.into(),
                items,
                notes: Default::default(),
                progress: Default::default(),
            });
        }
    };

    push(
        "most",
        if shows {
            "Most watched shows"
        } else {
            "Most watched films"
        },
        suits(media.most_voted_of(shows, None, PER_RAIL).await?).await?,
    );
    // Early in a year there is little out yet: last year's too, until there is enough.
    let mut fresh = media.new_of(shows, year, NEW_VOTES, PER_RAIL).await?;
    let mut title = "New this year";
    if fresh.len() < 8 {
        fresh = media.new_of(shows, year - 1, NEW_VOTES, PER_RAIL).await?;
        title = "New this year and last";
    }
    push("new", title, suits(fresh).await?);
    let genres: &[&'static str] = if shows { &SHOW_GENRES } else { &FILM_GENRES };
    for &g in genres {
        push(
            g,
            g,
            suits(media.most_voted_of(shows, Some(g), PER_RAIL).await?).await?,
        );
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sinephile_persistence::NewMediaItem;

    async fn titled(db: &Db, film: NewMediaItem, genre: &str) -> i64 {
        let id = MediaRepository::new(db).insert(&film).await.unwrap();
        sqlx::query("INSERT OR IGNORE INTO genres (name) VALUES (?)")
            .bind(genre)
            .execute(db.pool())
            .await
            .unwrap();
        sqlx::query("INSERT INTO media_genres (media_item_id, genre_id) SELECT ?, id FROM genres WHERE name = ?")
            .bind(id)
            .bind(genre)
            .execute(db.pool())
            .await
            .unwrap();
        id
    }

    #[tokio::test]
    async fn films_get_rails_of_the_catalogue_and_shows_get_their_own() {
        let db = Db::in_memory().await.unwrap();
        let heat = titled(
            &db,
            NewMediaItem {
                rating_votes: Some(900_000),
                ..NewMediaItem::film("Heat", 1995)
            },
            "Crime",
        )
        .await;
        let new = titled(
            &db,
            NewMediaItem {
                rating_votes: Some(2_000),
                ..NewMediaItem::film("Sinners", 2025)
            },
            "Horror",
        )
        .await;
        let rails = catalogue_rails(&db, false, false, 2026).await.unwrap();
        let keys: Vec<_> = rails.iter().map(|r| r.key).collect();
        assert_eq!(
            keys,
            ["most", "new", "Crime", "Horror"],
            "empty genres are left out"
        );
        assert_eq!(rails[0].items, [heat, new]);
        assert_eq!(
            rails[1].title, "New this year and last",
            "too little out this year"
        );
        assert_eq!(rails[1].items, [new]);
        assert!(
            catalogue_rails(&db, true, false, 2026)
                .await
                .unwrap()
                .is_empty(),
            "no shows"
        );
    }
}
