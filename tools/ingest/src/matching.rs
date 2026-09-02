//! Matching AniList titles to catalogue entries.
//!
//! # The problem
//!
//! AniList carries no IMDb id and IMDb carries no AniList id, so the only bridge is
//! the title itself, plus a year. That is a fuzzy join over two datasets that
//! disagree about punctuation, romanisation, season numbering and which name is
//! "the" name — and Phase 4's exit criterion E5 hand-checks fifty of them,
//! deliberately including the cases that break naive matching.
//!
//! # The rule that shapes everything here
//!
//! **A wrong match is worse than no match.** A missed anime shows an IMDb entry with
//! no Japanese title — mildly worse. A wrong one attaches *Fullmetal Alchemist*'s
//! episodes and airing schedule to *Fullmetal Alchemist: Brotherhood*, and nothing
//! downstream will ever notice. Phase 12 sets the same bar for filenames at a
//! false-confident rate under 1%.
//!
//! So: match on any of AniList's three title forms, require the year to agree, and
//! **refuse when two candidates are equally good.** Ambiguity is reported, not
//! guessed at.

use std::collections::HashMap;

/// How a match was made, so a caller can weigh it — and so "why did this match?" has
/// an answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchKind {
    /// Normalised titles are identical and the years agree. The strong case.
    ExactTitleAndYear,
    /// Titles agree; one side has no year at all. Common for older entries.
    ExactTitleYearUnknown,
    /// Titles agree after season-suffix stripping, and years agree.
    SeasonAware,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Match {
    pub media_item_id: i64,
    pub kind: MatchKind,
    /// The AniList title form that matched, for the explanation.
    pub matched_on: String,
}

/// Why no match was made. Reported rather than collapsed into `None`, because
/// "ambiguous" and "absent" need different fixes and E5 measures both.
#[derive(Debug, Clone, PartialEq)]
pub enum NoMatch {
    /// Nothing in the catalogue has this title.
    NotInCatalogue,
    /// Several candidates were equally good. Guessing is what E5 exists to prevent.
    Ambiguous { candidates: usize },
    /// A title matched but the years disagree by more than the tolerance — usually
    /// a different entry in a franchise.
    YearConflict { catalogue: i64, anilist: i64 },
}

/// A catalogue row a match could land on.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub media_item_id: i64,
    pub title: String,
    pub year: Option<i64>,
    /// The catalogue's `media_items.kind`. Used only to break a tie, never to make
    /// one — see [`match_title`].
    pub kind: String,
}

/// Broadcast years disagree by one more often than they agree exactly: a series
/// airing from October runs into the next year, and the two datasets pick different
/// ends. Two is too loose — it starts matching adjacent seasons of the same show.
const YEAR_TOLERANCE: i64 = 1;

/// Normalise a title for comparison.
///
/// Lowercases, strips punctuation and collapses whitespace. Deliberately does NOT
/// transliterate or strip diacritics beyond what casing does: "Kimi no Na wa" and
/// "君の名は" are different strings and are supposed to be, because they are matched
/// as separate title forms rather than folded together.
pub fn normalise(title: &str) -> String {
    let mut out = String::with_capacity(title.len());
    let mut last_was_space = true;

    for ch in title.chars() {
        let ch = ch.to_lowercase().next().unwrap_or(ch);
        if ch.is_alphanumeric() {
            out.push(ch);
            last_was_space = false;
        } else if !last_was_space {
            // Punctuation becomes a single space: "Fullmetal Alchemist: Brotherhood"
            // and "Fullmetal Alchemist Brotherhood" are the same title written twice.
            out.push(' ');
            last_was_space = true;
        }
    }
    out.trim_end().to_string()
}

/// Strip a trailing season marker, returning the base title and the season number.
///
/// AniList lists seasons as separate entries with names like "Season 2", "2nd
/// Season" or a bare roman numeral; IMDb lists one series with seasons inside it. So
/// AniList's "Kaguya-sama: Love is War Season 2" must be able to find IMDb's
/// "Kaguya-sama: Love is War" — while still knowing it was season 2, because
/// throwing that away is how the episodes of two seasons end up merged.
pub fn split_season(title: &str) -> (String, Option<i64>) {
    let normalised = normalise(title);
    let words: Vec<&str> = normalised.split(' ').filter(|w| !w.is_empty()).collect();
    if words.len() < 2 {
        return (normalised, None);
    }

    // "... season 2" / "... 2nd season" / "... part 2"
    if let Some(number) = trailing_season(&words) {
        let keep = words.len() - number.1;
        if keep > 0 {
            return (words[..keep].join(" "), Some(number.0));
        }
    }

    // A trailing roman numeral: "Ghost in the Shell II".
    if let Some(value) = roman_numeral(words[words.len() - 1]) {
        if value > 1 && words.len() > 1 {
            return (words[..words.len() - 1].join(" "), Some(value));
        }
    }

    (normalised, None)
}

