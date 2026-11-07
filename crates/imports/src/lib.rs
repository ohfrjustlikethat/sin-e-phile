//! Imports (SPEC.md Phase 14): a viewing history from elsewhere, so the taste model starts
//! from what a person has actually watched rather than a questionnaire.
//!
//! - [`letterboxd`]: the export `.zip`, or one of its CSVs.
//! - [`imdb`]: the ratings CSV.
//! - [`trakt`]: the JSON export (watched, ratings, watchlist; a `.zip` of them too).
//! - [`mal`]: MyAnimeList's XML, which AniList also exports.
//!
//! Each is read into [`Entry`]s; [`import`] matches them to the catalogue (an IMDb or MAL
//! id where the file gives one, otherwise the exact title and year) and records them for
//! one profile: ratings, dated watches and the watchlist. What cannot be matched is listed
//! in words, never guessed at. Importing the same file twice adds nothing.

pub mod csv;
pub mod imdb;
pub mod letterboxd;
pub mod mal;
pub mod trakt;

use std::collections::HashMap;
use std::path::Path;

use sinephile_persistence::model::{IdSource, MediaItem, MediaKind};
use sinephile_persistence::repositories::{MediaRepository, ProfileRepository};
use sinephile_persistence::{Db, DbError};

/// What the file says the title is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Film,
    Series,
    Anime,
}

/// One title from an export, in the export's own words.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Entry {
    pub title: String,
    pub year: Option<i64>,
    /// "tt0113277", when the file names it.
    pub imdb: Option<String>,
    /// MyAnimeList's id, when the file names it.
    pub mal: Option<i64>,
    pub kind: Option<Kind>,
    /// Out of 10 (a Letterboxd half-star is 1).
    pub rating: Option<f64>,
    /// Each watch, as "YYYY-MM-DD" when dated, `None` when the export only says watched.
    pub watches: Vec<Option<String>>,
    pub watchlist: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("could not read {0}: {1}")]
    Read(String, String),
    #[error("this is not an export sin-e-phile knows: {0}")]
    Unknown(String),
    #[error(transparent)]
    Db(#[from] DbError),
}

/// Where an export came from, as the app names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Letterboxd,
    Trakt,
    Imdb,
    Mal,
}

impl Format {
    pub fn name(self) -> &'static str {
        match self {
            Self::Letterboxd => "Letterboxd",
            Self::Trakt => "Trakt",
            Self::Imdb => "IMDb",
            Self::Mal => "MyAnimeList",
        }
    }
}

/// Read an export, whichever it is: by what is inside, not only the file's name.
pub fn read(path: &Path) -> Result<(Format, Vec<Entry>), ImportError> {
    let name = path.display().to_string();
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".zip") {
        let names = zip_names(path)?;
        if names.iter().any(|n| n.ends_with(".json")) {
            return Ok((Format::Trakt, trakt::read_zip(path)?));
        }
        return Ok((Format::Letterboxd, letterboxd::read_zip(path)?));
    }
    let bytes = std::fs::read(path).map_err(|e| ImportError::Read(name.clone(), e.to_string()))?;
    let text = String::from_utf8_lossy(&bytes);
    if lower.ends_with(".xml")
        || text.trim_start().starts_with("<?xml")
        || text.contains("<myanimelist>")
    {
        return Ok((Format::Mal, mal::read(&text)?));
    }
    if lower.ends_with(".json") {
        return Ok((Format::Trakt, trakt::read(&text, &lower)?));
    }
    let table = csv::Table::parse(&text).ok_or_else(|| ImportError::Unknown(name.clone()))?;
    if table.has("const") && table.has("your rating") {
        return Ok((Format::Imdb, imdb::read(&table)));
    }
    if table.has("name") && table.has("year") {
        let file = Path::new(&lower)
            .file_name()
            .map(|f| f.to_string_lossy().into_owned())
            .unwrap_or_default();
        return Ok((Format::Letterboxd, letterboxd::read_one(&file, &table)));
    }
    Err(ImportError::Unknown(name))
}

fn zip_names(path: &Path) -> Result<Vec<String>, ImportError> {
    let name = path.display().to_string();
    let file =
        std::fs::File::open(path).map_err(|e| ImportError::Read(name.clone(), e.to_string()))?;
    let zip = zip::ZipArchive::new(file).map_err(|e| ImportError::Read(name, e.to_string()))?;
    Ok(zip.file_names().map(|n| n.to_ascii_lowercase()).collect())
}

/// Every text file in a `.zip`, by its lower-case name.
pub(crate) fn zip_texts(path: &Path) -> Result<HashMap<String, String>, ImportError> {
    use std::io::Read;
    let name = path.display().to_string();
    let file =
        std::fs::File::open(path).map_err(|e| ImportError::Read(name.clone(), e.to_string()))?;
    let mut zip =
        zip::ZipArchive::new(file).map_err(|e| ImportError::Read(name.clone(), e.to_string()))?;
    let mut out = HashMap::new();
    for i in 0..zip.len() {
        let mut f = zip
            .by_index(i)
            .map_err(|e| ImportError::Read(name.clone(), e.to_string()))?;
        if f.is_dir() {
            continue;
        }
        let mut text = String::new();
        if f.read_to_string(&mut text).is_ok() {
            out.insert(f.name().to_ascii_lowercase(), text);
        }
    }
    Ok(out)
}

