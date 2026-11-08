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
    EXTRAS_WORDS.iter().any(|w| c.contains(&format!(" {w} ")))
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
            let p = parse(name);
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
