//! Cast and crew, for core-tier titles only.
//!
//! `title.principals` is 101.5 million rows — the largest thing in this phase by a
//! wide margin, and the reason the two-tier scope exists. Only the 10.3% of rows
//! pointing at a core title are stored.
//!
//! # Why this runs in three steps
//!
//! `credits` references `people`, so people must exist first. But loading all of
//! `name.basics` would be most of a gigabyte of actors who appear in nothing we
//! kept. So:
//!
//!   1. **scan** — walk `title.principals`, collecting the person ids that core
//!      titles actually reference. Nothing is written.
//!   2. **people** — load `name.basics`, keeping only those.
//!   3. **credits** — load the principals rows themselves.
//!
//! Step 1's output is an in-memory set, so a resume that lands mid-phase redoes the
//! scan. That is a few minutes of CPU against tens of millions of rows it would
//! otherwise have to re-insert, and it keeps the alternative — persisting a
//! ten-million-entry working set to disk — out of a database the user is expected to
//! be able to copy (§2.4).

use std::collections::{HashMap, HashSet};
use std::path::Path;

use sinephile_persistence::Db;

use crate::imdb;
use crate::job::{Batch, Job, JobError, SqliteTx};
use crate::load::tconst_id;
use crate::tsv::TsvReader;

/// Rows per transaction. Credits rows are far narrower than titles, so a larger
/// batch costs the same wall time on a crash and commits far less often.
const BATCH: usize = 20_000;
const ROWS_PER_STATEMENT: usize = 150;

/// `nm0000001` → `1`.
fn nconst_id(raw: &str) -> Option<u32> {
    raw.strip_prefix("nm")?.parse().ok()
}

/// IMDb's `category` to the schema's `credits.role` (migration 0002).
///
/// Anything unrecognised is dropped rather than coerced. The `CHECK` constraint
/// would reject it anyway, and a whole batch failing because of one unexpected
/// category is worse than that credit being absent.
fn role(category: &str) -> Option<&'static str> {
    Some(match category {
        "director" => "director",
        "writer" => "writer",
        "actor" | "actress" | "self" => "actor",
        "composer" => "composer",
        "cinematographer" => "cinematographer",
        "editor" => "editor",
        "producer" => "producer",
        "production_designer" | "archive_footage" | "archive_sound" => return None,
        _ => return None,
    })
}

/// The internal ids for every core title, keyed by IMDb id.
///
/// Read once. The alternative — a join per credit row — is ten million lookups.
pub async fn core_title_ids(db: &Db) -> Result<HashMap<u32, i64>, JobError> {
    let rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT e.external_id, e.media_item_id
         FROM external_ids e
         JOIN media_items m ON m.id = e.media_item_id
         WHERE e.source = 'imdb' AND m.in_core = 1",
    )
    .fetch_all(db.pool())
    .await?;

    Ok(rows
        .into_iter()
        .filter_map(|(tconst, id)| tconst_id(&tconst).map(|n| (n, id)))
        .collect())
}

/// Step 1 — which people do core titles reference?
pub fn scan_needed_people(
    principals: &Path,
    core: &HashMap<u32, i64>,
) -> Result<HashSet<u32>, JobError> {
    let mut reader = TsvReader::open(principals)?;
    imdb::check_columns(&reader, &imdb::TITLE_PRINCIPALS)?;

    let mut needed = HashSet::with_capacity(2_000_000);
    while reader.advance()? {
        let Some(row) = reader.current_row() else {
            continue;
        };
        let Some(title) = row.get("tconst").and_then(tconst_id) else {
            continue;
        };
        if !core.contains_key(&title) {
            continue;
        }
        if row.get("category").and_then(role).is_none() {
            continue;
        }
        if let Some(person) = row.get("nconst").and_then(nconst_id) {
            needed.insert(person);
        }
    }
    Ok(needed)
}