/// The app's 1-5 stars for a rating out of 10: halves round up (a Letterboxd 3.5 is 4).
pub fn stars(out_of_ten: f64) -> Option<i64> {
    (out_of_ten > 0.0).then(|| ((out_of_ten / 2.0).ceil() as i64).clamp(1, 5))
}

/// A title the import could not place, and why, for the user to read.
#[derive(Debug, Clone, PartialEq)]
pub struct Unmatched {
    pub title: String,
    pub year: Option<i64>,
    pub why: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Summary {
    pub entries: usize,
    pub matched: usize,
    pub ratings: usize,
    pub watches: usize,
    pub watchlist: usize,
    pub unmatched: Vec<Unmatched>,
}

fn kind_fits(kind: Option<Kind>, m: &MediaItem) -> bool {
    let film = matches!(m.kind, MediaKind::Film | MediaKind::AnimeFilm);
    let series = matches!(m.kind, MediaKind::Series | MediaKind::AnimeSeries);
    match kind {
        None => film || series,
        Some(Kind::Film) => film,
        Some(Kind::Series) => series,
        Some(Kind::Anime) => film || series,
    }
}

/// A name as compared when the exact name found nothing: lower case, "&" read as "and",
/// a leading "the" dropped, and nothing but letters, digits and single spaces kept. So
/// "Lock, Stock & Two Smoking Barrels" meets "Lock, Stock and Two Smoking Barrels", and
/// "Who Framed Roger Rabbit?" meets "Who Framed Roger Rabbit".
pub fn normalised(name: &str) -> String {
    let lower = name.to_lowercase().replace('&', " and ").replace('-', " ");
    let kept: String = lower
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect();
    let words: Vec<&str> = kept.split_whitespace().collect();
    let words = match words.first() {
        Some(&"the") if words.len() > 1 => &words[1..],
        _ => &words[..],
    };
    words.join(" ")
}

/// One candidate for a name: (catalogue id, release year, votes).
pub type Candidate = (i64, Option<i64>, i64);

/// Which candidate an entry from `year` is, or why it is asked about.
///
/// - The exact year, unless a title a year either side is plainly the one meant: a hundred
///   times its votes (release years differ by a year between sources often, and "300" of
///   2006 has 924,388 votes where a "300" of 2007 has 15).
/// - Two at one year are told apart only when one has ten times the votes of the next.
/// - Otherwise a year either side, on the same terms.
pub fn choose(candidates: &[Candidate], year: i64) -> Result<i64, String> {
    let at = |d: i64| -> Vec<Candidate> {
        let mut v: Vec<Candidate> = candidates
            .iter()
            .copied()
            .filter(|(_, y, _)| y.is_some_and(|y| (y - year).abs() == d))
            .collect();
        v.sort_by_key(|c| std::cmp::Reverse(c.2));
        v.dedup_by_key(|c| c.0);
        v
    };
    let plainly = |v: &[Candidate]| -> Option<i64> {
        match v {
            [one] => Some(one.0),
            [first, second, ..] if first.2 >= 10 * second.2.max(1) => Some(first.0),
            _ => None,
        }
    };
    let exact = at(0);
    let near = at(1);
    if let (Some(e), Some(n)) = (exact.first(), near.first()) {
        if n.2 >= 100 * e.2.max(1) {
            return plainly(&near)
                .ok_or_else(|| format!("more than one title by that name near {year}"));
        }
    }
    if !exact.is_empty() {
        return plainly(&exact)
            .ok_or_else(|| format!("more than one title by that name from {year}"));
    }
    match near.as_slice() {
        [] => Err(format!("no title by that name from {year}")),
        v => plainly(v).ok_or_else(|| format!("more than one title by that name near {year}")),
    }
}

/// The catalogue item an entry is, or why not.
pub async fn place(db: &Db, e: &Entry) -> Result<Result<i64, String>, DbError> {
    let media = MediaRepository::new(db);
    if let Some(tt) = &e.imdb {
        if let Some(m) = media.by_external_id(IdSource::Imdb, tt).await? {
            return Ok(Ok(m.id));
        }
    }
    if let Some(mal) = e.mal {
        if let Some(m) = media
            .by_external_id(IdSource::Mal, &mal.to_string())
            .await?
        {
            return Ok(Ok(m.id));
        }
    }
    let candidates: Vec<MediaItem> = media
        .by_exact_title(e.title.trim())
        .await?
        .into_iter()
        .filter(|m| kind_fits(e.kind, m))
        .collect();
    let Some(year) = e.year else {
        // Without a year only an unambiguous name is taken.
        return Ok(match candidates.as_slice() {
            [one] => Ok(one.id),
            [] => Err("no title by that name in the catalogue".into()),
            _ => Err("more than one title by that name, and no year to tell them apart".into()),
        });
    };
    if !candidates.is_empty() {
        let c: Vec<Candidate> = candidates
            .iter()
            .map(|m| (m.id, m.release_year, m.rating_votes.unwrap_or(0)))
            .collect();
        return Ok(choose(&c, year));
    }
    // The name written differently: compared normalised, a year either side.
    let wanted = normalised(&e.title);
    if wanted.is_empty() {
        return Ok(Err("no title by that name in the catalogue".into()));
    }
    let mut near: Vec<Candidate> = Vec::new();
    for (id, name, y, votes) in media.titles_released_between(year - 1, year + 1).await? {
        if normalised(&name) == wanted {
            near.push((id, Some(y), votes));
        }
    }
    if near.is_empty() {
        return Ok(Err("no title by that name in the catalogue".into()));
    }
    Ok(choose(&near, year))
}

/// Record `entries` for `profile`. `now` dates a watch the export did not date.
pub async fn import(
    db: &Db,
    profile: i64,
    entries: &[Entry],
    now: &str,
) -> Result<Summary, DbError> {
    let profiles = ProfileRepository::new(db);
    let media = MediaRepository::new(db);
    let mut s = Summary {
        entries: entries.len(),
        ..Default::default()
    };
    for e in entries {
        let id = match place(db, e).await? {
            Ok(id) => id,
            Err(why) => {
                s.unmatched.push(Unmatched {
                    title: e.title.clone(),
                    year: e.year,
                    why,
                });
                continue;
            }
        };
        s.matched += 1;
        if let Some(stars) = e.rating.and_then(stars) {
            profiles.rate(profile, id, Some(stars)).await?;
            s.ratings += 1;
        }
        if !e.watches.is_empty() {
            let runtime = media
                .by_id(id)
                .await?
                .and_then(|m| m.runtime_minutes)
                .unwrap_or(0)
                * 60;
            for at in &e.watches {
                let at = match at {
                    Some(day) => format!("{day} 00:00:00"),
                    None => now.to_string(),
                };
                if profiles
                    .record_watch_at(profile, id, runtime, true, &at)
                    .await?
                {
                    s.watches += 1;
                }
            }
        }
        if e.watchlist {
            profiles.add_to_watchlist(profile, id, None).await?;
            s.watchlist += 1;
        }
    }
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_written_differently_compare_equal() {
        assert_eq!(
            normalised("Lock, Stock & Two Smoking Barrels"),
            normalised("Lock, Stock and Two Smoking Barrels")
        );
        assert_eq!(
            normalised("Who Framed Roger Rabbit?"),
            "who framed roger rabbit"
        );
        assert_eq!(
            normalised("The X-Files: Fight the Future"),
            normalised("The X Files: Fight the Future")
        );
        assert_eq!(normalised("The Thing"), "thing");
        assert_eq!(
            normalised("The"),
            "the",
            "a title that is only an article keeps it"
        );
        assert_ne!(normalised("Heat"), normalised("Heathers"));
    }

    #[test]
    fn the_year_decides_unless_a_near_year_is_plainly_the_one() {
        // "300": the 2006 film, listed as 2007 by the export; an obscure 2007 "300" exists.
        assert_eq!(
            choose(&[(1, Some(2006), 924_388), (2, Some(2007), 15)], 2007),
            Ok(1)
        );
        // Comparable votes: the exact year stands.
        assert_eq!(
            choose(&[(1, Some(2006), 900), (2, Some(2007), 100)], 2007),
            Ok(2)
        );
        // Two at one year: only a clear favourite is taken.
        assert_eq!(
            choose(&[(1, Some(1972), 100_000), (2, Some(1972), 40)], 1972),
            Ok(1)
        );
        assert!(choose(&[(1, Some(1979), 30_000), (2, Some(1979), 9_000)], 1979).is_err());
        // A year either side, when the exact year has nothing.
        assert_eq!(choose(&[(3, Some(1981), 50)], 1980), Ok(3));
        assert_eq!(
            choose(&[(3, Some(1990), 50)], 1980),
            Err("no title by that name from 1980".into())
        );
        // The same title under two names counts once.
        assert_eq!(
            choose(&[(4, Some(2000), 10), (4, Some(2000), 10)], 2000),
            Ok(4)
        );
    }

    #[test]
    fn ratings_become_stars_with_halves_rounding_up() {
        assert_eq!(stars(10.0), Some(5));
        assert_eq!(stars(7.0), Some(4), "a Letterboxd 3.5");
        assert_eq!(stars(6.0), Some(3));
        assert_eq!(stars(1.0), Some(1), "a half star is still one");
        assert_eq!(stars(0.0), None, "unrated");
    }
}
