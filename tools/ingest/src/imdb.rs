//! The IMDb datasets: what they are, and what we take from them.
//!
//! Published by IMDb for personal and non-commercial use. `imdbws.com` is on
//! `tools/guard/allowlist.txt` as a metadata source (ADR-0010), which is what makes
//! the host name below permissible in shipped code at all.
//!
//! # Scoping is the point, not an optimisation
//!
//! IMDb lists roughly **eleven million** titles. The overwhelming majority are
//! individual episodes of television, adult titles, and video games. Ingesting all
//! of them would breach both of R4's triggers — over two hours, over four gigabytes —
//! and produce a catalogue that is worse to search, not better.
//!
//! `SPEC.md` R4's mitigation is explicit: *"Scope by a vote/popularity threshold
//! rather than ingesting everything."* [`Scope`] is that threshold, and the numbers
//! in it are chosen from a measured pass rather than guessed — see
//! `docs/eval-results.md`.

use crate::job::JobError;

/// Where the datasets come from. One place, so the guard has one thing to check.
pub const HOST: &str = "https://datasets.imdbws.com";

/// One IMDb dataset file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dataset {
    /// Step name, and the local filename stem.
    pub name: &'static str,
    pub filename: &'static str,
    /// The column the file is sorted by — the cursor for resumption.
    pub id_column: &'static str,
    /// Columns the loader reads. Checked up front so a changed schema is named
    /// rather than silently producing empty fields.
    pub required: &'static [&'static str],
}

impl Dataset {
    pub fn url(&self) -> String {
        format!("{HOST}/{}", self.filename)
    }
}

/// Titles: the spine of the catalogue.
pub const TITLE_BASICS: Dataset = Dataset {
    name: "title.basics",
    filename: "title.basics.tsv.gz",
    id_column: "tconst",
    required: &[
        "tconst",
        "titleType",
        "primaryTitle",
        "originalTitle",
        "isAdult",
        "startYear",
        "runtimeMinutes",
        "genres",
    ],
};

/// Ratings and vote counts — what the popularity threshold is computed from.
pub const TITLE_RATINGS: Dataset = Dataset {
    name: "title.ratings",
    filename: "title.ratings.tsv.gz",
    id_column: "tconst",
    required: &["tconst", "averageRating", "numVotes"],
};

/// Alternative titles, including romaji and native-script variants (§6.2).
pub const TITLE_AKAS: Dataset = Dataset {
    name: "title.akas",
    filename: "title.akas.tsv.gz",
    // Sorted by titleId, not tconst — the column names differ between files, which
    // is exactly why `id_column` is per-dataset rather than a constant.
    id_column: "titleId",
    required: &["titleId", "title", "region", "language", "isOriginalTitle"],
};

/// People.
pub const NAME_BASICS: Dataset = Dataset {
    name: "name.basics",
    filename: "name.basics.tsv.gz",
    id_column: "nconst",
    required: &[
        "nconst",
        "primaryName",
        "birthYear",
        "deathYear",
        "primaryProfession",
    ],
};

/// Cast and crew. The largest file by a wide margin.
pub const TITLE_PRINCIPALS: Dataset = Dataset {
    name: "title.principals",
    filename: "title.principals.tsv.gz",
    id_column: "tconst",
    required: &["tconst", "nconst", "category", "ordering", "characters"],
};

/// Episode-to-series mapping with season and episode numbers.
pub const TITLE_EPISODE: Dataset = Dataset {
    name: "title.episode",
    filename: "title.episode.tsv.gz",
    id_column: "tconst",
    required: &["tconst", "parentTconst", "seasonNumber", "episodeNumber"],
};

pub const ALL: &[Dataset] = &[
    TITLE_BASICS,
    TITLE_RATINGS,
    TITLE_AKAS,
    NAME_BASICS,
    TITLE_PRINCIPALS,
    TITLE_EPISODE,
];

/// Which IMDb title types become catalogue entries.
///
/// Excluded deliberately: `videoGame` (not watchable), `tvEpisode` (arrives via
/// `title.episode` attached to its series, not as a standalone catalogue entry),
/// and `tvPilot` (unaired, and duplicated by the series).
pub const KEPT_TYPES: &[&str] = &[
    "movie",
    "tvMovie",
    "tvSeries",
    "tvMiniSeries",
    "tvSpecial",
    "short",
    "video",
];