/// `(season number, how many trailing words it consumed)`.
fn trailing_season(words: &[&str]) -> Option<(i64, usize)> {
    let last = words[words.len() - 1];
    let second_last = words
        .get(words.len().wrapping_sub(2))
        .copied()
        .unwrap_or("");

    // "season 2" / "part 2"
    if (second_last == "season" || second_last == "part") && words.len() >= 3 {
        if let Ok(n) = last.parse::<i64>() {
            return Some((n, 2));
        }
    }
    // "2nd season" / "3rd season"
    if last == "season" && words.len() >= 3 {
        let ordinal = second_last.trim_end_matches(|c: char| c.is_alphabetic());
        if let Ok(n) = ordinal.parse::<i64>() {
            return Some((n, 2));
        }
    }
    None
}

fn roman_numeral(word: &str) -> Option<i64> {
    match word {
        "ii" => Some(2),
        "iii" => Some(3),
        "iv" => Some(4),
        "v" => Some(5),
        _ => None,
    }
}

/// An index of catalogue titles, normalised, for matching against.
#[derive(Debug, Default)]
pub struct TitleIndex {
    by_title: HashMap<String, Vec<Candidate>>,
}

impl TitleIndex {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, candidate: Candidate) {
        let key = normalise(&candidate.title);
        if key.is_empty() {
            return;
        }
        self.by_title.entry(key).or_default().push(candidate);
    }

    pub fn len(&self) -> usize {
        self.by_title.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_title.is_empty()
    }

    fn lookup(&self, title: &str) -> &[Candidate] {
        self.by_title
            .get(&normalise(title))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }
}

/// The three title forms AniList gives, in the order they are tried.
///
/// Romaji first: it is the form IMDb most often carries for anime, and the form the
/// akas script heuristic already produced on our side. English second. Native last,
/// because an IMDb entry rarely carries the Japanese script and a match on it is
/// therefore rare but very strong when it happens.
pub fn title_forms<'a>(
    romaji: Option<&'a str>,
    english: Option<&'a str>,
    native: Option<&'a str>,
) -> Vec<&'a str> {
    [romaji, english, native]
        .into_iter()
        .flatten()
        .filter(|t| !t.trim().is_empty())
        .collect()
}

/// Find the catalogue entry an AniList title refers to.
/// Does a catalogue kind agree with the shape AniList says this is?
///
/// Both the anime and non-anime kinds count, because a re-run matches against titles
/// a previous run already promoted.
fn kind_agrees(catalogue: &str, want: &str) -> bool {
    match want {
        "film" | "anime_film" => matches!(catalogue, "film" | "anime_film"),
        "series" | "anime_series" => matches!(catalogue, "series" | "anime_series"),
        _ => true,
    }
}

