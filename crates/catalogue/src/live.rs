//! "People who loved this also loved", live (ADR-0054 step 3): AniList's community
//! recommendations for anime (no key) and TMDB's for films and series (a profile's own key,
//! ADR-0013) stand beside MovieLens' matrix in the neighbour graph, each row marked with its
//! source. MovieLens is a snapshot GroupLens updates rarely; these change every day, and
//! they know this month's releases.
//!
//! # Who is asked about, and when
//!
//! The titles a profile liked or finished (`ProfileRepository::liked`, strongest first,
//! every profile), because the ranker reads the neighbours *of* what a profile loved: a
//! new release reaches a rail by being recommended beside something loved. Each title is
//! asked of each source once a [`WEEK`], answered or not, and a launch asks at most
//! [`ANILIST_PER_LAUNCH`] and [`TMDB_PER_LAUNCH`].
//!
//! # What a row is worth
//!
//! A live source gives an order, not a number on MovieLens' scale, so its k-th
//! recommendation scores what MovieLens' k-th neighbour scores on average
//! (`NeighbourRepository::typical_scores`): neither source outweighs the other by the
//! accident of its scale.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use sinephile_metadata_api::anilist::AniList;
use sinephile_metadata_api::tmdb::{Now, Tmdb};
use sinephile_metadata_api::Transport;
use sinephile_persistence::repositories::{
    CredentialRepository, MediaRepository, NeighbourRepository, NeighbourSource, ProfileRepository,
    TmdbAccess,
};
use sinephile_persistence::{Db, IdSource};

use crate::job::JobError;

/// How long a title's live neighbours stand before it is asked again.
pub const WEEK: i64 = 7;

/// AniList titles a launch asks about: four requests of fifty.
pub const ANILIST_PER_LAUNCH: usize = 200;

/// TMDB titles a launch asks about: each is a find, a page of recommendations, and the IMDb
/// id of each recommendation not met before (about twenty requests the first time, two
/// once its neighbours are known).
pub const TMDB_PER_LAUNCH: usize = 25;

/// Where a launch records what this did, for Settings.
pub const LAST_KEY: &str = "neighbours.live";

/// What a launch did.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Report {
    pub anilist_asked: usize,
    pub anilist_rows: usize,
    pub tmdb_asked: usize,
    pub tmdb_rows: usize,
}

fn step(e: impl std::fmt::Display) -> JobError {
    JobError::step("live neighbours", e.to_string())
}

/// What the k-th (from 0) of a live source's list is worth beside MovieLens: the matrix's
/// mean at that rank, its last past the end; before there is a matrix, a curve fitted to
/// the one measured on ml-latest (0.29 at the first, 0.18 at the tenth, 0.12 at the
/// fiftieth).
pub fn score_at(typical: &[f32], rank: usize) -> f32 {
    typical
        .get(rank)
        .or(typical.last())
        .copied()
        .unwrap_or_else(|| 0.29 / (1.0 + rank as f32).powf(0.25))
}

/// TMDB's films and series have separate ids, so a cached TMDB id says which.
fn tmdb_key(what: Now, id: i64) -> String {
    match what {
        Now::Films => format!("movie/{id}"),
        Now::Series => format!("tv/{id}"),
    }
}

/// Every profile's liked titles, strongest first, each once.
async fn wanted(db: &Db) -> Result<Vec<i64>, JobError> {
    let profiles = ProfileRepository::new(db);
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for p in profiles.all().await.map_err(step)? {
        for (id, _) in profiles.liked(p.id).await.map_err(step)? {
            if seen.insert(id) {
                out.push(id);
            }
        }
    }
    Ok(out)
}