/// The popularity threshold (R4).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Scope {
    /// Minimum vote count. The single most effective filter: it removes titles
    /// nobody has heard of without removing anything anyone searches for.
    pub min_votes: i64,
    /// Titles with no rating row at all. Most are extremely obscure, but a
    /// just-released film also has none, so this is a separate decision from
    /// `min_votes` rather than a consequence of it.
    pub keep_unrated: bool,
    /// Rescue unrated titles released within this many years, whatever
    /// `keep_unrated` says. A film released last week has no votes yet, and that is
    /// exactly when a discovery-first app must be able to find it.
    pub unrated_recent_years: Option<i64>,
    /// IMDb's adult flag.
    pub keep_adult: bool,
}

impl Scope {
    /// Everything IMDb has. Used to measure the unscoped shape before choosing a
    /// threshold — R4 says measure first.
    pub const EVERYTHING: Scope = Scope {
        min_votes: 0,
        keep_unrated: true,
        unrated_recent_years: None,
        keep_adult: true,
    };

    pub fn keeps(&self, title_type: &str, votes: Option<i64>, is_adult: bool) -> bool {
        self.keeps_with_year(title_type, votes, is_adult, None)
    }

    /// As `keeps`, with the release year available for the recent-unrated rescue.
    pub fn keeps_with_year(
        &self,
        title_type: &str,
        votes: Option<i64>,
        is_adult: bool,
        year: Option<i64>,
    ) -> bool {
        if !KEPT_TYPES.contains(&title_type) {
            return false;
        }
        if is_adult && !self.keep_adult {
            return false;
        }
        match votes {
            Some(votes) => votes >= self.min_votes,
            None => {
                if self.keep_unrated {
                    return true;
                }
                // A missing year cannot be rescued: whether it is recent is
                // unknowable, and guessing yes would readmit most of the long tail.
                match (self.unrated_recent_years, year) {
                    (Some(window), Some(year)) => year >= current_year() - window,
                    _ => false,
                }
            }
        }
    }
}

/// The two-tier catalogue (author's ruling, 2026-09-01: "a combination of C and A").
///
/// # Why two tiers rather than one threshold
///
/// The measurement showed the size risk is not where R4 assumed. `title.basics` at
/// its widest is 471 MB — nowhere near the 4 GB trigger — because excluding
/// `tvEpisode` already removes 77% of IMDb. What is genuinely large is
/// `title.principals` (roughly 90 million cast and crew rows) and `title.akas`, and
/// **both scale with the number of titles kept**.
///
/// So the filter belongs where the cost is, not where the rows are counted:
///
/// - **index** — every kept-type title enters `media_items`. The catalogue is
///   complete and everything is findable, which is what §1's "vastest library"
///   actually promises.
/// - **core** — only titles clearing the popularity bar get the expensive parts:
///   cast, crew, alternative titles, and the Phase 5 embedding documents.
///
/// This is R4's own fallback — *"ship a core index of well-known titles, fetch the
/// long tail live on demand"* — with the halves reassigned by what measurement
/// showed: the title index is cheap enough to be complete, and **enrichment** is
/// what gets rationed.
///
/// An obscure 1913 short is therefore findable by name, with its year and genre, and
/// simply has no cast list until someone asks for it. That is a better failure than
/// not existing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CatalogueScope {
    /// What enters the catalogue at all.
    pub index: Scope,
    /// What additionally gets cast, crew, akas and embeddings.
    pub core: Scope,
}

impl CatalogueScope {
    /// The shipped default, chosen from the measured table in `docs/eval-results.md`.
    pub const DEFAULT: CatalogueScope = CatalogueScope {
        // A: everything of a keepable type. Adult titles are excluded from the index
        // entirely rather than merely from the core — they are not what this app is
        // for, and IMDb flags them reliably.
        index: Scope {
            min_votes: 0,
            keep_unrated: true,
            unrated_recent_years: None,
            keep_adult: false,
        },
        // C: ten votes means *someone* has seen it, plus a rescue for anything from
        // the last two years, which has had no time to accumulate any.
        core: Scope {
            min_votes: 10,
            keep_unrated: false,
            unrated_recent_years: Some(2),
            keep_adult: false,
        },
    };

