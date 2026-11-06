//! Subtitles that are files: beside the film on disk, or in the same torrent (SPEC.md
//! §14 step 2). Found by name, so the language comes from the name too:
//! `Film.en.srt`, `Film.eng.forced.srt`, `Subs/English.srt`.

use crate::{Found, Origin};

/// Extensions of the films themselves.
const VIDEO: [&str; 7] = ["mkv", "mp4", "avi", "m4v", "mov", "webm", "ts"];

fn is_video(path: &str) -> bool {
    split_ext(file_name(path)).is_some_and(|(_, e)| VIDEO.contains(&e.as_str()))
}

/// Extensions the player can load as subtitles.
const EXTENSIONS: [&str; 5] = ["srt", "ass", "ssa", "vtt", "sub"];

/// Common names and ISO 639-2 codes, to the BCP 47 primary subtag the app uses.
const LANGUAGES: [(&str, &str); 24] = [
    ("english", "en"),
    ("eng", "en"),
    ("en", "en"),
    ("french", "fr"),
    ("fre", "fr"),
    ("fra", "fr"),
    ("german", "de"),
    ("ger", "de"),
    ("deu", "de"),
    ("spanish", "es"),
    ("spa", "es"),
    ("italian", "it"),
    ("ita", "it"),
    ("japanese", "ja"),
    ("jpn", "ja"),
    ("hindi", "hi"),
    ("hin", "hi"),
    ("portuguese", "pt"),
    ("por", "pt"),
    ("dutch", "nl"),
    ("dut", "nl"),
    ("nld", "nl"),
    ("korean", "ko"),
    ("kor", "ko"),
];

/// Two-letter codes are recognised generally; a short list stands in for every
/// language's name, which is a dictionary, not a parser.
fn language(token: &str) -> Option<String> {
    let t = token.to_ascii_lowercase();
    if let Some((_, code)) = LANGUAGES.iter().find(|(name, _)| *name == t) {
        return Some((*code).to_string());
    }
    (t.len() == 2 && t.bytes().all(|b| b.is_ascii_lowercase())).then_some(t)
}

fn split_ext(name: &str) -> Option<(&str, String)> {
    let (stem, ext) = name.rsplit_once('.')?;
    Some((stem, ext.to_ascii_lowercase()))
}

/// What a subtitle file's name says about it, relative to the film's stem.
fn describe(name: &str) -> (Option<String>, bool) {
    let Some((stem, _)) = split_ext(name) else {
        return (None, false);
    };
    let tokens: Vec<&str> = stem
        .split(['.', '_', ' ', '-', '[', ']', '(', ')'])
        .collect();
    let forced = tokens.iter().any(|t| t.eq_ignore_ascii_case("forced"));
    // The language is the last token that is one, scanning from the end, where names
    // put it ("Film.2019.en.forced").
    let lang = tokens.iter().rev().find_map(|t| language(t));
    (lang, forced)
}

fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

fn is_subtitle(path: &str) -> bool {
    split_ext(file_name(path)).is_some_and(|(_, e)| EXTENSIONS.contains(&e.as_str()))
}

/// Subtitle files beside a film: `names` is what the film's folder contains. A file
/// belongs to the film if its name starts with the film's stem — or if the folder holds
/// only one film, any subtitle in it does.
pub fn beside(film: &str, names: &[String]) -> Vec<Found> {
    let film_name = file_name(film);
    let stem = split_ext(film_name)
        .map_or(film_name, |(s, _)| s)
        .to_ascii_lowercase();
    let films_here = names.iter().filter(|n| is_video(n)).count();
    let dir = film.len() - film_name.len();
    names
        .iter()
        .filter(|n| is_subtitle(n))
        .filter(|n| films_here <= 1 || n.to_ascii_lowercase().starts_with(&stem))
        .map(|n| {
            let (language, forced) = describe(n);
            Found {
                origin: Origin::Sidecar {
                    path: format!("{}{n}", &film[..dir]),
                },
                language,
                forced,
            }
        })
        .collect()
}

/// Subtitle files in the film's torrent: `files` is (index, path inside the torrent).
/// Ones beside the film or in a `Subs`-like folder count; a torrent of a whole season
/// keeps each episode's to itself by the same stem rule as on disk.
pub fn in_torrent(film_index: u32, files: &[(u32, String)]) -> Vec<Found> {
    let Some((_, film)) = files.iter().find(|(i, _)| *i == film_index) else {
        return vec![];
    };
    let stem = split_ext(file_name(film))
        .map_or("", |(s, _)| s)
        .to_ascii_lowercase();
    let videos = files.iter().filter(|(_, p)| is_video(p)).count();
    files
        .iter()
        .filter(|(_, p)| is_subtitle(p))
        .filter(|(_, p)| {
            videos <= 1
                || file_name(p).to_ascii_lowercase().starts_with(&stem)
                || p.to_ascii_lowercase().contains(&stem)
        })
        .map(|(i, p)| {
            let (language, forced) = describe(file_name(p));
            // A folder named for the language ("Subs/English.srt") says it as well.
            let language =
                language.or_else(|| p.split(['/', '\\']).rev().nth(1).and_then(language_folder));
            Found {
                origin: Origin::Torrent {
                    file_index: *i,
                    path: p.clone(),
                },
                language,
                forced,
            }
        })
        .collect()
}

fn language_folder(name: &str) -> Option<String> {
    language(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn langs(found: &[Found]) -> Vec<(Option<&str>, bool)> {
        found
            .iter()
            .map(|f| (f.language.as_deref(), f.forced))
            .collect()
    }

    #[test]
    fn languages_come_from_the_name() {
        let names: Vec<String> = [
            "Sunrise.1927.mkv",
            "Sunrise.1927.en.srt",
            "Sunrise.1927.eng.forced.srt",
            "Sunrise.1927.French.ass",
            "Sunrise.1927.srt",
            "Sunrise.1927.nfo",
        ]
        .map(String::from)
        .to_vec();
        let found = beside("D:/Films/Sunrise.1927.mkv", &names);
        assert_eq!(
            langs(&found),
            [
                (Some("en"), false),
                (Some("en"), true),
                (Some("fr"), false),
                (None, false)
            ]
        );
        assert!(
            matches!(&found[0].origin, Origin::Sidecar { path } if path == "D:/Films/Sunrise.1927.en.srt")
        );
    }

    #[test]
    fn in_a_folder_of_episodes_each_keeps_its_own() {
        let names: Vec<String> = [
            "Show.S01E01.mkv",
            "Show.S01E02.mkv",
            "Show.S01E01.en.srt",
            "Show.S01E02.en.srt",
        ]
        .map(String::from)
        .to_vec();
        let found = beside("E:/Show/Show.S01E02.mkv", &names);
        assert_eq!(found.len(), 1);
        assert!(
            matches!(&found[0].origin, Origin::Sidecar { path } if path.ends_with("S01E02.en.srt"))
        );
    }

    #[test]
    fn a_torrents_subs_folder_counts_and_names_the_language() {
        let files = vec![
            (0, "Film (1927)/Film.1927.mkv".to_string()),
            (1, "Film (1927)/Subs/English.srt".to_string()),
            (2, "Film (1927)/Subs/Spanish/2_Spanish.srt".to_string()),
            (3, "Film (1927)/sample.txt".to_string()),
        ];
        let found = in_torrent(0, &files);
        assert_eq!(langs(&found), [(Some("en"), false), (Some("es"), false)]);
        assert!(matches!(
            found[0].origin,
            Origin::Torrent { file_index: 1, .. }
        ));
    }
}