/// Find the catalogue entry an AniList title refers to.
///
/// `want_kind` is what AniList's `format` says this is — `"film"` or `"series"`. It
/// is a TIE-BREAKER and nothing more: it can choose between candidates already equally
/// good on title and year, and it can never create a match or reject one on its own.
/// "Naruto" is one series and two unrelated films with no year between them; without
/// the shape that is three coin flips, and with it there is one answer.
pub fn match_title(
    index: &TitleIndex,
    forms: &[&str],
    anilist_year: Option<i64>,
    want_kind: Option<&str>,
) -> Result<Match, NoMatch> {
    let mut year_conflict: Option<NoMatch> = None;
    let mut saw_a_title = false;

    for form in forms {
        for (kind, key) in [
            (MatchKind::ExactTitleAndYear, (*form).to_string()),
            (MatchKind::SeasonAware, split_season(form).0),
        ] {
            let candidates = index.lookup(&key);
            if candidates.is_empty() {
                continue;
            }
            saw_a_title = true;

            let viable: Vec<&Candidate> = candidates
                .iter()
                .filter(|c| years_agree(c.year, anilist_year))
                .collect();

            // COLLAPSE BY ITEM FIRST. A catalogue entry carries one title row per
            // spelling — "Death Note", "DEATH NOTE", "Death note" — and all of them
            // normalise to the same key, so each comes back as its own candidate.
            // Counting rows made one unambiguous series look like five rivals, and
            // refused Death Note, One Piece, Naruto and Attack on Titan: the four most
            // recognisable titles in the sample, every one of them ambiguous against
            // itself. Measured over 250 entries, this alone was 97 refusals.
            let mut ids: Vec<i64> = viable.iter().map(|c| c.media_item_id).collect();
            ids.sort_unstable();
            ids.dedup();

            match ids.len() {
                0 => {
                    // Remember the first conflict, but keep trying other forms — a
                    // different form may match a different entry cleanly.
                    if year_conflict.is_none() {
                        if let (Some(catalogue), Some(anilist)) = (candidates[0].year, anilist_year)
                        {
                            year_conflict = Some(NoMatch::YearConflict { catalogue, anilist });
                        }
                    }
                }
                1 => return Ok(settle(&viable, ids[0], kind, form, anilist_year)),
                _ => {
                    // Several genuinely different catalogue entries share this title.
                    // Narrow by STRENGTH OF EVIDENCE, in stages, and settle only if a
                    // stage leaves exactly one. Each stage may discard a candidate
                    // that is worse-evidenced than another; none may invent a match or
                    // reject the last one standing.
                    let mut pool: Vec<&Candidate> = viable.clone();

                    // Stage 1: a year that is KNOWN AND AGREES corroborates the match.
                    // A catalogue entry with no year at all merely fails to contradict
                    // it, and `years_agree` is permissive about exactly that. Treating
                    // the two as equal let a year-less stub veto a match the dated
                    // entry had earned: Death Note, Sword Art Online and Steins;Gate
                    // were all refused this way despite an exact year match sitting in
                    // the candidate set.
                    if anilist_year.is_some() {
                        let dated: Vec<&Candidate> =
                            pool.iter().copied().filter(|c| c.year.is_some()).collect();
                        if !dated.is_empty() {
                            pool = dated;
                        }
                    }

                    // Stage 2: AniList states whether this is a film or a series.
                    if let Some(want) = want_kind {
                        let agreeing: Vec<&Candidate> = pool
                            .iter()
                            .copied()
                            .filter(|c| kind_agrees(&c.kind, want))
                            .collect();
                        if !agreeing.is_empty() {
                            pool = agreeing;
                        }
                    }

                    let mut remaining: Vec<i64> = pool.iter().map(|c| c.media_item_id).collect();
                    remaining.sort_unstable();
                    remaining.dedup();

                    if remaining.len() == 1 {
                        return Ok(settle(&pool, remaining[0], kind, form, anilist_year));
                    }

                    // Picking one now would be a coin flip recorded as a fact.
                    return Err(NoMatch::Ambiguous {
                        candidates: remaining.len(),
                    });
                }
            }
        }
    }

    match (year_conflict, saw_a_title) {
        (Some(conflict), _) => Err(conflict),
        (None, _) => Err(NoMatch::NotInCatalogue),
    }
}

/// Build the `Match` for a settled candidate.
///
/// A match made where either side has no year is recorded as `ExactTitleYearUnknown`
/// rather than as the stronger kind it was reached by, because that is what actually
/// happened — the year did not corroborate it, it merely failed to contradict it.
fn settle(
    viable: &[&Candidate],
    media_item_id: i64,
    kind: MatchKind,
    form: &str,
    anilist_year: Option<i64>,
) -> Match {
    let chosen = viable
        .iter()
        .find(|c| c.media_item_id == media_item_id)
        .expect("the id came from viable");
    let kind = if anilist_year.is_none() || chosen.year.is_none() {
        MatchKind::ExactTitleYearUnknown
    } else {
        kind
    };
    Match {
        media_item_id,
        kind,
        matched_on: form.to_string(),
    }
}