    pub fn in_index(&self, title_type: &str, votes: Option<i64>, is_adult: bool) -> bool {
        self.index.keeps(title_type, votes, is_adult)
    }

    pub fn in_core(
        &self,
        title_type: &str,
        votes: Option<i64>,
        is_adult: bool,
        year: Option<i64>,
    ) -> bool {
        self.core.keeps_with_year(title_type, votes, is_adult, year)
    }
}

/// The current year, for the recent-unrated rescue.
///
/// Read from the clock rather than baked in, so the two-year window keeps moving. A
/// frozen constant would quietly stop rescuing new releases a year after shipping,
/// and nothing would report it.
fn current_year() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    // Good enough for a year boundary: being a day out at New Year changes nothing
    // about which titles are rescued.
    1970 + seconds / 31_557_600
}

/// A vote-count histogram, for choosing the threshold from evidence.
///
/// Deliberately fixed buckets rather than a percentile sketch: the question is
/// "how many titles survive a threshold of 100?", which is a lookup, and the
/// answer needs to be reproducible across runs.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct VoteHistogram {
    pub buckets: Vec<(i64, u64)>,
    pub unrated: u64,
    pub total: u64,
}

pub const BUCKET_EDGES: &[i64] = &[0, 10, 50, 100, 500, 1_000, 5_000, 10_000, 50_000, 100_000];

impl VoteHistogram {
    pub fn new() -> Self {
        Self {
            buckets: BUCKET_EDGES.iter().map(|edge| (*edge, 0)).collect(),
            unrated: 0,
            total: 0,
        }
    }

    pub fn add(&mut self, votes: Option<i64>) {
        self.total += 1;
        let Some(votes) = votes else {
            self.unrated += 1;
            return;
        };
        // The highest edge the value clears.
        let index = BUCKET_EDGES
            .iter()
            .rposition(|edge| votes >= *edge)
            .unwrap_or(0);
        self.buckets[index].1 += 1;
    }

    /// How many titles have at least `threshold` votes.
    pub fn at_least(&self, threshold: i64) -> u64 {
        self.buckets
            .iter()
            .filter(|(edge, _)| *edge >= threshold)
            .map(|(_, count)| count)
            .sum()
    }
}

/// Parse IMDb's `0`/`1` adult flag.
pub fn is_adult(raw: Option<&str>) -> bool {
    matches!(raw, Some("1"))
}

