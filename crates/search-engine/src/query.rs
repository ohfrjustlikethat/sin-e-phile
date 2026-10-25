//! Query understanding: pulling structured filters out of what someone typed.
//!
//! `SPEC.md` Phase 5: *"detect and extract structured filters from natural language
//! (year ranges, 'in Japanese', 'under 100 minutes', 'directed by') and apply them as
//! **constraints rather than as embedding input**"*.
//!
//! # Why this is not just a nicety
//!
//! "Kurosawa samurai films from the 50s" embedded whole asks the model to encode a
//! decade, and models are poor at numbers. Worse, the digits dilute the part that
//! carries meaning. Extracted, the decade becomes `release_year BETWEEN 1950 AND 1959`
//! — exact, cheap, and applied by the database — and "Kurosawa samurai films" is what
//! gets embedded, which is what the vector half is good at.
//!
//! # The rule that shapes everything here: a wrong filter is worse than no filter
//!
//! A missed filter costs some ranking quality. A **wrong** filter *excludes correct
//! answers entirely*, and the user sees an empty page with no way to tell why. So every
//! pattern below is deliberately narrow, matches only unambiguous phrasing, and leaves
//! anything it is unsure about in the text.
//!
//! # Language is specified and cannot be built
//!
//! The fourth filter the spec names has nowhere to read from: `media_items.
//! original_language` is **NULL on all 2,702,737 rows**, because the IMDb datasets do
//! not publish it. Shipping a language filter today would mean shipping one that
//! silently matches nothing — the exact failure this module's design rule exists to
//! prevent. Recorded as debt instead.

/// What a query turned out to be asking for.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Parsed {
    /// What is left to embed and to hand to BM25.
    ///
    /// **Empty when the query was nothing but filters** — "directed by Akira Kurosawa"
    /// leaves no text at all. That is information, not a problem to paper over: the
    /// caller answers such a query from the filters themselves
    /// (`SearchRepository::by_filters`) rather than searching for an empty string.
    pub text: String,
    /// Inclusive, both ends resolved — `Some((1990, 1999))` for "the 90s".
    pub years: Option<(i64, i64)>,
    pub runtime: Option<Runtime>,
    /// The name as typed, for matching against `credits.role = 'director'`.
    pub director: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Runtime {
    /// "under 100 minutes", "less than an hour and a half".
    Under(i64),
    /// "over two hours".
    Over(i64),
}

impl Parsed {
    /// Did anything get extracted? A caller with no filters can skip the SQL entirely.
    pub fn has_filters(&self) -> bool {
        self.years.is_some() || self.runtime.is_some() || self.director.is_some()
    }
}

/// The latest year a query can mean, so "after 2010" has an upper bound.
///
/// Deliberately generous rather than `now()`: a catalogue holds announced titles with
/// future release years, and clamping to today would hide them. Not read from the clock,
/// because a function whose output changes with the date cannot be tested.
const LATEST_YEAR: i64 = 2100;

/// Extract what can be extracted, leave the rest as text.
pub fn parse(query: &str) -> Parsed {
    let mut remaining = query.to_string();

    // Order matters: the director's name is taken first, because it may contain a year
    // ("directed by John Ford 1939") and stripping the year first would cut the phrase
    // in half.
    let director = take_director(&mut remaining);
    let years = take_years(&mut remaining);
    let runtime = take_runtime(&mut remaining);

    let text = tidy(&remaining);
    Parsed {
        // Empty stays empty when something WAS extracted. If nothing was, the original
        // survives — an unparsed query is still a query.
        text: if text.is_empty() && director.is_none() && years.is_none() && runtime.is_none() {
            query.trim().to_string()
        } else {
            text
        },
        years,
        runtime,
        director,
    }
}

