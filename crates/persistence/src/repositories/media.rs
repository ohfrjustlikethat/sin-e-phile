//! Reads and writes for media items, titles, and external ids.
//!
//! Every SQL statement in the application lives in a repository like this one.
//! Callers get typed methods; nothing above this layer knows the schema exists.

use std::collections::HashMap;

use sqlx::Row;

use crate::db::{Db, DbError};

/// Where [`MediaRepository::keep_rising`] keeps its list (settings).
pub const RISING_KEY: &str = "catalogue.rising";

/// Genres a kids profile never shows, whatever else a title is filed under.
const KIDS_EXCLUDED: &str = "'Adult', 'Horror', 'Thriller', 'Crime', 'War', 'Film-Noir'";
use crate::model::{IdSource, MediaItem, MediaKind, NewMediaItem, Title, TitleVariant};

/// One credit as the detail page lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Credit {
    /// For their filmography (Phase 11's pause overlay).
    pub person_id: i64,
    pub name: String,
    /// `director`, `creator`, `writer`, `actor`, …
    pub role: String,
    pub character: Option<String>,
}

/// One line of a filmography.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Appearance {
    pub media_item_id: i64,
    pub title: String,
    pub year: Option<i64>,
    pub kind: String,
    pub role: String,
    pub character: Option<String>,
}

/// What the recommender weighs about a title, read for many titles at once (Phase 17).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Facts {
    pub id: i64,
    pub title: String,
    pub year: Option<i64>,
    pub runtime_minutes: Option<i64>,
    pub votes: Option<i64>,
    pub genres: Vec<String>,
    /// (person id, name), billing order.
    pub directors: Vec<(i64, String)>,
    /// IMDb's rating times ten, 0 to 100 (Phase 18: the acclaimed and the deep cuts).
    pub rating: Option<i64>,
    /// The countries it comes from and the movements it belongs to, as Wikidata names them
    /// (Phase 18, migration 0033); empty until asked, or when Wikidata does not say.
    pub countries: Vec<String>,
    pub movements: Vec<String>,
}

pub struct MediaRepository<'a> {
    db: &'a Db,
}

impl<'a> MediaRepository<'a> {
    pub fn new(db: &'a Db) -> Self {
        Self { db }
    }

    /// Insert an item and its primary title, returning the new id.
    ///
    /// One transaction, because an item without its title row is a half-created
    /// record that every later join has to defend against. Failing atomically is
    /// simpler than tolerating the intermediate state everywhere else.
    pub async fn insert(&self, item: &NewMediaItem) -> Result<i64, DbError> {
        let kind = item.kind.unwrap_or(MediaKind::Film);
        let mut tx = self.db.pool().begin().await?;

        let id: i64 = sqlx::query_scalar(
            "INSERT INTO media_items (
                 kind, primary_title, sort_title, release_year, release_date,
                 runtime_minutes, original_language, countries, synopsis,
                 rating, rating_votes, is_adult
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             RETURNING id",
        )
        .bind(kind.as_str())
        .bind(&item.primary_title)
        .bind(&item.sort_title)
        .bind(item.release_year)
        .bind(&item.release_date)
        .bind(item.runtime_minutes)
        .bind(&item.original_language)
        .bind(&item.countries)
        .bind(&item.synopsis)
        .bind(item.rating)
        .bind(item.rating_votes)
        .bind(i64::from(item.is_adult))
        .fetch_one(&mut *tx)
        .await?;

        sqlx::query(
            "INSERT INTO titles (media_item_id, title, variant, language)
             VALUES (?, ?, 'primary', ?)",
        )
        .bind(id)
        .bind(&item.primary_title)
        .bind(&item.original_language)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(id)
    }

