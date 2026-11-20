//! Rating many films one after another (ADR-0056, the author: a rate-a-batch screen, so a
//! taste is built quickly). Well-known films, most voted first, because a person has most
//! likely seen those and a rating of something seen is the strongest signal there is.
//! Never one already rated, skipped ("haven't seen it") or dismissed; for a kids profile,
//! only what suits children.

use sinephile_persistence::repositories::{MediaRepository, ProfileRepository};
use sinephile_persistence::{Db, DbError};

/// Films with fewer IMDb votes than this are not "well known" enough to ask about.
pub const MIN_VOTES: i64 = 100_000;

/// The next `n` films to ask this profile to rate.
pub async fn batch(db: &Db, profile: i64, kids: bool, n: usize) -> Result<Vec<i64>, DbError> {
    let profiles = ProfileRepository::new(db);
    let media = MediaRepository::new(db);
    let mut out = Vec::new();
    let mut offset = 0;
    // A kids profile leaves out most of what is asked, so it reads further.
    let page = (n as i64).max(1) * if kids { 6 } else { 1 };
    while out.len() < n {
        let ids = profiles.to_rate(profile, MIN_VOTES, offset, page).await?;
        if ids.is_empty() {
            break;
        }
        offset += ids.len() as i64;
        let ids = if kids {
            let ok = media.suitable_for_children(&ids).await?;
            ids.into_iter().filter(|id| ok.contains(id)).collect()
        } else {
            ids
        };
        out.extend(ids.into_iter().take(n - out.len()));
    }
    Ok(out)
}
