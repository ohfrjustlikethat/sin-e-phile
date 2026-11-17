//! What is showing now, for a profile with its own TMDB key (Phase 16, ADR-0013, ADR-0027).
//!
//! The offline catalogue knows years, not release dates, so "new releases" from it alone is
//! "this year, most watched". TMDB knows what is in cinemas and on the air this week. Once a
//! day, for each profile with a key, that list is fetched, matched to the catalogue by IMDb
//! id, and kept for the profile (`NOW_KEY`); Home's "New releases" puts it first
//! (`sinephile_taste::home`). Without a key nothing changes.

use sinephile_metadata_api::tmdb::{Now, Tmdb, TmdbError};
use sinephile_persistence::repositories::{CredentialRepository, MediaRepository};
use sinephile_persistence::{Db, DbError, IdSource};

/// Where a profile's list is kept: `{"day":"YYYY-MM-DD","ids":[…]}`. Read by Home.
pub const NOW_KEY: &str = "tmdb.now";

#[derive(Debug, thiserror::Error)]
pub enum NowError {
    #[error(transparent)]
    Tmdb(#[from] TmdbError),
    #[error(transparent)]
    Db(#[from] DbError),
}

/// Fetch what is showing now (films in cinemas, then series on the air), match it to the
/// catalogue, and keep it for `profile` as of `day`. Returns how many matched.
pub async fn refresh(db: &Db, tmdb: &Tmdb, profile: i64, day: &str) -> Result<usize, NowError> {
    let media = MediaRepository::new(db);
    let mut ids: Vec<i64> = Vec::new();
    for (what, pages) in [(Now::Films, 2), (Now::Series, 1)] {
        for imdb in tmdb.now_imdb(what, pages).await? {
            if let Some(m) = media.by_external_id(IdSource::Imdb, &imdb).await? {
                if !ids.contains(&m.id) {
                    ids.push(m.id);
                }
            }
        }
    }
    let list = ids.iter().map(i64::to_string).collect::<Vec<_>>().join(",");
    CredentialRepository::new(db)
        .set(
            profile,
            NOW_KEY,
            &format!(r#"{{"day":"{day}","ids":[{list}]}}"#),
        )
        .await?;
    Ok(ids.len())
}

/// The day a profile's list was last fetched, if it was.
pub async fn fetched_on(db: &Db, profile: i64) -> Result<Option<String>, DbError> {
    let raw = CredentialRepository::new(db).get(profile, NOW_KEY).await?;
    // Written by `refresh` alone, in one shape: the day is the ten characters after it.
    Ok(raw.and_then(|r| {
        let at = r.find(r#""day":""#)? + 7;
        r.get(at..at + 10).map(str::to_string)
    }))
}