    pub async fn by_id(&self, id: i64) -> Result<Option<MediaItem>, DbError> {
        let row = sqlx::query(
            "SELECT id, kind, primary_title, sort_title, release_year, release_date,
                    runtime_minutes, original_language, countries, synopsis,
                    rating, rating_votes, is_adult
             FROM media_items WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(self.db.pool())
        .await?;

        row.map(map_media_item).transpose()
    }

    /// Resolve an external id to an internal item. The lookup ingestion performs
    /// constantly, and the reason `idx_external_ids_lookup` exists.
    pub async fn by_external_id(
        &self,
        source: IdSource,
        external_id: &str,
    ) -> Result<Option<MediaItem>, DbError> {
        let row = sqlx::query(
            "SELECT m.id, m.kind, m.primary_title, m.sort_title, m.release_year,
                    m.release_date, m.runtime_minutes, m.original_language,
                    m.countries, m.synopsis, m.rating, m.rating_votes, m.is_adult
             FROM media_items m
             JOIN external_ids e ON e.media_item_id = m.id
             WHERE e.source = ? AND e.external_id = ?",
        )
        .bind(source.as_str())
        .bind(external_id)
        .fetch_optional(self.db.pool())
        .await?;

        row.map(map_media_item).transpose()
    }

    /// Exact title match across every variant.
    ///
    /// Phase 5's exit criterion is a 100% exact-title top-1 rate, and this is the
    /// short-circuit that guarantees it before any ranking runs. `COLLATE NOCASE`
    /// rather than `LOWER()` on both sides so the index is still usable.
    pub async fn by_exact_title(&self, title: &str) -> Result<Vec<MediaItem>, DbError> {
        let rows = sqlx::query(
            "SELECT DISTINCT m.id, m.kind, m.primary_title, m.sort_title, m.release_year,
                    m.release_date, m.runtime_minutes, m.original_language,
                    m.countries, m.synopsis, m.rating, m.rating_votes, m.is_adult
             FROM media_items m
             JOIN titles t ON t.media_item_id = m.id
             WHERE t.title = ? COLLATE NOCASE
             ORDER BY m.rating_votes DESC NULLS LAST",
        )
        .bind(title)
        .fetch_all(self.db.pool())
        .await?;

        rows.into_iter().map(map_media_item).collect()
    }

    pub async fn add_external_id(
        &self,
        media_item_id: i64,
        source: IdSource,
        external_id: &str,
        confidence: f64,
    ) -> Result<(), DbError> {
        sqlx::query(
            "INSERT INTO external_ids (media_item_id, source, external_id, confidence)
             VALUES (?, ?, ?, ?)
             ON CONFLICT (media_item_id, source) DO UPDATE
                 SET external_id = excluded.external_id,
                     confidence  = excluded.confidence",
        )
        .bind(media_item_id)
        .bind(source.as_str())
        .bind(external_id)
        .bind(confidence)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    pub async fn add_title(
        &self,
        media_item_id: i64,
        title: &str,
        variant: TitleVariant,
        language: Option<&str>,
    ) -> Result<(), DbError> {
        sqlx::query(
            "INSERT INTO titles (media_item_id, title, variant, language)
             VALUES (?, ?, ?, ?)
             ON CONFLICT DO NOTHING",
        )
        .bind(media_item_id)
        .bind(title)
        .bind(variant.as_str())
        .bind(language)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    pub async fn titles_for(&self, media_item_id: i64) -> Result<Vec<Title>, DbError> {
        let rows = sqlx::query(
            "SELECT id, media_item_id, title, variant, language, region
             FROM titles WHERE media_item_id = ? ORDER BY variant",
        )
        .bind(media_item_id)
        .fetch_all(self.db.pool())
        .await?;

        rows.into_iter()
            .map(|row| {
                Ok(Title {
                    id: row.try_get("id")?,
                    media_item_id: row.try_get("media_item_id")?,
                    title: row.try_get("title")?,
                    variant: parse_variant(row.try_get::<String, _>("variant")?.as_str()),
                    language: row.try_get("language")?,
                    region: row.try_get("region")?,
                })
            })
            .collect()
    }

    /// Each of `items`' id at `source`, those that have one (ADR-0054: the titles a live
    /// source is asked about, by its own ids).
    pub async fn external_ids_of(
        &self,
        source: IdSource,
        items: &[i64],
    ) -> Result<HashMap<i64, String>, DbError> {
        let mut out = HashMap::new();
        for chunk in items.chunks(500) {
            let marks = vec!["?"; chunk.len()].join(",");
            let sql = format!(
                "SELECT media_item_id, external_id FROM external_ids
                 WHERE source = ? AND media_item_id IN ({marks})"
            );
            let mut query = sqlx::query_as::<_, (i64, String)>(&sql).bind(source.as_str());
            for id in chunk {
                query = query.bind(id);
            }
            out.extend(query.fetch_all(self.db.pool()).await?);
        }
        Ok(out)
    }

    /// The catalogue item behind each of `ids` at `source`, those the catalogue holds: a
    /// live source answers in its own ids.
    pub async fn items_by_external(
        &self,
        source: IdSource,
        ids: &[String],
    ) -> Result<HashMap<String, i64>, DbError> {
        let mut out = HashMap::new();
        for chunk in ids.chunks(500) {
            let marks = vec!["?"; chunk.len()].join(",");
            let sql = format!(
                "SELECT external_id, media_item_id FROM external_ids
                 WHERE source = ? AND external_id IN ({marks})"
            );
            let mut query = sqlx::query_as::<_, (String, i64)>(&sql).bind(source.as_str());
            for id in chunk {
                query = query.bind(id);
            }
            out.extend(query.fetch_all(self.db.pool()).await?);
        }
        Ok(out)
    }

    /// This item's id at one external source (`imdb`, `tmdb`, `anilist`, …).
    pub async fn external_id(
        &self,
        media_item_id: i64,
        source: &str,
    ) -> Result<Option<String>, DbError> {
        Ok(sqlx::query_scalar(
            "SELECT external_id FROM external_ids WHERE media_item_id = ? AND source = ?",
        )
        .bind(media_item_id)
        .bind(source)
        .fetch_optional(self.db.pool())
        .await?)
    }

    /// Who made it and who is in it, for the detail page: directors and creators first,
    /// then the cast in billing order (unbilled last).
    pub async fn credits(&self, media_item_id: i64, limit: i64) -> Result<Vec<Credit>, DbError> {
        let rows: Vec<(i64, String, String, String)> = sqlx::query_as(
            "SELECT p.id, p.name, c.role, c.character
               FROM credits c JOIN people p ON p.id = c.person_id
              WHERE c.media_item_id = ?
              ORDER BY CASE c.role WHEN 'director' THEN 0 WHEN 'creator' THEN 1
                                   WHEN 'writer' THEN 2 ELSE 3 END,
                       c.billing IS NULL, c.billing, p.name
              LIMIT ?",
        )
        .bind(media_item_id)
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?;
        Ok(rows
            .into_iter()
            .map(|(person_id, name, role, character)| Credit {
                person_id,
                name,
                role,
                character: (!character.is_empty()).then_some(character),
            })
            .collect())
    }

    /// What a person has been in or made, newest first: the filmography a cast member
    /// opens to from the pause overlay. Films and series only, not single episodes.
    pub async fn filmography(
        &self,
        person_id: i64,
        limit: i64,
    ) -> Result<Vec<Appearance>, DbError> {
        let rows: Vec<(i64, String, Option<i64>, String, String, String)> = sqlx::query_as(
            "SELECT m.id, m.primary_title, m.release_year, m.kind, c.role, c.character
               FROM credits c JOIN media_items m ON m.id = c.media_item_id
              WHERE c.person_id = ? AND m.kind != 'episode'
              ORDER BY m.release_year IS NULL, m.release_year DESC, m.primary_title
              LIMIT ?",
        )
        .bind(person_id)
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?;
        Ok(rows
            .into_iter()
            .map(
                |(media_item_id, title, year, kind, role, character)| Appearance {
                    media_item_id,
                    title,
                    year,
                    kind,
                    role,
                    character: (!character.is_empty()).then_some(character),
                },
            )
            .collect())
    }

    /// The other work of whoever directed or created `loved`, newest first, at most
    /// `per_person` each: (loved title, person, their name, other title). How a title nobody
    /// has rated yet reaches a profile that loved its maker's earlier ones (ADR-0054 step 5).
    pub async fn made_alike(
        &self,
        loved: &[i64],
        per_person: i64,
    ) -> Result<Vec<(i64, i64, String, i64)>, DbError> {
        let mut out = Vec::new();
        for chunk in loved.chunks(500) {
            let marks = vec!["?"; chunk.len()].join(",");
            let sql = format!(
                "WITH makers AS (
                     SELECT DISTINCT media_item_id AS loved, person_id FROM credits
                      WHERE role IN ('director', 'creator') AND media_item_id IN ({marks})),
                 works AS (
                     SELECT k.person_id, c.media_item_id AS title,
                            ROW_NUMBER() OVER (PARTITION BY k.person_id
                                ORDER BY m.release_year IS NULL, m.release_year DESC, m.id DESC)
                                AS newest
                       FROM (SELECT DISTINCT person_id FROM makers) k
                       JOIN credits c ON c.person_id = k.person_id
                                     AND c.role IN ('director', 'creator')
                       JOIN media_items m ON m.id = c.media_item_id AND m.kind <> 'episode')
                 SELECT mk.loved, mk.person_id, p.name, w.title
                   FROM makers mk
                   JOIN works w ON w.person_id = mk.person_id AND w.newest <= ?
                   JOIN people p ON p.id = mk.person_id
                  WHERE w.title <> mk.loved
                  ORDER BY mk.loved, mk.person_id, w.newest"
            );
            let mut query = sqlx::query_as::<_, (i64, i64, String, i64)>(&sql);
            for id in chunk {
                query = query.bind(id);
            }
            out.extend(query.bind(per_person).fetch_all(self.db.pool()).await?);
        }
        Ok(out)
    }

    /// What gained the most votes in its last change, among titles whose count changed in
    /// the last `days`, films and series, not adult, with at least `min_votes`: (title,
    /// votes gained). Rising now (ADR-0054 step 6, migration 0032).
    pub async fn rising(
        &self,
        days: i64,
        min_votes: i64,
        limit: i64,
    ) -> Result<Vec<(i64, i64)>, DbError> {
        Ok(sqlx::query_as(
            "SELECT id, rating_votes - previous_votes AS gained
               FROM media_items INDEXED BY idx_media_items_votes_changed
              WHERE previous_votes IS NOT NULL
                AND votes_changed_at >= datetime('now', '-' || ?1 || ' days')
                AND rating_votes >= ?2 AND rating_votes > previous_votes AND is_adult = 0
                AND kind IN ('film', 'anime_film', 'series', 'anime_series')
              ORDER BY gained DESC, id LIMIT ?3",
        )
        .bind(days)
        .bind(min_votes)
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?)
    }

    /// Work out what is rising after a ratings refresh and keep it, best first, so Home
    /// reads a list rather than ranking every changed title (a refresh changes hundreds of
    /// thousands). Returns how many were kept.
    pub async fn keep_rising(
        &self,
        days: i64,
        min_votes: i64,
        kept: i64,
    ) -> Result<usize, DbError> {
        let rising = self.rising(days, min_votes, kept).await?;
        let value = rising
            .iter()
            .map(|(id, gained)| format!("{id}:{gained}"))
            .collect::<Vec<_>>()
            .join(",");
        sqlx::query(
            "INSERT INTO settings (key, value, updated_at) VALUES (?, ?, datetime('now'))
             ON CONFLICT (key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')",
        )
        .bind(RISING_KEY)
        .bind(value)
        .execute(self.db.pool())
        .await?;
        Ok(rising.len())
    }

    /// What [`MediaRepository::keep_rising`] kept: (title, votes gained), best first.
    pub async fn kept_rising(&self) -> Result<Vec<(i64, i64)>, DbError> {
        let value: Option<String> = sqlx::query_scalar("SELECT value FROM settings WHERE key = ?")
            .bind(RISING_KEY)
            .fetch_optional(self.db.pool())
            .await?;
        Ok(value
            .unwrap_or_default()
            .split(',')
            .filter_map(|pair| {
                let (id, gained) = pair.split_once(':')?;
                Some((id.parse().ok()?, gained.parse().ok()?))
            })
            .collect())
    }

    /// A title's genres, by name (Phase 15's taste axes).
    /// [`Facts`] for each of `ids` the catalogue holds, in no particular order: a rail's
    /// thousand candidates in four queries per 500 rather than four per title.
    pub async fn facts(&self, ids: &[i64]) -> Result<Vec<Facts>, DbError> {
        let mut out: HashMap<i64, Facts> = HashMap::with_capacity(ids.len());
        for chunk in ids.chunks(500) {
            let marks = vec!["?"; chunk.len()].join(",");
            let sql = format!(
                "SELECT id, primary_title, release_year, runtime_minutes, rating_votes, rating
                 FROM media_items WHERE id IN ({marks})"
            );
            type Row = (
                i64,
                String,
                Option<i64>,
                Option<i64>,
                Option<i64>,
                Option<i64>,
            );
            let mut q = sqlx::query_as::<_, Row>(&sql);
            for id in chunk {
                q = q.bind(id);
            }
            for (id, title, year, runtime_minutes, votes, rating) in
                q.fetch_all(self.db.pool()).await?
            {
                out.insert(
                    id,
                    Facts {
                        id,
                        title,
                        year,
                        runtime_minutes,
                        votes,
                        rating,
                        ..Default::default()
                    },
                );
            }
            let sql = format!(
                "SELECT media_item_id, axis, name FROM title_regions
                 WHERE media_item_id IN ({marks}) ORDER BY axis, name"
            );
            let mut q = sqlx::query_as::<_, (i64, String, String)>(&sql);
            for id in chunk {
                q = q.bind(id);
            }
            for (id, axis, name) in q.fetch_all(self.db.pool()).await? {
                if let Some(f) = out.get_mut(&id) {
                    if axis == "country" {
                        f.countries.push(name);
                    } else {
                        f.movements.push(name);
                    }
                }
            }
            let sql = format!(
                "SELECT mg.media_item_id, g.name FROM media_genres mg JOIN genres g ON g.id = mg.genre_id
                 WHERE mg.media_item_id IN ({marks}) ORDER BY g.name"
            );
            let mut q = sqlx::query_as::<_, (i64, String)>(&sql);
            for id in chunk {
                q = q.bind(id);
            }
            for (id, genre) in q.fetch_all(self.db.pool()).await? {
                if let Some(f) = out.get_mut(&id) {
                    f.genres.push(genre);
                }
            }
            let sql = format!(
                "SELECT c.media_item_id, p.id, p.name FROM credits c JOIN people p ON p.id = c.person_id
                 WHERE c.role = 'director' AND c.media_item_id IN ({marks})
                 ORDER BY c.billing IS NULL, c.billing, p.id"
            );
            let mut q = sqlx::query_as::<_, (i64, i64, String)>(&sql);
            for id in chunk {
                q = q.bind(id);
            }
            for (id, person, name) in q.fetch_all(self.db.pool()).await? {
                if let Some(f) = out.get_mut(&id) {
                    f.directors.push((person, name));
                }
            }
        }
        Ok(out.into_values().collect())
    }

    pub async fn genres(&self, media_item_id: i64) -> Result<Vec<String>, DbError> {
        Ok(sqlx::query_scalar(
            "SELECT g.name FROM media_genres mg JOIN genres g ON g.id = mg.genre_id
             WHERE mg.media_item_id = ? ORDER BY g.name",
        )
        .bind(media_item_id)
        .fetch_all(self.db.pool())
        .await?)
    }

    /// Which of `ids` a kids profile may see (Phase 14). Fails closed: only titles
    /// positively filed as for children (MovieLens "Children", from `ingest movielens`)
    /// or family (IMDb "Family"), not adult, and in none of the genres no children's film
    /// carries. Nothing else is guessed at: an unfiled title is not shown.
    pub async fn suitable_for_children(
        &self,
        ids: &[i64],
    ) -> Result<std::collections::HashSet<i64>, DbError> {
        let mut out = std::collections::HashSet::new();
        for chunk in ids.chunks(500) {
            let sql = format!(
                "SELECT m.id FROM media_items m
                 WHERE m.id IN ({}) AND m.is_adult = 0
                   AND EXISTS (SELECT 1 FROM media_genres mg JOIN genres g ON g.id = mg.genre_id
                               WHERE mg.media_item_id = m.id AND g.name IN ('Children', 'Family'))
                   AND NOT EXISTS (SELECT 1 FROM media_genres mg JOIN genres g ON g.id = mg.genre_id
                               WHERE mg.media_item_id = m.id
                                 AND g.name IN ({KIDS_EXCLUDED}))",
                vec!["?"; chunk.len()].join(", ")
            );
            let mut q = sqlx::query_scalar::<_, i64>(&sql);
            for id in chunk {
                q = q.bind(id);
            }
            out.extend(q.fetch_all(self.db.pool()).await?);
        }
        Ok(out)
    }

    /// Every name of every film and series released from `from` to `to`, with its year and
    /// votes: (id, name, year, votes). For matching a name written differently from the
    /// catalogue's ("&" for "and"), where an exact lookup found nothing (Phase 14 imports).
    pub async fn titles_released_between(
        &self,
        from: i64,
        to: i64,
    ) -> Result<Vec<(i64, String, i64, i64)>, DbError> {
        Ok(sqlx::query_as(
            "SELECT m.id, t.title, m.release_year, COALESCE(m.rating_votes, 0)
             FROM media_items m JOIN titles t ON t.media_item_id = m.id
             WHERE m.release_year BETWEEN ? AND ?
               AND m.kind IN ('film', 'series', 'anime_film', 'anime_series')",
        )
        .bind(from)
        .bind(to)
        .fetch_all(self.db.pool())
        .await?)
    }

    /// The most-voted films, most first, not adult: the Home rail every install has, key or
    /// none, sources or none (Phase 14).
    pub async fn most_voted_films(&self, limit: i64) -> Result<Vec<i64>, DbError> {
        Ok(sqlx::query_scalar(
            "SELECT id FROM media_items
             WHERE kind IN ('film', 'anime_film') AND is_adult = 0 AND rating_votes IS NOT NULL
             ORDER BY rating_votes DESC, id LIMIT ?",
        )
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?)
    }

    /// New releases (Phase 16): films and series of `from_year` or later with at least
    /// `min_votes` (so they have come out and are being watched), with their votes, most
    /// voted first. The catalogue has years, not dates, so "new" is "this year".
    pub async fn new_releases(
        &self,
        from_year: i64,
        min_votes: i64,
        limit: i64,
    ) -> Result<Vec<(i64, i64)>, DbError> {
        Ok(sqlx::query_as(
            // By year first (migration 0027): only this year's titles are read. Named,
            // because the planner otherwise walks the votes index to avoid a sort and
            // reads most of the catalogue (92 ms against 11 ms on the real one).
            "SELECT id, rating_votes FROM media_items INDEXED BY idx_media_items_year_votes
             WHERE release_year >= ?1 AND rating_votes >= ?2 AND is_adult = 0
               AND +kind IN ('film', 'anime_film', 'series', 'anime_series')
             ORDER BY rating_votes DESC, id LIMIT ?3",
        )
        .bind(from_year)
        .bind(min_votes)
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?)
    }

    /// The most-voted titles with their votes, films and series, not adult: the pool the
    /// taste model's popularity prior draws from (Phase 15).
    pub async fn most_voted_with_votes(&self, limit: i64) -> Result<Vec<(i64, i64)>, DbError> {
        Ok(sqlx::query_as(
            "SELECT id, rating_votes FROM media_items
             WHERE +kind IN ('film', 'anime_film', 'series', 'anime_series')
               AND is_adult = 0 AND rating_votes IS NOT NULL
             ORDER BY rating_votes DESC, id LIMIT ?",
        )
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?)
    }

    /// Films rated at least `min_rating` (0 to 100) by at least `min_votes`, not adult, most
    /// voted first: the acclaimed, whom the discovery engine's blind spots and canon are drawn
    /// from (Phase 18). About 3,600 films at 7.5 and 5,000 votes.
    pub async fn acclaimed(
        &self,
        min_rating: i64,
        min_votes: i64,
        limit: i64,
    ) -> Result<Vec<i64>, DbError> {
        Ok(sqlx::query_scalar(
            "SELECT id FROM media_items
             WHERE rating_votes >= ? AND rating >= ? AND is_adult = 0
               AND +kind IN ('film', 'anime_film')
             ORDER BY rating_votes DESC, id LIMIT ?",
        )
        .bind(min_votes)
        .bind(min_rating)
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?)
    }

    /// The most-voted films released from `from` to `to`: onboarding's title cards are
    /// drawn decade by decade, so the first round spreads across eras (Phase 14).
    pub async fn most_voted_films_between(
        &self,
        from: i64,
        to: i64,
        limit: i64,
    ) -> Result<Vec<i64>, DbError> {
        Ok(sqlx::query_scalar(
            // The `+` keeps kind and year out of the index choice, so SQLite walks the
            // votes index (0025) in order and stops at `limit`: 0.067 s for eleven decades
            // instead of 2.3 s sorting each one, the same answers.
            "SELECT id FROM media_items
             WHERE +kind IN ('film', 'anime_film') AND is_adult = 0 AND rating_votes IS NOT NULL
               AND +release_year BETWEEN ? AND ?
             ORDER BY rating_votes DESC, id LIMIT ?",
        )
        .bind(from)
        .bind(to)
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?)
    }

    /// The most-voted films (or, with `series`, shows), not adult, optionally in one genre:
    /// the catalogue rails under Films and TV Shows (Phase 16), so neither tab is ever empty.
    /// Walks the votes index and checks the genre per row (the `+` keeps kind out of the
    /// index choice): 0–90 ms for the genres the rails use, on 3.1 million titles.
    pub async fn most_voted_of(
        &self,
        series: bool,
        genre: Option<&str>,
        limit: i64,
    ) -> Result<Vec<i64>, DbError> {
        let kinds = if series {
            "'series', 'anime_series'"
        } else {
            "'film', 'anime_film'"
        };
        let sql = format!(
            "SELECT m.id FROM media_items m
             WHERE +m.kind IN ({kinds}) AND m.is_adult = 0 AND m.rating_votes IS NOT NULL
               AND (?1 IS NULL OR EXISTS (
                   SELECT 1 FROM media_genres mg JOIN genres g ON g.id = mg.genre_id
                    WHERE mg.media_item_id = m.id AND g.name = ?1))
             ORDER BY m.rating_votes DESC, m.id LIMIT ?2"
        );
        Ok(sqlx::query_scalar(&sql)
            .bind(genre)
            .bind(limit)
            .fetch_all(self.db.pool())
            .await?)
    }

    /// Films (or shows) of `from_year` or later with at least `min_votes`, most voted first:
    /// "New this year" under Films and TV Shows. By the year index, as `new_releases`.
    pub async fn new_of(
        &self,
        series: bool,
        from_year: i64,
        min_votes: i64,
        limit: i64,
    ) -> Result<Vec<i64>, DbError> {
        let kinds = if series {
            "'series', 'anime_series'"
        } else {
            "'film', 'anime_film'"
        };
        let sql = format!(
            "SELECT id FROM media_items INDEXED BY idx_media_items_year_votes
             WHERE release_year >= ?1 AND rating_votes >= ?2 AND is_adult = 0
               AND +kind IN ({kinds})
             ORDER BY rating_votes DESC, id LIMIT ?3"
        );
        Ok(sqlx::query_scalar(&sql)
            .bind(from_year)
            .bind(min_votes)
            .bind(limit)
            .fetch_all(self.db.pool())
            .await?)
    }

    pub async fn count(&self) -> Result<i64, DbError> {
        Ok(sqlx::query_scalar("SELECT COUNT(*) FROM media_items")
            .fetch_one(self.db.pool())
            .await?)
    }

    /// Bulk insert, for ingestion and for the 500,000-row benchmark.
    ///
    /// One transaction for the whole batch. SQLite commits are the cost here, not
    /// the inserts: committing per row turns a few seconds into tens of minutes,
    /// because each commit is a durability barrier.
    pub async fn insert_many(&self, items: &[NewMediaItem]) -> Result<Vec<i64>, DbError> {
        let mut tx = self.db.pool().begin().await?;
        let mut ids = Vec::with_capacity(items.len());

        for item in items {
            let kind = item.kind.unwrap_or(MediaKind::Film);
            let id: i64 = sqlx::query_scalar(
                "INSERT INTO media_items (
                     kind, primary_title, sort_title, release_year, runtime_minutes,
                     original_language, synopsis, rating, rating_votes, is_adult
                 ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                 RETURNING id",
            )
            .bind(kind.as_str())
            .bind(&item.primary_title)
            .bind(&item.sort_title)
            .bind(item.release_year)
            .bind(item.runtime_minutes)
            .bind(&item.original_language)
            .bind(&item.synopsis)
            .bind(item.rating)
            .bind(item.rating_votes)
            .bind(i64::from(item.is_adult))
            .fetch_one(&mut *tx)
            .await?;

            sqlx::query(
                "INSERT INTO titles (media_item_id, title, variant) VALUES (?, ?, 'primary')",
            )
            .bind(id)
            .bind(&item.primary_title)
            .execute(&mut *tx)
            .await?;

            ids.push(id);
        }

        tx.commit().await?;
        Ok(ids)
    }
}

fn map_media_item(row: sqlx::sqlite::SqliteRow) -> Result<MediaItem, DbError> {
    Ok(MediaItem {
        id: row.try_get("id")?,
        kind: parse_kind(row.try_get::<String, _>("kind")?.as_str()),
        primary_title: row.try_get("primary_title")?,
        sort_title: row.try_get("sort_title")?,
        release_year: row.try_get("release_year")?,
        release_date: row.try_get("release_date").unwrap_or(None),
        runtime_minutes: row.try_get("runtime_minutes")?,
        original_language: row.try_get("original_language")?,
        countries: row.try_get("countries").unwrap_or(None),
        synopsis: row.try_get("synopsis")?,
        rating: row.try_get("rating")?,
        rating_votes: row.try_get("rating_votes")?,
        is_adult: row.try_get::<i64, _>("is_adult")? != 0,
    })
}

/// The schema's `CHECK` constraint is the gate, so anything reaching here is
/// already one of the eight. Falling back to `Film` rather than panicking keeps a
/// corrupted row from taking down a library scan.
fn parse_kind(s: &str) -> MediaKind {
    MediaKind::ALL
        .into_iter()
        .find(|k| k.as_str() == s)
        .unwrap_or(MediaKind::Film)
}

fn parse_variant(s: &str) -> TitleVariant {
    TitleVariant::ALL
        .into_iter()
        .find(|v| v.as_str() == s)
        .unwrap_or(TitleVariant::Alternative)
}
