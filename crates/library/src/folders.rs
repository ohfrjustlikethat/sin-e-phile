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