fn years_agree(catalogue: Option<i64>, anilist: Option<i64>) -> bool {
    match (catalogue, anilist) {
        // A missing year on either side is not evidence against a match. Requiring
        // one would drop most pre-2000 entries.
        (None, _) | (_, None) => true,
        (Some(a), Some(b)) => (a - b).abs() <= YEAR_TOLERANCE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn index(rows: &[(i64, &str, Option<i64>)]) -> TitleIndex {
        let kinded_rows: Vec<(i64, &str, Option<i64>, &str)> = rows
            .iter()
            .map(|(a, b, c)| (*a, *b, *c, "series"))
            .collect();
        kinded(&kinded_rows)
    }

    #[test]
    fn a_corroborating_year_beats_a_candidate_with_no_year_at_all() {
        // `years_agree` is permissive about a missing year, which is right — it is not
        // evidence AGAINST a match. But it is not evidence for one either, and letting
        // a year-less stub tie with an exact year match refused Death Note, Sword Art
        // Online and Steins;Gate outright.
        let index = kinded(&[
            (1, "Death Note", Some(2006), "series"),
            (2, "Death Note", None, "series"),
        ]);
        let found = match_title(&index, &["Death Note"], Some(2006), Some("series"))
            .expect("the dated entry is better evidenced");
        assert_eq!(found.media_item_id, 1);
    }

    #[test]
    fn two_candidates_that_both_agree_on_year_stay_ambiguous() {
        // Narrowing by evidence must never become ranking. Nothing separates these.
        let index = kinded(&[
            (1, "Monster", Some(2004), "series"),
            (2, "Monster", Some(2004), "series"),
        ]);
        assert_eq!(
            match_title(&index, &["Monster"], Some(2004), Some("series")),
            Err(NoMatch::Ambiguous { candidates: 2 })
        );
    }

    #[test]
    fn a_year_less_candidate_still_matches_when_it_is_the_only_one() {
        // The narrowing may discard a worse-evidenced rival; it may never discard the
        // last candidate standing.
        let index = kinded(&[(1, "An Old Series", None, "series")]);
        let found = match_title(&index, &["An Old Series"], Some(1975), Some("series"))
            .expect("still a match");
        assert_eq!(found.media_item_id, 1);
        assert_eq!(found.kind, MatchKind::ExactTitleYearUnknown);
    }

    fn kinded(rows: &[(i64, &str, Option<i64>, &str)]) -> TitleIndex {
        let mut index = TitleIndex::new();
        for (id, title, year, kind) in rows {
            index.insert(Candidate {
                media_item_id: *id,
                title: (*title).to_string(),
                year: *year,
                kind: (*kind).to_string(),
            });
        }
        index
    }

    #[test]
    fn punctuation_does_not_prevent_a_match() {
        assert_eq!(
            normalise("Fullmetal Alchemist: Brotherhood"),
            normalise("Fullmetal Alchemist Brotherhood")
        );
        assert_eq!(
            normalise("Re:ZERO -Starting Life-"),
            "re zero starting life"
        );
    }

    #[test]
    fn scripts_are_not_folded_together() {
        // They are matched as separate FORMS, not merged into one string.
        assert_ne!(normalise("Kimi no Na wa"), normalise("君の名は"));
        assert_eq!(normalise("君の名は"), "君の名は");
    }

    #[test]
    fn a_season_suffix_is_stripped_but_remembered() {
        // AniList lists seasons separately; IMDb nests them. Throwing the number
        // away is how two seasons' episodes end up merged.
        assert_eq!(
            split_season("Kaguya-sama: Love is War Season 2"),
            ("kaguya sama love is war".to_string(), Some(2))
        );
        assert_eq!(
            split_season("Kaguya-sama: Love is War 2nd Season"),
            ("kaguya sama love is war".to_string(), Some(2))
        );
        assert_eq!(
            split_season("Ghost in the Shell II"),
            ("ghost in the shell".to_string(), Some(2))
        );
    }

    #[test]
    fn a_title_that_merely_ends_in_a_number_is_not_a_season() {
        // "Blade Runner 2049" is not season 2049 of Blade Runner.
        assert_eq!(split_season("Blade Runner 2049").1, None);
        assert_eq!(split_season("Ocean's 11").1, None);
    }

    #[test]
    fn an_exact_title_and_year_matches() {
        let index = index(&[(1, "Cowboy Bebop", Some(1998))]);
        let found = match_title(&index, &["Cowboy Bebop"], Some(1998), None).expect("match");
        assert_eq!(found.media_item_id, 1);
        assert_eq!(found.kind, MatchKind::ExactTitleAndYear);
    }

    #[test]
    fn a_year_off_by_one_still_matches() {
        // A series airing from October runs into the next year and the two datasets
        // pick different ends.
        let index = index(&[(1, "A Series", Some(2019))]);
        assert!(match_title(&index, &["A Series"], Some(2020), None).is_ok());
    }

    #[test]
    fn a_year_off_by_more_is_a_conflict_not_a_match() {
        // Usually a different entry in the same franchise.
        let index = index(&[(1, "Fullmetal Alchemist", Some(2003))]);
        let error =
            match_title(&index, &["Fullmetal Alchemist"], Some(2009), None).expect_err("no match");
        assert_eq!(
            error,
            NoMatch::YearConflict {
                catalogue: 2003,
                anilist: 2009
            }
        );
    }

    #[test]
    fn two_equally_good_candidates_are_refused_rather_than_guessed() {
        // THE RULE. A coin flip recorded as a fact is worse than no match at all.
        let index = index(&[(1, "Gamera", Some(1995)), (2, "Gamera", Some(1995))]);
        let error = match_title(&index, &["Gamera"], Some(1995), None).expect_err("refused");
        assert_eq!(error, NoMatch::Ambiguous { candidates: 2 });
    }

    #[test]
    fn a_missing_year_on_either_side_is_not_evidence_against() {
        // Requiring one would drop most pre-2000 entries.
        let no_year = index(&[(1, "An Old Series", None)]);
        let found = match_title(&no_year, &["An Old Series"], Some(1975), None).expect("match");
        assert_eq!(found.kind, MatchKind::ExactTitleYearUnknown);

        let no_anilist_year = index(&[(2, "Another", Some(1975))]);
        let found = match_title(&no_anilist_year, &["Another"], None, None).expect("match");
        assert_eq!(found.kind, MatchKind::ExactTitleYearUnknown);
    }

    #[test]
    fn the_english_title_is_tried_when_the_romaji_misses() {
        let index = index(&[(1, "Spirited Away", Some(2001))]);
        let forms = title_forms(
            Some("Sen to Chihiro no Kamikakushi"),
            Some("Spirited Away"),
            None,
        );
        let found = match_title(&index, &forms, Some(2001), None).expect("match");
        assert_eq!(found.matched_on, "Spirited Away");
    }

    #[test]
    fn the_native_script_matches_when_the_catalogue_carries_it() {
        // Rare, and very strong when it happens — akas gives us native titles.
        let index = index(&[(1, "君の名は。", Some(2016))]);
        let forms = title_forms(
            Some("Kimi no Na wa."),
            Some("Your Name."),
            Some("君の名は。"),
        );
        let found = match_title(&index, &forms, Some(2016), None).expect("match");
        assert_eq!(found.matched_on, "君の名は。");
    }

    #[test]
    fn a_season_entry_finds_the_base_series() {
        // AniList's "Season 2" against IMDb's one series entry.
        let index = index(&[(1, "Kaguya-sama: Love is War", Some(2019))]);
        let found = match_title(
            &index,
            &["Kaguya-sama: Love is War Season 2"],
            Some(2020),
            None,
        )
        .expect("match");
        assert_eq!(found.media_item_id, 1);
        assert_eq!(found.kind, MatchKind::SeasonAware);
    }

    #[test]
    fn nothing_in_the_catalogue_is_reported_distinctly_from_ambiguity() {
        // They need different fixes, and E5 measures both.
        let index = index(&[(1, "Something Else", Some(2000))]);
        assert_eq!(
            match_title(&index, &["Not Here At All"], Some(2000), None).expect_err("no match"),
            NoMatch::NotInCatalogue
        );
    }

    #[test]
    fn an_empty_title_form_is_skipped_rather_than_matching_everything() {
        let forms = title_forms(Some(""), Some("  "), Some("Real Title"));
        assert_eq!(forms, vec!["Real Title"]);
    }

    #[test]
    fn a_blank_candidate_never_enters_the_index() {
        let mut index = TitleIndex::new();
        index.insert(Candidate {
            media_item_id: 1,
            title: "!!!".to_string(),
            kind: "series".to_string(),
            year: None,
        });
        assert!(
            index.is_empty(),
            "punctuation-only titles normalise to nothing"
        );
    }

    #[test]
    fn a_later_form_can_rescue_a_year_conflict_on_an_earlier_one() {
        // The romaji hits the 2003 series; the english hits the 2009 one cleanly.
        let index = index(&[
            (1, "Fullmetal Alchemist", Some(2003)),
            (2, "Fullmetal Alchemist Brotherhood", Some(2009)),
        ]);
        let forms = title_forms(
            Some("Fullmetal Alchemist"),
            Some("Fullmetal Alchemist: Brotherhood"),
            None,
        );
        let found = match_title(&index, &forms, Some(2009), None).expect("match");
        assert_eq!(
            found.media_item_id, 2,
            "it did not stop at the first conflict"
        );
    }
}