/// "directed by Akira Kurosawa" → the name, and the phrase removed.
///
/// Only "directed by". A bare "by X" is not enough: "a film by the sea" and "songs by
/// the fire" are ordinary English, and a filter on a director called "the sea" returns
/// nothing at all.
fn take_director(remaining: &mut String) -> Option<String> {
    let lower = remaining.to_lowercase();
    let marker = "directed by ";
    let at = lower.find(marker)?;
    let after = at + marker.len();

    // The name runs to a comma, or to a word that starts a new clause. Without a stop
    // list, "directed by Kurosawa in the 50s" would take "Kurosawa in the 50s" as a
    // person's name and match nobody.
    const STOPS: [&str; 6] = [" in ", " from ", " under ", " over ", " with ", " about "];
    let tail = &remaining[after..];
    let mut end = tail.len();
    for stop in STOPS {
        if let Some(found) = tail.to_lowercase().find(stop) {
            end = end.min(found);
        }
    }
    if let Some(comma) = tail.find(',') {
        end = end.min(comma);
    }

    let name = tail[..end].trim().to_string();
    if name.is_empty() {
        return None;
    }
    remaining.replace_range(at..after + end, " ");
    Some(name)
}

/// Decades, single years, and open-ended bounds.
fn take_years(remaining: &mut String) -> Option<(i64, i64)> {
    let lower = remaining.to_lowercase();

    // "between 1990 and 1999"
    if let Some(at) = lower.find("between ") {
        let tail = &lower[at..];
        let numbers = four_digit_years(tail);
        if numbers.len() >= 2 {
            let (from, to) = (numbers[0].min(numbers[1]), numbers[0].max(numbers[1]));
            if let Some(end) = tail.find(&numbers[1].to_string()) {
                remaining.replace_range(at..at + end + 4, " ");
                return Some((from, to));
            }
        }
    }

    // "the 90s", "1990s", "'90s". Two-digit decades below 30 are read as 2000s, which is
    // what someone typing "the 20s" in this century means — and the ambiguity is real,
    // so it is resolved once, here, rather than differently at each call site.
    if let Some((needle, range)) = decade_pattern(&lower) {
        remaining.replace_range(needle.0..needle.1, " ");
        return Some(range);
    }

    // "before 1980" / "after 2010" / "since 1995"
    for (marker, is_before) in [("before ", true), ("after ", false), ("since ", false)] {
        if let Some(at) = lower.find(marker) {
            let tail = &lower[at + marker.len()..];
            if let Some(year) = leading_year(tail) {
                remaining.replace_range(at..at + marker.len() + 4, " ");
                return Some(if is_before {
                    (0, year - 1)
                } else {
                    (year + 1, LATEST_YEAR)
                });
            }
        }
    }

    // "in 1994" / "from 1994"
    for marker in ["in ", "from ", "of "] {
        if let Some(at) = lower.find(marker) {
            let tail = &lower[at + marker.len()..];
            if let Some(year) = leading_year(tail) {
                remaining.replace_range(at..at + marker.len() + 4, " ");
                return Some((year, year));
            }
        }
    }

    None
}

/// The first decade mention: its byte range, and the years it means.
fn decade_pattern(lower: &str) -> Option<((usize, usize), (i64, i64))> {
    let bytes = lower.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        // A four-digit decade: 1990s
        if i + 5 <= bytes.len()
            && bytes[i..i + 4].iter().all(u8::is_ascii_digit)
            && bytes[i + 4] == b's'
        {
            if let Ok(year) = lower[i..i + 4].parse::<i64>() {
                let start = year - year % 10;
                return Some(((i, i + 5), (start, start + 9)));
            }
        }
        // A two-digit decade: 90s, '90s
        if i + 3 <= bytes.len()
            && bytes[i..i + 2].iter().all(u8::is_ascii_digit)
            && bytes[i + 2] == b's'
            && (i == 0 || !bytes[i - 1].is_ascii_digit())
        {
            if let Ok(two) = lower[i..i + 2].parse::<i64>() {
                let century = if two < 30 { 2000 } else { 1900 };
                let start = century + two - two % 10;
                let from = if i > 0 && bytes[i - 1] == b'\'' {
                    i - 1
                } else {
                    i
                };
                return Some(((from, i + 3), (start, start + 9)));
            }
        }
        i += 1;
    }
    None
}

fn four_digit_years(text: &str) -> Vec<i64> {
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    for i in 0..bytes.len() {
        if i + 4 <= bytes.len() && bytes[i..i + 4].iter().all(u8::is_ascii_digit) {
            let boundary_before = i == 0 || !bytes[i - 1].is_ascii_digit();
            let boundary_after = i + 4 == bytes.len() || !bytes[i + 4].is_ascii_digit();
            if boundary_before && boundary_after {
                if let Ok(year) = text[i..i + 4].parse::<i64>() {
                    if (1870..=LATEST_YEAR).contains(&year) {
                        out.push(year);
                    }
                }
            }
        }
    }
    out
}

