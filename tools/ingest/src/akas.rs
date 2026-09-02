//! Alternative titles, for core-tier titles only.
//!
//! `SPEC.md` §6.2 requires romaji, native and english title variants explicitly, and
//! this is where most of them come from. It is also what makes search work for
//! someone who types "Nausicaa" for 風の谷のナウシカ.
//!
//! `title.akas` is 59.1 million rows; 10% of them point at a core title.
//!
//! # What IMDb gives us, and what it does not
//!
//! `title.akas` has `region`, `language` and `isOriginalTitle`. It has **no romaji
//! flag** — a transliteration and a native-script title are both just rows with
//! `language = ja`.
//!
//! So the variant is decided by looking at the **script of the text itself**: a
//! Japanese-language title written in Latin characters is a transliteration; one
//! written in kana or kanji is the native form. That is a heuristic, it is stated as
//! one, and it is correct for the overwhelming majority of the cases §6.2 cares
//! about. AniList (subtask 4.4) supplies romaji as an asserted fact and should
//! overwrite anything guessed here.

use std::collections::HashMap;

use crate::imdb;
use crate::job::{Batch, Job, JobError, SqliteTx};
use crate::load::tconst_id;
use crate::tsv::TsvReader;

const BATCH: usize = 20_000;
const ROWS_PER_STATEMENT: usize = 200;

/// Languages whose native script is not Latin, so a Latin-script title in one of
/// them is a transliteration rather than the original.
const TRANSLITERATED: &[&str] = &[
    "ja", "ko", "zh", "cmn", "yue", "th", "ru", "ar", "he", "el", "hi",
];

/// Does this text use a non-Latin script?
///
/// Deliberately crude: anything outside Basic Latin and Latin-1/Extended-A counts as
/// non-Latin. That misclassifies nothing we care about — a Japanese title is either
/// almost entirely kana/kanji or almost entirely ASCII — and it needs no dependency.
fn is_non_latin(text: &str) -> bool {
    let significant = text.chars().filter(|c| c.is_alphabetic()).count();
    if significant == 0 {
        return false;
    }
    let non_latin = text
        .chars()
        .filter(|c| c.is_alphabetic() && *c as u32 > 0x024F)
        .count();
    non_latin * 2 > significant
}

/// Which `titles.variant` an akas row is.
///
/// Order matters: `isOriginalTitle` is an assertion by IMDb and outranks any guess.
pub fn variant(
    is_original: bool,
    language: Option<&str>,
    region: Option<&str>,
    title: &str,
) -> &'static str {
    if is_original {
        return "original";
    }
    let non_latin = is_non_latin(title);

    match (language, region) {
        // A non-Latin-script language written in Latin characters: a transliteration.
        (Some(lang), _) if TRANSLITERATED.contains(&lang) => {
            if non_latin {
                "native"
            } else {
                "romaji"
            }
        }
        (Some("en"), _) => "english",

        // A KNOWN language that is neither English nor a transliterated one. The
        // region says nothing here and must not be consulted: IMDb lists the Spanish
        // title of Spirited Away with region US and the French one with region CA,
        // and reading the region first labelled both of them `english`. A release
        // region is where a title was used, not what language it is in.
        (Some(_), _) => {
            if non_latin {
                "native"
            } else {
                "alternative"
            }
        }

        // No language at all — now the region is the only evidence there is.
        (None, Some("US" | "GB" | "CA" | "AU")) => "english",
        (None, _) => {
            if non_latin {
                "native"
            } else {
                "alternative"
            }
        }
    }
}

/// Load alternative titles for core-tier titles.
pub async fn load_akas(
    job: &mut Job<'_>,
    akas: std::path::PathBuf,
    core: std::sync::Arc<HashMap<u32, i64>>,
) -> Result<(), JobError> {
    job.run_step("title.akas", move |tx, cursor| {
        let akas = akas.clone();
        let core = std::sync::Arc::clone(&core);
        Box::pin(async move {
            let mut reader = TsvReader::open(&akas)?;
            imdb::check_columns(&reader, &imdb::TITLE_AKAS)?;

            let mut pending: Vec<Aka> = Vec::new();
            if let Some(cursor) = cursor.as_deref() {
                reader.seek_past("titleId", cursor)?;
                if let Some(aka) = aka_from(&reader, &core) {
                    pending.push(aka);
                }
            }

            let mut last_id = None;
            while pending.len() < BATCH {
                if !reader.advance()? {
                    break;
                }
                if let Some(aka) = aka_from(&reader, &core) {
                    pending.push(aka);
                }
                if let Some(row) = reader.current_row() {
                    if let Some(id) = row.get("titleId") {
                        last_id = Some(id.to_string());
                    }
                }
            }

            let count = pending.len() as i64;
            let finished = pending.len() < BATCH;
            insert_akas(tx, &pending).await?;

            Ok(match (finished, last_id) {
                (false, Some(id)) => Batch::more(id, count),
                _ => Batch::finished(count),
            })
        })
    })
    .await?;
    Ok(())
}

struct Aka {
    media_item_id: i64,
    title: String,
    variant: &'static str,
    language: Option<String>,
    region: Option<String>,
}

