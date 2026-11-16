//! What the folders around a file say. A library is usually organised: "The Wire (2002)/
//! Season 1/03 - The Buys.mkv" names its series and season even when the file does not.

use filename_parser::{comparable, parse};

/// The nearest meaningful folder's reading.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FolderHint {
    pub title: String,
    pub year: Option<u16>,
    pub season: Option<u32>,
}

/// Folders that organise rather than name: "Season 2", "S03", "Specials", "VIDEO_TS".
fn season_of(name: &str) -> Option<u32> {
    let c = comparable(name);
    let digits = c
        .strip_prefix("season ")
        .or_else(|| c.strip_prefix("series "))
        .or_else(|| c.strip_prefix("saison "))
        .or_else(|| c.strip_prefix('s'))?
        .trim();
    digits.parse().ok()
}

fn organises(name: &str) -> bool {
    let c = comparable(name);
    season_of(name).is_some()
        || matches!(
            c.as_str(),
            "specials" | "extras" | "featurettes" | "video ts" | "bdmv" | "stream" | "subs"
        )
        || c.starts_with("disc ")
        || c.starts_with("cd ")
}

/// Folders that hold a release's extras rather than the work: never searched for, so a
/// featurette called "Cherry Pie" cannot become a film (Phase 16, the author's Twin Peaks).
const EXTRAS_FOLDERS: &[&str] = &[
    "extras",
    "extra",
    "featurettes",
    "featurette",
    "behind the scenes",
    "deleted scenes",
    "interviews",
    "trailers",
    "trailer",
    "menu art",
    "menus",
    "bonus",
    "bonus features",
    "special features",
    "shorts",
    "making of",
    "scenes",
    "samples",
    "sample",
    "other",
    "promos",
    "nc",
    "creditless",
];

/// Words in a file's name that mark an extra wherever it sits: only words no real title
/// uses ("trailer" is not one: Trailer Park Boys; trailers are caught by their folder).
const EXTRAS_WORDS: &[&str] = &[
    "behind the scenes",
    "featurette",
    "gag reel",
    "deleted scene",
    "deleted scenes",
    "bloopers",
    "ncop",
    "nced",
    "creditless",
];

/// Whether a file is an extra: under an extras folder, or named as one. `folders` are
/// those between the library root and the file, outermost first.
pub fn is_extra(folders: &[String], name: &str) -> bool {
    if folders
        .iter()
        .any(|f| EXTRAS_FOLDERS.contains(&comparable(f).as_str()))
    {
        return true;
    }
    let stem = name.rsplit_once('.').map_or(name, |(s, _)| s);
    let c = format!(" {} ", comparable(stem));
    if EXTRAS_WORDS.iter().any(|w| c.contains(&format!(" {w} "))) {
        return true;
    }
    // "Monster - Extra 1 - Origin": "extra" numbered is a release's extra. Alone it is not
    // ("Extra Ordinary", a film; "Extras", a series).
    let words: Vec<&str> = c.split_whitespace().collect();
    words
        .windows(2)
        .any(|w| w[0] == "extra" && w[1].chars().all(|ch| ch.is_ascii_digit()))
}

/// A prefix someone put on a folder to order their shelf: "2a. Samurai Champloo (2004)",
/// "01. Twin Peaks", "3) Monster". One to three digits, perhaps a letter, then "." or ")"
/// and a space: a title that starts with a number ("2001 A Space Odyssey", "12 Angry
/// Men", "1917") has no such punctuation after it.
fn without_shelf_order(name: &str) -> &str {
    let b = name.as_bytes();
    let digits = b.iter().take(3).take_while(|c| c.is_ascii_digit()).count();
    if digits == 0 {
        return name;
    }
    let mut i = digits;
    if b.get(i).is_some_and(u8::is_ascii_alphabetic) {
        i += 1;
    }
    match (b.get(i), b.get(i + 1)) {
        (Some(b'.' | b')'), Some(b' ')) => name[i + 2..].trim_start(),
        _ => name,
    }
}