fn leading_year(tail: &str) -> Option<i64> {
    let trimmed = tail.trim_start();
    if trimmed.len() < 4 {
        return None;
    }
    let candidate = &trimmed[..4];
    if !candidate.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    // A fifth digit means this is not a year.
    if trimmed.as_bytes().get(4).is_some_and(u8::is_ascii_digit) {
        return None;
    }
    candidate
        .parse::<i64>()
        .ok()
        .filter(|y| (1870..=LATEST_YEAR).contains(y))
}

/// "under 100 minutes", "over two hours", "less than 90 min".
fn take_runtime(remaining: &mut String) -> Option<Runtime> {
    let lower = remaining.to_lowercase();
    const UNDER: [&str; 4] = ["under ", "less than ", "shorter than ", "at most "];
    const OVER: [&str; 4] = ["over ", "more than ", "longer than ", "at least "];

    for (markers, is_under) in [(UNDER, true), (OVER, false)] {
        for marker in markers {
            let Some(at) = lower.find(marker) else {
                continue;
            };
            let tail = &lower[at + marker.len()..];
            let Some((minutes, consumed)) = leading_duration(tail) else {
                continue;
            };
            remaining.replace_range(at..at + marker.len() + consumed, " ");
            return Some(if is_under {
                Runtime::Under(minutes)
            } else {
                Runtime::Over(minutes)
            });
        }
    }
    None
}

/// "100 minutes", "two hours", "90 min", "an hour and a half" → minutes, bytes consumed.
fn leading_duration(tail: &str) -> Option<(i64, usize)> {
    const WORDS: [(&str, i64); 6] = [
        ("an hour and a half", 90),
        ("one and a half hours", 90),
        ("half an hour", 30),
        ("two hours", 120),
        ("three hours", 180),
        ("an hour", 60),
    ];
    for (phrase, minutes) in WORDS {
        if tail.starts_with(phrase) {
            return Some((minutes, phrase.len()));
        }
    }

    let digits: String = tail.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() {
        return None;
    }
    let value: i64 = digits.parse().ok()?;
    let rest = tail[digits.len()..].trim_start();
    let offset = tail.len() - rest.len();

    for (unit, multiplier) in [
        ("minutes", 1),
        ("minute", 1),
        ("mins", 1),
        ("min", 1),
        ("hours", 60),
        ("hour", 60),
        ("hrs", 60),
        ("hr", 60),
    ] {
        if rest.starts_with(unit) {
            return Some((value * multiplier, offset + unit.len()));
        }
    }
    None
}