fn aka_from(reader: &TsvReader, core: &HashMap<u32, i64>) -> Option<Aka> {
    let row = reader.current_row()?;
    let media_item_id = *core.get(&row.get("titleId").and_then(tconst_id)?)?;
    let title = row.get("title")?;
    let language = row.get("language");
    let region = row.get("region");
    let is_original = matches!(row.get("isOriginalTitle"), Some("1"));

    Some(Aka {
        media_item_id,
        variant: variant(is_original, language, region, title),
        title: title.to_string(),
        language: language.map(str::to_string),
        region: region.map(str::to_string),
    })
}

async fn insert_akas(tx: &mut SqliteTx<'_>, akas: &[Aka]) -> Result<(), JobError> {
    for chunk in akas.chunks(ROWS_PER_STATEMENT) {
        // ON CONFLICT DO NOTHING against idx_titles_unique, which is
        // (media_item_id, variant, language, region). IMDb lists several akas that
        // collapse to the same key — a series with two US titles differing only in
        // punctuation, for instance — and the first one wins.
        let sql = format!(
            "INSERT INTO titles (media_item_id, title, variant, language, region) \
             VALUES {} ON CONFLICT DO NOTHING",
            vec!["(?, ?, ?, ?, ?)"; chunk.len()].join(", ")
        );
        let mut query = sqlx::query(&sql);
        for aka in chunk {
            query = query
                .bind(aka.media_item_id)
                .bind(&aka.title)
                .bind(aka.variant)
                .bind(&aka.language)
                .bind(&aka.region);
        }
        query
            .execute(&mut **tx)
            .await
            .map_err(|e| JobError::step("title.akas", format!("titles: {e}")))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_asserted_original_outranks_any_guess() {
        assert_eq!(variant(true, Some("ja"), None, "七人の侍"), "original");
        assert_eq!(
            variant(true, Some("en"), Some("US"), "Seven Samurai"),
            "original"
        );
    }

    #[test]
    fn a_release_region_never_overrides_a_known_language() {
        // Found in the real catalogue: IMDb lists the Spanish title of Spirited Away
        // with region US and the French one with region CA. Consulting the region
        // before checking the language labelled both of them `english`, so the film's
        // "English title" was whichever regional title happened to load last.
        assert_eq!(
            variant(false, Some("es"), Some("US"), "El viaje de Chihiro"),
            "alternative"
        );
        assert_eq!(
            variant(false, Some("fr"), Some("CA"), "Le voyage de Chihiro"),
            "alternative"
        );
        // The region is still the only evidence when there is no language at all.
        assert_eq!(
            variant(false, None, Some("US"), "Some Title"),
            "english",
            "region remains the fallback when the language is unknown"
        );
    }

    #[test]
    fn a_japanese_title_in_latin_script_is_romaji() {
        // The §6.2 case: someone types "Nausicaa", not 風の谷のナウシカ.
        assert_eq!(
            variant(false, Some("ja"), None, "Shichinin no samurai"),
            "romaji"
        );
        assert_eq!(variant(false, Some("ja"), None, "七人の侍"), "native");
    }

    #[test]
    fn the_same_rule_holds_for_other_non_latin_scripts() {
        assert_eq!(
            variant(false, Some("ru"), None, "Bronenosets Potyomkin"),
            "romaji"
        );
        assert_eq!(
            variant(false, Some("ru"), None, "Броненосец Потёмкин"),
            "native"
        );
        assert_eq!(variant(false, Some("ko"), None, "Gisaengchung"), "romaji");
    }

    #[test]
    fn english_is_recognised_by_language_or_by_region() {
        assert_eq!(variant(false, Some("en"), None, "Seven Samurai"), "english");
        assert_eq!(variant(false, None, Some("US"), "Seven Samurai"), "english");
        assert_eq!(variant(false, None, Some("GB"), "Seven Samurai"), "english");
    }

    #[test]
    fn anything_else_is_alternative() {
        assert_eq!(
            variant(false, Some("fr"), Some("FR"), "Les sept samouraïs"),
            "alternative"
        );
        assert_eq!(variant(false, None, None, "Some Title"), "alternative");
    }

    #[test]
    fn a_non_latin_title_with_no_language_is_still_native() {
        assert_eq!(variant(false, None, Some("JP"), "七人の侍"), "native");
    }

    #[test]
    fn accented_latin_is_not_mistaken_for_a_foreign_script() {
        // Les sept samouraïs, Amélie, Katō — all Latin.
        assert!(!is_non_latin("Les sept samouraïs"));
        assert!(!is_non_latin("Amélie"));
        assert!(!is_non_latin("Daisuke Katō"));
        assert!(is_non_latin("七人の侍"));
        assert!(is_non_latin("Броненосец"));
    }

    #[test]
    fn punctuation_and_digits_do_not_decide_the_script() {
        assert!(!is_non_latin("2001: A Space Odyssey"));
        assert!(!is_non_latin("8½"), "a numeral is not a script");
        assert!(!is_non_latin("---"), "no letters at all is not non-Latin");
    }
}