/// Read `folders` (outermost first, the library root itself excluded): the season from
/// the nearest season folder, the title and year from the nearest folder that names
/// something.
pub fn hint(folders: &[String]) -> FolderHint {
    let mut out = FolderHint::default();
    for name in folders.iter().rev() {
        if out.season.is_none() {
            out.season = season_of(name);
        }
        if !organises(name) {
            let p = parse(without_shelf_order(name));
            if !p.title.is_empty() {
                out.title = p.title;
                out.year = p.year;
                if out.season.is_none() {
                    out.season = p.season;
                }
                break;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(folders: &[&str]) -> FolderHint {
        hint(&folders.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    }

    fn v(f: &[&str]) -> Vec<String> {
        f.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn a_shelf_order_prefix_is_not_part_of_the_show() {
        // The author's library (2026-11-16).
        let champloo = h(&["2a. Samurai Champloo (2004)"]);
        assert_eq!(champloo.title, "Samurai Champloo");
        assert_eq!(champloo.year, Some(2004));
        assert_eq!(h(&["01. Twin Peaks"]).title, "Twin Peaks");
        // A title that starts with a number keeps it.
        assert_eq!(
            h(&["2001 A Space Odyssey (1968)"]).title,
            "2001 A Space Odyssey"
        );
        assert_eq!(h(&["12 Angry Men (1957)"]).title, "12 Angry Men");
    }

    #[test]
    fn an_extra_is_known_by_its_number_too() {
        let monster = "Monster [2004-2005] DVDRip x265 AC-3 DD 2.0 GRPX [GRPY]";
        assert!(is_extra(
            &v(&[monster]),
            "Monster - Extra 1 - Origin DVDRip x265 GRPX [GRPY].mkv"
        ));
        assert!(!is_extra(&v(&[]), "Extra Ordinary (2019) 1080p.mkv"));
        assert!(!is_extra(
            &v(&[monster]),
            "Monster - Chapter 01 - Herr Dr. Tenma DVDRip x265 GRPX [GRPY].mkv"
        ));
    }

    #[test]
    fn extras_are_known_by_their_folder_or_their_name() {
        let show = "Twin Peaks (1990) Season 1-3 S01-S03 (1080p BluRay x265 NORDALE)";
        assert!(is_extra(
            &v(&[show, "Featurettes", "Season 2"]),
            "Cherry Pie.mkv"
        ));
        assert!(is_extra(
            &v(&[show, "Featurettes", "Season 1", "Behind the Scenes"]),
            "Tim Hunter.mkv"
        ));
        assert!(is_extra(
            &v(&["Cowboy Bebop (1998)"]),
            "[KOHAKU] Cowboy Bebop - NCOP [1080p].mkv"
        ));
        assert!(is_extra(
            &v(&["Breaking Bad (2008)"]),
            "Breaking.Bad.Gag.Reel.mkv"
        ));
        assert!(!is_extra(
            &v(&[show, "Season 1"]),
            "Twin Peaks (1990) - S01E03 - Zen.mkv"
        ));
        assert!(!is_extra(
            &v(&["Trailer Park Boys"]),
            "Trailer.Park.Boys.S01E01.mkv"
        ));
        assert!(
            !is_extra(&v(&["Films"]), "The Other Guys (2010).mkv"),
            "'other' as a folder only"
        );
        assert!(
            !is_extra(&v(&["Films"]), "Interview (2007).mkv"),
            "'interviews' as a folder only"
        );
    }

    #[test]
    fn the_nearest_naming_folder_and_the_season() {
        assert_eq!(
            h(&["The Wire (2002)", "Season 01"]),
            FolderHint {
                title: "The Wire".into(),
                year: Some(2002),
                season: Some(1)
            }
        );
        assert_eq!(h(&["Mad Men", "S04"]).season, Some(4));
        assert_eq!(h(&["Persona (1966)", "VIDEO_TS"]).title, "Persona");
        assert_eq!(h(&[]), FolderHint::default());
        assert_eq!(h(&["Films", "Stalker 1979"]).year, Some(1979));
    }
}