/// Collapse the holes left by everything that was removed.
fn tidy(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last_space = true;
    for ch in text.chars() {
        if ch.is_whitespace() {
            if !last_space {
                out.push(' ');
            }
            last_space = true;
        } else {
            out.push(ch);
            last_space = false;
        }
    }
    // Filler a stripped phrase leaves dangling: "films from the 50s" loses "50s" and
    // would otherwise end "films from the". Stripped repeatedly, because removing one
    // word usually exposes another.
    //
    // Only ever from the END. These are ordinary words in the middle of a sentence, and
    // a query like "movies where the city is the main character" must survive intact.
    const DANGLING: [&str; 10] = [
        "from", "in", "of", "the", "a", "an", "between", "and", "before", "after",
    ];
    let mut cleaned = out.trim().trim_end_matches(',').trim().to_string();
    while let Some((head, last)) = cleaned.rsplit_once(' ') {
        if !DANGLING.contains(&last.to_lowercase().as_str()) {
            break;
        }
        cleaned = head.trim_end_matches(',').trim().to_string();
    }
    cleaned
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_decade_becomes_a_range_and_leaves_the_meaning_behind() {
        let parsed = parse("kurosawa samurai films from the 50s");
        assert_eq!(parsed.years, Some((1950, 1959)));
        assert_eq!(parsed.text, "kurosawa samurai films");
    }

    #[test]
    fn decades_are_written_several_ways() {
        assert_eq!(parse("horror from the 1980s").years, Some((1980, 1989)));
        assert_eq!(parse("music docs of the '90s").years, Some((1990, 1999)));
        // Someone typing "the 20s" in this century means the 2020s, not the 1920s. The
        // ambiguity is real; it is resolved once, here.
        assert_eq!(parse("comedies from the 20s").years, Some((2020, 2029)));
        assert_eq!(parse("silent films of the 1920s").years, Some((1920, 1929)));
    }

    #[test]
    fn open_ended_and_exact_years() {
        assert_eq!(parse("noir before 1950").years, Some((0, 1949)));
        assert_eq!(parse("anime after 2010").years, Some((2011, LATEST_YEAR)));
        assert_eq!(parse("films in 1994").years, Some((1994, 1994)));
        assert_eq!(
            parse("westerns between 1960 and 1975").years,
            Some((1960, 1975))
        );
    }

    #[test]
    fn runtime_in_digits_and_in_words() {
        assert_eq!(
            parse("comedies under 100 minutes").runtime,
            Some(Runtime::Under(100))
        );
        assert_eq!(
            parse("something under 90 min").runtime,
            Some(Runtime::Under(90))
        );
        assert_eq!(
            parse("epics over two hours").runtime,
            Some(Runtime::Over(120))
        );
        assert_eq!(
            parse("a film less than an hour and a half").runtime,
            Some(Runtime::Under(90))
        );
    }

    #[test]
    fn a_director_is_taken_only_from_an_unambiguous_phrase() {
        let parsed = parse("thrillers directed by Alfred Hitchcock");
        assert_eq!(parsed.director.as_deref(), Some("Alfred Hitchcock"));
        assert_eq!(parsed.text, "thrillers");

        // A bare "by" is ordinary English and must not become a filter — a director
        // called "the sea" matches nobody, and the user sees an empty page.
        let innocent = parse("a film by the sea");
        assert_eq!(innocent.director, None);
        assert_eq!(innocent.text, "a film by the sea");
    }

    #[test]
    fn a_directors_name_stops_before_the_next_clause() {
        // Without a stop list this takes "Kurosawa in the 50s" as a person's name.
        let parsed = parse("directed by Akira Kurosawa in the 50s");
        assert_eq!(parsed.director.as_deref(), Some("Akira Kurosawa"));
        assert_eq!(parsed.years, Some((1950, 1959)));
    }

    #[test]
    fn several_filters_at_once() {
        let parsed = parse("funny films directed by Billy Wilder from the 1950s under 100 minutes");
        assert_eq!(parsed.director.as_deref(), Some("Billy Wilder"));
        assert_eq!(parsed.years, Some((1950, 1959)));
        assert_eq!(parsed.runtime, Some(Runtime::Under(100)));
        assert_eq!(parsed.text, "funny films");
        assert!(parsed.has_filters());
    }

    #[test]
    fn a_query_that_is_nothing_but_filters_reports_no_text() {
        // "directed by Akira Kurosawa" is a complete query with nothing left to search
        // for. Reporting the leftover as text sent it to BM25 as "directed by akira
        // kurosawa" and returned NOTHING; the engine now answers it from the filter.
        let parsed = parse("directed by Akira Kurosawa");
        assert_eq!(parsed.text, "");
        assert_eq!(parsed.director.as_deref(), Some("Akira Kurosawa"));

        // A query that parsed nothing keeps itself — an unparsed query is still a query.
        assert_eq!(parse("1994").text, "1994");
    }

    #[test]
    fn ordinary_queries_are_left_completely_alone() {
        // The failure that matters: over-eager extraction EXCLUDES correct answers, and
        // the user cannot tell why. None of these contains a filter.
        for query in [
            "films about grief that aren't depressing",
            "like Wong Kar-wai but Korean",
            "blade runner",
            "movies where the city is the main character",
            "the thing",
        ] {
            let parsed = parse(query);
            assert!(!parsed.has_filters(), "{query} produced {parsed:?}");
            assert_eq!(parsed.text, query, "text must survive untouched");
        }
    }

    #[test]
    fn a_year_inside_a_title_is_not_a_filter() {
        // "2001" here is a name, and "blade runner 2049" is a sequel, not a date range.
        // Both must reach the exact-title tier intact.
        let parsed = parse("blade runner 2049");
        assert_eq!(parsed.years, None, "{parsed:?}");
        assert_eq!(parsed.text, "blade runner 2049");
    }
}