/// Ask the live sources about the titles that are due. Either source may be absent: no key,
/// or a test.
pub async fn keep_current(
    db: &Db,
    anilist: Option<&AniList<'_>>,
    tmdb: Option<&Tmdb>,
) -> Result<Report, JobError> {
    let repo = NeighbourRepository::new(db);
    let media = MediaRepository::new(db);
    let wanted = wanted(db).await?;
    let typical = repo.typical_scores(50).await.map_err(step)?;
    let mut report = Report::default();

    if let Some(anilist) = anilist {
        let ids = media
            .external_ids_of(IdSource::Anilist, &wanted)
            .await
            .map_err(step)?;
        let anime: Vec<i64> = wanted
            .iter()
            .copied()
            .filter(|w| ids.contains_key(w))
            .collect();
        let due = repo
            .due(NeighbourSource::AniList, &anime, WEEK)
            .await
            .map_err(step)?;
        let due: Vec<i64> = due.into_iter().take(ANILIST_PER_LAUNCH).collect();
        for group in due.chunks(50) {
            let asked: Vec<i64> = group.iter().filter_map(|id| ids[id].parse().ok()).collect();
            let answers = match anilist.recommendations(&asked).await {
                Ok(answers) => answers,
                Err(error) => {
                    tracing::info!(%error, "live neighbours: AniList stopped answering");
                    break;
                }
            };
            let targets: Vec<String> = answers
                .iter()
                .flat_map(|(_, recs)| recs.iter().map(|(t, _)| t.to_string()))
                .collect();
            let items = media
                .items_by_external(IdSource::Anilist, &targets)
                .await
                .map_err(step)?;
            let by_anilist: HashMap<i64, Vec<(i64, i64)>> = answers.into_iter().collect();
            for &item in group {
                let Ok(own) = ids[&item].parse::<i64>() else {
                    continue;
                };
                // Ranked by the source's own order, so a title the catalogue lacks leaves
                // its place empty rather than promoting the next.
                let rows: Vec<(i64, f32)> = by_anilist
                    .get(&own)
                    .into_iter()
                    .flatten()
                    .enumerate()
                    .filter_map(|(rank, (t, _))| {
                        Some((*items.get(&t.to_string())?, score_at(&typical, rank)))
                    })
                    .collect();
                repo.replace_for(item, NeighbourSource::AniList, &rows)
                    .await
                    .map_err(step)?;
                report.anilist_asked += 1;
                report.anilist_rows += rows.len();
            }
        }
    }

    if let Some(tmdb) = tmdb {
        let imdb = media
            .external_ids_of(IdSource::Imdb, &wanted)
            .await
            .map_err(step)?;
        let titled: Vec<i64> = wanted
            .iter()
            .copied()
            .filter(|w| imdb.contains_key(w))
            .collect();
        let due = repo
            .due(NeighbourSource::Tmdb, &titled, WEEK)
            .await
            .map_err(step)?;
        'titles: for item in due.into_iter().take(TMDB_PER_LAUNCH) {
            let found = match tmdb.find(&imdb[&item]).await {
                Ok(found) => found,
                Err(error) => {
                    tracing::info!(%error, "live neighbours: TMDB stopped answering");
                    break;
                }
            };
            let mut rows = Vec::new();
            if let Some((what, id)) = found {
                remember(&media, item, &tmdb_key(what, id)).await;
                let recs = match tmdb.recommendations(what, id).await {
                    Ok(recs) => recs,
                    Err(error) => {
                        tracing::info!(%error, "live neighbours: TMDB stopped answering");
                        break;
                    }
                };
                let keys: Vec<String> = recs.iter().map(|r| tmdb_key(what, *r)).collect();
                let known = media
                    .items_by_external(IdSource::Tmdb, &keys)
                    .await
                    .map_err(step)?;
                for (rank, (r, key)) in recs.iter().zip(&keys).enumerate() {
                    let neighbour = match known.get(key) {
                        Some(&n) => Some(n),
                        // Met for the first time: its IMDb id finds it in the catalogue, and
                        // its TMDB id is kept so it is never looked up again.
                        None => match tmdb.imdb_id(what, *r).await {
                            Ok(Some(i)) => {
                                let n = media
                                    .items_by_external(IdSource::Imdb, &[i])
                                    .await
                                    .map_err(step)?
                                    .into_values()
                                    .next();
                                if let Some(n) = n {
                                    remember(&media, n, key).await;
                                }
                                n
                            }
                            Ok(None) => None,
                            Err(error) => {
                                tracing::info!(%error, "live neighbours: TMDB stopped answering");
                                break 'titles;
                            }
                        },
                    };
                    if let Some(n) = neighbour {
                        rows.push((n, score_at(&typical, rank)));
                    }
                }
            }
            repo.replace_for(item, NeighbourSource::Tmdb, &rows)
                .await
                .map_err(step)?;
            report.tmdb_asked += 1;
            report.tmdb_rows += rows.len();
        }
    }
    Ok(report)
}

/// Keep a title's TMDB id. Another title may hold it already (a duplicate in the
/// catalogue): the first keeps it, and this is not worth stopping for.
async fn remember(media: &MediaRepository<'_>, item: i64, key: &str) {
    if let Err(error) = media.add_external_id(item, IdSource::Tmdb, key, 1.0).await {
        tracing::debug!(%error, item, key, "TMDB id not kept");
    }
}

/// What the app runs at launch and at each hourly check: AniList always, TMDB with the first
/// profile's own key that has one (the neighbours are facts about titles, shared by every
/// profile like the rest of the catalogue). Its report is recorded under [`LAST_KEY`].
pub async fn at_launch(db: &Db, transport: Arc<dyn Transport>) -> Result<Report, JobError> {
    let mut key = None;
    for p in ProfileRepository::new(db).all().await.map_err(step)? {
        if let Ok(TmdbAccess::Configured(k)) = CredentialRepository::new(db).tmdb_access(p.id).await
        {
            key = Some(k);
            break;
        }
    }
    let tmdb = key.map(|k| Tmdb::new(transport.clone(), k));
    let anilist = AniList::new(transport.as_ref()).await;
    let report = keep_current(db, Some(&anilist), tmdb.as_ref()).await?;
    let record = format!(
        r#"{{"anilist_asked":{},"anilist_rows":{},"tmdb_asked":{},"tmdb_rows":{},"tmdb":{}}}"#,
        report.anilist_asked,
        report.anilist_rows,
        report.tmdb_asked,
        report.tmdb_rows,
        tmdb.is_some()
    );
    ProfileRepository::new(db)
        .set_setting(LAST_KEY, &record)
        .await
        .map_err(step)?;
    Ok(report)
}