/// Step 2 — load only the people a core title references.
pub async fn load_people(
    job: &mut Job<'_>,
    names: std::path::PathBuf,
    needed: std::sync::Arc<HashSet<u32>>,
) -> Result<(), JobError> {
    job.run_step("name.basics", move |tx, cursor| {
        let names = names.clone();
        let needed = std::sync::Arc::clone(&needed);
        Box::pin(async move {
            let mut reader = TsvReader::open(&names)?;
            imdb::check_columns(&reader, &imdb::NAME_BASICS)?;

            let mut pending: Vec<(u32, String, Option<i64>, Option<i64>)> = Vec::new();
            if let Some(cursor) = cursor.as_deref() {
                reader.seek_past("nconst", cursor)?;
                if let Some(person) = person_from(&reader, &needed) {
                    pending.push(person);
                }
            }

            let mut last_id = None;
            while pending.len() < BATCH {
                if !reader.advance()? {
                    break;
                }
                if let Some(person) = person_from(&reader, &needed) {
                    pending.push(person);
                }
                if let Some(row) = reader.current_row() {
                    if let Some(id) = row.get("nconst") {
                        last_id = Some(id.to_string());
                    }
                }
            }

            let count = pending.len() as i64;
            let finished = pending.len() < BATCH;
            insert_people(tx, &pending).await?;

            Ok(match (finished, last_id) {
                (false, Some(id)) => Batch::more(id, count),
                _ => Batch::finished(count),
            })
        })
    })
    .await?;
    Ok(())
}

fn person_from(
    reader: &TsvReader,
    needed: &HashSet<u32>,
) -> Option<(u32, String, Option<i64>, Option<i64>)> {
    let row = reader.current_row()?;
    let id = row.get("nconst").and_then(nconst_id)?;
    if !needed.contains(&id) {
        return None;
    }
    Some((
        id,
        row.get("primaryName")?.to_string(),
        row.parse::<i64>("birthYear"),
        row.parse::<i64>("deathYear"),
    ))
}

/// People carry their IMDb id in the `id` column directly.
///
/// `people.id` is `INTEGER PRIMARY KEY`, so an explicit id is allowed and makes the
/// credits insert a plain integer join instead of a lookup per row. `nm0000001`
/// becomes `1`, which is unique across IMDb by construction.
async fn insert_people(
    tx: &mut SqliteTx<'_>,
    people: &[(u32, String, Option<i64>, Option<i64>)],
) -> Result<(), JobError> {
    for chunk in people.chunks(ROWS_PER_STATEMENT) {
        let sql = format!(
            "INSERT INTO people (id, name, birth_year, death_year) VALUES {} \
             ON CONFLICT (id) DO NOTHING",
            vec!["(?, ?, ?, ?)"; chunk.len()].join(", ")
        );
        let mut query = sqlx::query(&sql);
        for (id, name, birth, death) in chunk {
            query = query.bind(*id as i64).bind(name).bind(birth).bind(death);
        }
        query
            .execute(&mut **tx)
            .await
            .map_err(|e| JobError::step("name.basics", format!("people: {e}")))?;
    }
    Ok(())
}

/// Step 3 — the credits themselves, for core titles only.
pub async fn load_credits(
    job: &mut Job<'_>,
    principals: std::path::PathBuf,
    core: std::sync::Arc<HashMap<u32, i64>>,
) -> Result<(), JobError> {
    job.run_step("title.principals", move |tx, cursor| {
        let principals = principals.clone();
        let core = std::sync::Arc::clone(&core);
        Box::pin(async move {
            let mut reader = TsvReader::open(&principals)?;
            imdb::check_columns(&reader, &imdb::TITLE_PRINCIPALS)?;

            let mut pending: Vec<Credit> = Vec::new();
            if let Some(cursor) = cursor.as_deref() {
                reader.seek_past("tconst", cursor)?;
                if let Some(credit) = credit_from(&reader, &core) {
                    pending.push(credit);
                }
            }

            let mut last_id = None;
            while pending.len() < BATCH {
                if !reader.advance()? {
                    break;
                }
                if let Some(credit) = credit_from(&reader, &core) {
                    pending.push(credit);
                }
                if let Some(row) = reader.current_row() {
                    if let Some(id) = row.get("tconst") {
                        last_id = Some(id.to_string());
                    }
                }
            }

            let count = pending.len() as i64;
            let finished = pending.len() < BATCH;
            insert_credits(tx, &pending).await?;

            Ok(match (finished, last_id) {
                (false, Some(id)) => Batch::more(id, count),
                _ => Batch::finished(count),
            })
        })
    })
    .await?;
    Ok(())
}

