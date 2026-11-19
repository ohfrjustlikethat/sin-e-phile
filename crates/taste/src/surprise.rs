//! Surprise me (SPEC.md Phase 18, ADR-0043): one film the viewer will probably like, from their
//! taste and their sources, opened ready to play.
//!
//! The film is one Home would recommend now ([`crate::home::rails_at`], the same plan): from
//! the taste rail and the discovery rails, or, for a profile with no taste yet, from new
//! releases and the widely loved. Films only, and within the length limit when one is asked
//! (I42). What is on this PC comes first, since it plays at once. Of the first [`FROM`] that
//! qualify, one at random: asking again gives another.

use std::collections::HashSet;

use sinephile_persistence::model::MediaKind;
use sinephile_persistence::repositories::{LibraryRepository, MediaRepository};
use sinephile_persistence::{Db, DbError};
use sinephile_recommend::bandit::Rng;
use sinephile_recommend::strategy::Arm;
use sinephile_search_engine::Semantic;

use crate::home::{rails_at, Ask};

/// How many of the best qualifying films the pick is drawn from.
pub const FROM: usize = 5;

/// The film chosen.
#[derive(Debug, Clone, PartialEq)]
pub struct Surprise {
    pub id: i64,
    /// Why it was chosen, as its card on Home says it.
    pub note: Option<String>,
    /// In the library: it plays at once.
    pub on_this_pc: bool,
}

/// Choose one, `seed` making the draw (a new one each press).
#[allow(clippy::too_many_arguments)]
pub async fn surprise(
    db: &Db,
    semantic: Option<&mut Semantic>,
    profile: i64,
    kids: bool,
    love: Option<&str>,
    cache: &crate::model::ModelCache,
    ask: Ask,
    seed: u64,
) -> Result<Option<Surprise>, DbError> {
    let rails = rails_at(db, semantic, profile, kids, love, cache, ask).await?;
    let recommended =
        |key: &str| key == "taste" || Arm::from_key(key).is_some_and(|a| a != Arm::Familiar);
    let mut order: Vec<(i64, Option<String>)> = Vec::new();
    for want in [recommended as fn(&str) -> bool, |k: &str| {
        matches!(k, "new" | "loved")
    }] {
        for rail in rails.iter().filter(|r| want(r.key)) {
            for id in &rail.items {
                if !order.iter().any(|(o, _)| o == id) {
                    order.push((*id, rail.notes.get(id).cloned()));
                }
            }
        }
        if !order.is_empty() {
            break;
        }
    }

    // Films only: a series is not one evening's surprise.
    let media = MediaRepository::new(db);
    let mut films: Vec<(i64, Option<String>)> = Vec::new();
    for (id, note) in order {
        if let Some(item) = media.by_id(id).await? {
            if matches!(item.kind, MediaKind::Film | MediaKind::AnimeFilm) {
                films.push((id, note));
            }
        }
    }
    let here: HashSet<i64> = LibraryRepository::new(db)
        .library_titles(false)
        .await?
        .into_iter()
        .collect();
    let local: Vec<&(i64, Option<String>)> =
        films.iter().filter(|(id, _)| here.contains(id)).collect();
    let pool: Vec<&(i64, Option<String>)> = if local.is_empty() {
        films.iter().collect()
    } else {
        local
    };
    if pool.is_empty() {
        return Ok(None);
    }
    let from = pool.len().min(FROM);
    let at = ((Rng::new(seed).unit() * from as f64) as usize).min(from - 1);
    let (id, note) = pool[at].clone();
    Ok(Some(Surprise {
        id,
        note,
        on_this_pc: here.contains(&id),
    }))
}
