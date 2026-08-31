//! The canonical types (`SPEC.md` §6.2).
//!
//! One `MediaItem` represents a film, an episode, a series, an anime season and —
//! later, without a migration — a manga chapter. That genericity is the point:
//! Phases 24 and 25 add reading and comics, and they are only cheap if nothing
//! here assumes film.

use serde::{Deserialize, Serialize};

/// What an item *is*.
///
/// All eight values exist from the first migration, including the two nothing
/// uses until Phase 24. Adding a variant to a SQLite `CHECK` constraint later
/// means rebuilding the table, and the entire argument for a generic schema is
/// that those phases should cost nothing extra.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum MediaKind {
    Film,
    Episode,
    Series,
    AnimeFilm,
    AnimeSeries,
    LiveChannel,
    MangaChapter,
    ComicIssue,
}

impl MediaKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Film => "film",
            Self::Episode => "episode",
            Self::Series => "series",
            Self::AnimeFilm => "anime_film",
            Self::AnimeSeries => "anime_series",
            Self::LiveChannel => "live_channel",
            Self::MangaChapter => "manga_chapter",
            Self::ComicIssue => "comic_issue",
        }
    }

    /// Every variant, so a test can assert the schema and the enum agree.
    pub const ALL: [MediaKind; 8] = [
        Self::Film,
        Self::Episode,
        Self::Series,
        Self::AnimeFilm,
        Self::AnimeSeries,
        Self::LiveChannel,
        Self::MangaChapter,
        Self::ComicIssue,
    ];

    /// Whether this kind is watched as a single sitting, as opposed to being a
    /// container (a series) or read (manga). Used by Phase 9's source selection.
    pub fn is_playable(self) -> bool {
        matches!(
            self,
            Self::Film | Self::Episode | Self::AnimeFilm | Self::LiveChannel
        )
    }
}

/// Where an external identifier came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum IdSource {
    Imdb,
    Tmdb,
    Tvdb,
    Anilist,
    Mal,
    Movielens,
}

impl IdSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Imdb => "imdb",
            Self::Tmdb => "tmdb",
            Self::Tvdb => "tvdb",
            Self::Anilist => "anilist",
            Self::Mal => "mal",
            Self::Movielens => "movielens",
        }
    }

    pub const ALL: [IdSource; 6] = [
        Self::Imdb,
        Self::Tmdb,
        Self::Tvdb,
        Self::Anilist,
        Self::Mal,
        Self::Movielens,
    ];
}

/// Which of an item's several names a row holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum TitleVariant {
    /// The one the UI shows.
    Primary,
    /// As released in its own country, in its own script.
    Original,
    /// Latin transliteration — the form most users type for anime.
    Romaji,
    /// Native script.
    Native,
    English,
    Alternative,
}

impl TitleVariant {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::Original => "original",
            Self::Romaji => "romaji",
            Self::Native => "native",
            Self::English => "english",
            Self::Alternative => "alternative",
        }
    }

    pub const ALL: [TitleVariant; 6] = [
        Self::Primary,
        Self::Original,
        Self::Romaji,
        Self::Native,
        Self::English,
        Self::Alternative,
    ];
}

/// A row of `media_items`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MediaItem {
    pub id: i64,
    pub kind: MediaKind,
    pub primary_title: String,
    pub sort_title: Option<String>,
    pub release_year: Option<i64>,
    pub release_date: Option<String>,
    pub runtime_minutes: Option<i64>,
    pub original_language: Option<String>,
    pub countries: Option<String>,
    pub synopsis: Option<String>,
    /// 0-100, so ranking never compares floats.
    pub rating: Option<i64>,
    pub rating_votes: Option<i64>,
    pub is_adult: bool,
}

/// What is needed to create one. Separate from `MediaItem` because an id is
/// assigned by the database, and an insert type carrying a meaningless `id: 0` is
/// the kind of small lie that turns into a bug.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct NewMediaItem {
    pub kind: Option<MediaKind>,
    pub primary_title: String,
    pub sort_title: Option<String>,
    pub release_year: Option<i64>,
    pub release_date: Option<String>,
    pub runtime_minutes: Option<i64>,
    pub original_language: Option<String>,
    pub countries: Option<String>,
    pub synopsis: Option<String>,
    pub rating: Option<i64>,
    pub rating_votes: Option<i64>,
    pub is_adult: bool,
}

impl NewMediaItem {
    /// A film with just a title and year — the overwhelmingly common case in
    /// ingestion, and in tests.
    pub fn film(title: impl Into<String>, year: i64) -> Self {
        Self {
            kind: Some(MediaKind::Film),
            primary_title: title.into(),
            release_year: Some(year),
            ..Default::default()
        }
    }
}

/// A name an item is known by.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Title {
    pub id: i64,
    pub media_item_id: i64,
    pub title: String,
    pub variant: TitleVariant,
    pub language: Option<String>,
    pub region: Option<String>,
}

/// One source's numbering of one episode — the reconciliation record.
///
/// See `migrations/0003_series.up.sql` for why this cannot be computed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EpisodeNumbering {
    pub episode_id: i64,
    pub source: IdSource,
    pub season_number: Option<i64>,
    pub episode_number: Option<i64>,
    pub absolute_number: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_strings_are_unique_and_snake_case() {
        let mut seen = std::collections::HashSet::new();
        for kind in MediaKind::ALL {
            let s = kind.as_str();
            assert!(seen.insert(s), "duplicate discriminator: {s}");
            assert!(
                s.chars().all(|c| c.is_ascii_lowercase() || c == '_'),
                "not snake_case: {s}"
            );
        }
        assert_eq!(seen.len(), 8, "SPEC.md §6.2 names eight kinds");
    }

    #[test]
    fn manga_and_comics_exist_before_anything_needs_them() {
        // The point of the generic schema: Phases 24-25 must require no migration.
        assert!(MediaKind::ALL.contains(&MediaKind::MangaChapter));
        assert!(MediaKind::ALL.contains(&MediaKind::ComicIssue));
    }

    #[test]
    fn containers_and_reading_are_not_playable() {
        assert!(MediaKind::Film.is_playable());
        assert!(MediaKind::Episode.is_playable());
        assert!(MediaKind::LiveChannel.is_playable());
        assert!(!MediaKind::Series.is_playable(), "a series is a container");
        assert!(!MediaKind::MangaChapter.is_playable());
        assert!(!MediaKind::ComicIssue.is_playable());
    }

    #[test]
    fn anime_title_variants_are_all_present() {
        // SPEC.md §6.2 names romaji, native and english specifically.
        for wanted in [
            TitleVariant::Romaji,
            TitleVariant::Native,
            TitleVariant::English,
        ] {
            assert!(TitleVariant::ALL.contains(&wanted));
        }
    }
}
