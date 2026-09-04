//! Checking the catalogue against the hand-checked fixture (exit criterion E5).
//!
//! `fixtures/anime/e5-hand-checked.tsv` holds fifty-odd AniList-to-IMDb mappings that
//! were verified against independent knowledge of what each work is — not by running
//! the matcher and writing down its answer. This module is what checks the database
//! against them.
//!
//! **A fixture generated from the code it tests cannot fail.** That is why the
//! expected IMDb ids are written by hand and why a disagreement here is worth
//! investigating in both directions: the matcher may be wrong, and so may the fixture.

use std::path::Path;

use sinephile_persistence::Db;

use crate::job::JobError;

/// What the fixture says must happen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Expect {
    /// Must map to exactly this IMDb id.
    Match,
    /// The matcher must refuse: several catalogue entries are equally good.
    Ambiguous,
    /// Resolves correctly, but an earlier entry of the same series holds the mapping.
    Claimed,
    /// Refused because the years disagree by more than the tolerance. A different
    /// failure from ambiguity and wanting a different fix, which is why it is a
    /// separate label even though the database cannot tell them apart.
    Conflict,
}

impl Expect {
    fn parse(raw: &str) -> Option<Self> {
        match raw {
            "match" => Some(Expect::Match),
            "ambiguous" => Some(Expect::Ambiguous),
            "claimed" => Some(Expect::Claimed),
            "conflict" => Some(Expect::Conflict),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Row {
    pub anilist_id: i64,
    pub imdb_id: Option<String>,
    pub expect: Expect,
    pub title: String,
    pub why: String,
}

#[derive(Debug, Clone)]
pub struct Outcome {
    pub row: Row,
    pub actual: Option<String>,
    pub passed: bool,
    pub detail: String,
}

pub fn load(path: &Path) -> Result<Vec<Row>, JobError> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| JobError::step("verify", format!("{}: {e}", path.display())))?;
    let mut rows = Vec::new();
    for line in text.lines() {
        let line = line.trim_end();
        if line.is_empty() || line.starts_with('#') || line.starts_with("anilist_id\t") {
            continue;
        }
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 5 {
            return Err(JobError::step(
                "verify",
                format!("expected 5 columns, got {}: {line}", f.len()),
            ));
        }
        let Ok(anilist_id) = f[0].parse::<i64>() else {
            return Err(JobError::step(
                "verify",
                format!("bad anilist id: {}", f[0]),
            ));
        };
        let Some(expect) = Expect::parse(f[2]) else {
            return Err(JobError::step("verify", format!("bad expect: {}", f[2])));
        };
        rows.push(Row {
            anilist_id,
            imdb_id: (f[1] != "-").then(|| f[1].to_string()),
            expect,
            title: f[3].to_string(),
            why: f[4].to_string(),
        });
    }
    Ok(rows)
}

/// Check every row against the database.
pub async fn run(db: &Db, rows: &[Row]) -> Result<Vec<Outcome>, JobError> {
    let mut out = Vec::new();
    for row in rows {
        // What the catalogue thinks this AniList entry is, and what IMDb id that
        // catalogue item carries.
        let actual: Option<String> = sqlx::query_scalar(
            "SELECT i.external_id
               FROM external_ids a
               JOIN external_ids i ON i.media_item_id = a.media_item_id AND i.source = 'imdb'
              WHERE a.source = 'anilist' AND a.external_id = ?",
        )
        .bind(row.anilist_id.to_string())
        .fetch_optional(db.pool())
        .await?;

        let (passed, detail) = match row.expect {
            Expect::Match => match (&row.imdb_id, &actual) {
                (Some(want), Some(got)) if want == got => (true, String::new()),
                (Some(want), Some(got)) => {
                    (false, format!("expected {want}, catalogue says {got}"))
                }
                (Some(want), None) => (false, format!("expected {want}, nothing mapped")),
                (None, _) => (false, "fixture row says match but gives no imdb id".into()),
            },
            // Both negative kinds mean the same thing in the database: this AniList
            // entry holds no mapping. They are distinguished in the fixture because
            // they fail for different reasons and want different fixes.
            Expect::Ambiguous | Expect::Claimed | Expect::Conflict => match &actual {
                None => (true, String::new()),
                Some(got) => (
                    false,
                    format!("expected no mapping, catalogue mapped it to {got}"),
                ),
            },
        };

        out.push(Outcome {
            row: row.clone(),
            actual,
            passed,
            detail,
        });
    }
    Ok(out)
}

pub fn report(outcomes: &[Outcome]) -> bool {
    let failed: Vec<&Outcome> = outcomes.iter().filter(|o| !o.passed).collect();
    let passed = outcomes.len() - failed.len();

    println!();
    println!("  E5 hand-checked anime mappings");
    println!("  {passed}/{} pass", outcomes.len());

    if !failed.is_empty() {
        println!();
        for o in &failed {
            println!("    FAIL  {:<38}  {}", o.row.title, o.detail);
            println!("          {}", o.row.why);
        }
    }
    println!();
    failed.is_empty()
}