/// Reject a dataset whose columns have changed, naming what is missing.
pub fn check_columns(reader: &crate::tsv::TsvReader, dataset: &Dataset) -> Result<(), JobError> {
    reader.require_columns(dataset.required)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_are_built_from_one_host() {
        // One place for the guard to check, and one place to change.
        for dataset in ALL {
            assert!(dataset.url().starts_with(HOST));
            assert!(dataset.url().ends_with(".tsv.gz"));
        }
    }

    #[test]
    fn akas_is_keyed_differently_from_the_rest() {
        // The reason id_column is per-dataset. Assuming `tconst` everywhere would
        // make the akas cursor silently wrong, and resumption would skip titles.
        assert_eq!(TITLE_AKAS.id_column, "titleId");
        assert_eq!(TITLE_BASICS.id_column, "tconst");
    }

    #[test]
    fn video_games_and_episodes_are_not_catalogue_entries() {
        let scope = Scope::EVERYTHING;
        assert!(!scope.keeps("videoGame", Some(100_000), false));
        assert!(!scope.keeps("tvEpisode", Some(100_000), false));
        assert!(scope.keeps("movie", Some(1), false));
        assert!(scope.keeps("tvSeries", None, false));
    }

    #[test]
    fn the_vote_threshold_filters() {
        let scope = Scope {
            min_votes: 100,
            keep_unrated: false,
            unrated_recent_years: None,
            keep_adult: false,
        };
        assert!(scope.keeps("movie", Some(100), false));
        assert!(!scope.keeps("movie", Some(99), false));
        assert!(
            !scope.keeps("movie", None, false),
            "unrated is excluded here"
        );
        assert!(
            !scope.keeps("movie", Some(10_000), true),
            "adult is excluded here"
        );
    }

    #[test]
    fn unrated_is_a_separate_decision_from_the_threshold() {
        // A just-released film has no votes yet and is not obscure.
        let keep = Scope {
            min_votes: 1_000,
            keep_unrated: true,
            unrated_recent_years: None,
            keep_adult: false,
        };
        assert!(keep.keeps("movie", None, false));
        let drop = Scope {
            keep_unrated: false,
            ..keep
        };
        assert!(!drop.keeps("movie", None, false));
    }

    #[test]
    fn the_histogram_counts_what_survives_a_threshold() {
        let mut histogram = VoteHistogram::new();
        for votes in [5, 5, 60, 200, 200, 200, 20_000] {
            histogram.add(Some(votes));
        }
        histogram.add(None);

        assert_eq!(histogram.total, 8);
        assert_eq!(histogram.unrated, 1);
        assert_eq!(histogram.at_least(0), 7, "every rated title");
        assert_eq!(histogram.at_least(100), 4, "200 x3 and 20,000");
        assert_eq!(histogram.at_least(10_000), 1);
        assert_eq!(histogram.at_least(1_000_000), 0);
    }

    #[test]
    fn the_index_keeps_everything_and_the_core_does_not() {
        // The author's ruling: A for the index, C for enrichment.
        let scope = CatalogueScope::DEFAULT;

        // An obscure 1913 short with two votes: in the catalogue, not enriched.
        assert!(scope.in_index("short", Some(2), false));
        assert!(!scope.in_core("short", Some(2), false, Some(1913)));

        // A well-known film: both.
        assert!(scope.in_index("movie", Some(400_000), false));
        assert!(scope.in_core("movie", Some(400_000), false, Some(1954)));

        // Never seen by anyone, no year: indexed, not enriched.
        assert!(scope.in_index("video", None, false));
        assert!(!scope.in_core("video", None, false, None));
    }

    #[test]
    fn a_new_release_with_no_votes_is_still_enriched() {
        // The reason the plain ">= 10 votes" option was rejected: a film released
        // last week has no votes, and that is exactly when it must be findable AND
        // have its cast.
        let scope = CatalogueScope::DEFAULT;
        let this_year = current_year();

        assert!(scope.in_core("movie", None, false, Some(this_year)));
        assert!(scope.in_core("movie", None, false, Some(this_year - 1)));
        assert!(
            !scope.in_core("movie", None, false, Some(this_year - 20)),
            "the rescue is for new releases, not for the whole unrated long tail"
        );
    }

    #[test]
    fn adult_titles_are_excluded_from_the_index_entirely() {
        let scope = CatalogueScope::DEFAULT;
        assert!(!scope.in_index("movie", Some(100_000), true));
        assert!(!scope.in_core("movie", Some(100_000), true, Some(2020)));
    }

    #[test]
    fn the_core_is_always_a_subset_of_the_index() {
        // If this ever fails, something would be enriched that is not in the
        // catalogue at all — a foreign key waiting to happen.
        let scope = CatalogueScope::DEFAULT;
        let year = current_year();
        for title_type in ["movie", "short", "tvSeries", "videoGame"] {
            for votes in [None, Some(0), Some(9), Some(10), Some(1_000_000)] {
                for adult in [false, true] {
                    for y in [None, Some(1913), Some(year)] {
                        if scope.in_core(title_type, votes, adult, y) {
                            assert!(
                                scope.in_index(title_type, votes, adult),
                                "core but not indexed: {title_type} {votes:?} adult={adult} year={y:?}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn the_recent_window_moves_with_the_clock() {
        // A baked-in year would stop rescuing new releases a year after shipping,
        // silently.
        assert!(current_year() >= 2026, "the clock is readable");
        assert!(current_year() < 2100, "and not obviously wrong");
    }

    #[test]
    fn adult_flag_is_only_the_literal_one() {
        assert!(is_adult(Some("1")));
        assert!(!is_adult(Some("0")));
        assert!(!is_adult(None));
        assert!(!is_adult(Some("\\N")));
    }
}
