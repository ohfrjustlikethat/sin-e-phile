//! What Settings says about each source the app keeps current (ADR-0054 step 7): when it
//! last updated and what it did, in words. Each job records its own report as a setting;
//! the setting's write time is the "last updated".

use sinephile_persistence::repositories::{
    MeaningRepository, MediaRepository, NeighbourRepository, NeighbourSource, ProfileRepository,
    RISING_KEY,
};
use sinephile_persistence::{Db, DbError};

/// Where MovieLens' build records itself (`neighbours::keep_current`).
pub const MOVIELENS_KEY: &str = "neighbours.movielens";

/// One source, as Settings shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    pub name: &'static str,
    /// When it last updated, ISO 8601 UTC; `None` before its first.
    pub updated_at: Option<String>,
    /// What it did, or what it is waiting for.
    pub detail: String,
}

/// A setting's write time ("YYYY-MM-DD HH:MM:SS", UTC) as ISO 8601, which a browser reads as
/// UTC rather than local time.
fn iso(written: &str) -> String {
    if written.ends_with('Z') {
        written.to_string()
    } else {
        format!("{}Z", written.replace(' ', "T"))
    }
}

fn n(x: i64) -> String {
    let digits = x.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, d) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(d);
    }
    out
}

fn field(record: &serde_json::Value, name: &str) -> i64 {
    record[name].as_i64().unwrap_or(0)
}

/// Every source, in the order the launch runs them.
pub async fn sources(db: &Db) -> Result<Vec<Source>, DbError> {
    let profiles = ProfileRepository::new(db);
    let written = |key: &'static str| {
        let profiles = &profiles;
        async move {
            Ok::<_, DbError>(profiles.setting_written(key).await?.map(|(value, at)| {
                (
                    serde_json::from_str::<serde_json::Value>(&value).unwrap_or_default(),
                    iso(&at),
                )
            }))
        }
    };
    let mut out = Vec::new();

    // The catalogue (ADR-0030): its record carries its own time.
    let imdb = profiles
        .setting(crate::freshness::LAST_REFRESH)
        .await?
        .and_then(|v| serde_json::from_str::<serde_json::Value>(&v).ok());
    out.push(match imdb {
        Some(r) => Source {
            name: "IMDb's catalogue",
            updated_at: r["at"].as_str().map(str::to_string),
            detail: format!(
                "{} new titles and {} new episodes",
                n(field(&r, "titles_added")),
                n(field(&r, "episodes_added"))
            ),
        },
        None => Source {
            name: "IMDb's catalogue",
            updated_at: None,
            detail: "Checked every hour; not updated since it was built".into(),
        },
    });

    // Meaning for titles without it (step 2).
    let counts = MeaningRepository::new(db).counts().await?;
    out.push(match written(crate::meaning::LAST_KEY).await? {
        Some((r, at)) => Source {
            name: "Meaning for new titles",
            updated_at: Some(at),
            detail: format!(
                "{} titles given meaning at the last update, {} in all",
                n(field(&r, "embedded")),
                n(counts.embedded)
            ),
        },
        None => Source {
            name: "Meaning for new titles",
            updated_at: None,
            detail: "Starts once search by meaning is set up".into(),
        },
    });

    // MovieLens' matrix (step 4).
    let neighbours = NeighbourRepository::new(db);
    let titles = neighbours.titles().await?;
    out.push(Source {
        name: "MovieLens",
        updated_at: written(MOVIELENS_KEY).await?.map(|(_, at)| at),
        detail: if titles > 0 {
            format!(
                "{} titles with neighbours; rebuilt when GroupLens publishes a new release",
                n(titles)
            )
        } else {
            "Not built yet; fetched from GroupLens at the next launch".into()
        },
    });

    // The live sources (step 3).
    let live = written(crate::live::LAST_KEY).await?;
    let (anilist_at, anilist_titles) = neighbours.last_asked(NeighbourSource::AniList).await?;
    out.push(Source {
        name: "AniList's recommendations",
        updated_at: anilist_at.as_deref().map(iso),
        detail: format!(
            "{} anime with recommendations; each asked again after a week",
            n(anilist_titles)
        ),
    });
    let (tmdb_at, tmdb_titles) = neighbours.last_asked(NeighbourSource::Tmdb).await?;
    let with_key = live
        .as_ref()
        .is_some_and(|(r, _)| r["tmdb"].as_bool() == Some(true));
    out.push(Source {
        name: "TMDB's recommendations",
        updated_at: tmdb_at.as_deref().map(iso),
        detail: if with_key || tmdb_titles > 0 {
            format!(
                "{} titles with recommendations; each asked again after a week",
                n(tmdb_titles)
            )
        } else {
            "Used with a profile's own TMDB key; none is set".into()
        },
    });

    // Rising now (step 6).
    let rising = MediaRepository::new(db).kept_rising().await?;
    out.push(Source {
        name: "Rising now",
        updated_at: profiles
            .setting_written(RISING_KEY)
            .await?
            .map(|(_, at)| iso(&at)),
        detail: if rising.is_empty() {
            "Starts at the next catalogue update".into()
        } else {
            format!("{} titles gaining the most ratings", n(rising.len() as i64))
        },
    });
    Ok(out)
}