struct Credit {
    media_item_id: i64,
    person_id: i64,
    role: &'static str,
    character: String,
    billing: Option<i64>,
}

fn credit_from(reader: &TsvReader, core: &HashMap<u32, i64>) -> Option<Credit> {
    let row = reader.current_row()?;
    let media_item_id = *core.get(&row.get("tconst").and_then(tconst_id)?)?;
    let person_id = row.get("nconst").and_then(nconst_id)? as i64;
    let role = role(row.get("category")?)?;

    // IMDb writes characters as a JSON array: `["Kambei Shimada"]`. Taking the first
    // is enough for a cast list, and it stays a plain string rather than dragging a
    // JSON dependency into the loader for one field.
    let character = row
        .get("characters")
        .map(|raw| {
            raw.trim_matches(['[', ']'])
                .split("\",\"")
                .next()
                .unwrap_or("")
                .trim_matches('"')
                .replace("\\\"", "\"")
        })
        .unwrap_or_default();

    Some(Credit {
        media_item_id,
        person_id,
        role,
        character,
        billing: row.parse::<i64>("ordering"),
    })
}

async fn insert_credits(tx: &mut SqliteTx<'_>, credits: &[Credit]) -> Result<(), JobError> {
    for chunk in credits.chunks(ROWS_PER_STATEMENT) {
        // ON CONFLICT DO NOTHING because the primary key is
        // (media_item_id, person_id, role, character) and IMDb occasionally lists
        // the same person twice in the same role on one title.
        let sql = format!(
            "INSERT INTO credits (media_item_id, person_id, role, character, billing) \
             VALUES {} ON CONFLICT DO NOTHING",
            vec!["(?, ?, ?, ?, ?)"; chunk.len()].join(", ")
        );
        let mut query = sqlx::query(&sql);
        for credit in chunk {
            query = query
                .bind(credit.media_item_id)
                .bind(credit.person_id)
                .bind(credit.role)
                .bind(&credit.character)
                .bind(credit.billing);
        }
        query
            .execute(&mut **tx)
            .await
            .map_err(|e| JobError::step("title.principals", format!("credits: {e}")))?;
    }
    Ok(())
}

/// People and credit counts, for reporting.
pub async fn counts(db: &Db) -> Result<(i64, i64), JobError> {
    let people: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM people")
        .fetch_one(db.pool())
        .await?;
    let credits: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM credits")
        .fetch_one(db.pool())
        .await?;
    Ok((people, credits))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nconst_parses_to_its_numeric_part() {
        assert_eq!(nconst_id("nm0000001"), Some(1));
        assert_eq!(nconst_id("tt0000001"), None, "a title is not a person");
    }

    #[test]
    fn actress_and_self_both_become_actor() {
        // The schema has one `actor` role. Keeping IMDb's three would mean every
        // query for a cast list had to know all three.
        assert_eq!(role("actor"), Some("actor"));
        assert_eq!(role("actress"), Some("actor"));
        assert_eq!(role("self"), Some("actor"));
    }

    #[test]
    fn an_unknown_category_is_dropped_not_coerced() {
        // The CHECK constraint would reject it, failing a whole batch of 20,000 for
        // one row. Dropping the credit is the smaller loss.
        assert_eq!(role("production_designer"), None);
        assert_eq!(role("something_imdb_added_last_tuesday"), None);
    }

    #[test]
    fn every_mapped_role_is_one_the_schema_allows() {
        // migration 0002's CHECK constraint. If these drift, every insert fails.
        const ALLOWED: &[&str] = &[
            "director",
            "writer",
            "actor",
            "composer",
            "cinematographer",
            "editor",
            "producer",
            "studio",
            "creator",
        ];
        for category in [
            "director",
            "writer",
            "actor",
            "actress",
            "self",
            "composer",
            "cinematographer",
            "editor",
            "producer",
        ] {
            let mapped = role(category).expect("mapped");
            assert!(
                ALLOWED.contains(&mapped),
                "{category} maps to {mapped}, not in the CHECK"
            );
        }
    }
}
