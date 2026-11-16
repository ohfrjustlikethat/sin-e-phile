//! A show's own files in the order a viewer expects, where the catalogue cannot place them
//! (Phase 16): a file matched to the show itself because the catalogue has no such episode
//! (Space Dandy's "S01E14" where IMDb's first season has 13; Twin Peaks' third season,
//! which IMDb lists as a separate 2017 series). Each keeps what its name says, season and
//! episode, and is never renumbered by adding seasons together (ADR-0031).

use crate::identify::{read, Evidence};

/// One file as the show's page lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShelvedFile {
    pub file_id: i64,
    pub path: String,
    pub season: Option<u32>,
    pub episode: Option<u32>,
    /// The episode's own title from the name ("Part 5 Case Files"), if it has one.
    pub title: Option<String>,
}

/// `files` (id and full path) in season, then episode, then name order; anything the
/// name gives no number for comes last, by name.
pub fn in_order(files: &[(i64, String)]) -> Vec<ShelvedFile> {
    let mut out: Vec<ShelvedFile> = files
        .iter()
        .map(|(id, path)| {
            let mut parts: Vec<String> = path.split(['/', '\\']).map(str::to_string).collect();
            let name = parts.pop().unwrap_or_default();
            let r = read(&Evidence {
                name,
                folders: parts,
                duration_minutes: None,
            });
            ShelvedFile {
                file_id: *id,
                path: path.clone(),
                season: r.season,
                episode: r.episodes.first().copied(),
                title: r.parsed.episode_title.clone(),
            }
        })
        .collect();
    let key = |f: &ShelvedFile| {
        (
            f.season.unwrap_or(u32::MAX),
            f.episode.unwrap_or(u32::MAX),
            f.path.to_lowercase(),
        )
    };
    out.sort_by_key(key);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shows_own_files_sort_by_season_and_episode_as_their_names_say() {
        // The author's Twin Peaks, season 3 (groups replaced), handed over out of order.
        let dir = r"D:\TV\Twin Peaks (1990) Season 1-3 (1080p BluRay x265 NORDALE)\Season 3";
        let f = |id: i64, n: &str| (id, format!(r"{dir}\{n}"));
        let files = [
            f(3, "Twin Peaks (1990) - S03E10 - Part 10 Laura Is the One (1080p BluRay x265 NORDALE).mkv"),
            f(1, "Twin Peaks (1990) - S03E02 - Part 2 The Stars Turn (1080p BluRay x265 NORDALE).mkv"),
            f(9, "notes.mkv"),
            f(2, "Twin Peaks (1990) - S03E05 - Part 5 Case Files (1080p BluRay x265 NORDALE).mkv"),
        ];
        let shelved = in_order(&files);
        let order: Vec<(i64, Option<u32>, Option<u32>)> = shelved
            .iter()
            .map(|s| (s.file_id, s.season, s.episode))
            .collect();
        assert_eq!(
            order,
            vec![
                (1, Some(3), Some(2)),
                (2, Some(3), Some(5)),
                (3, Some(3), Some(10)),
                (9, None, None),
            ]
        );
        assert_eq!(shelved[1].title.as_deref(), Some("Part 5 Case Files"));
    }

    #[test]
    fn a_number_past_the_catalogues_season_is_kept_as_written() {
        let dir = "E:/Anime/Space Dandy (2014) Season 1 S01 (1080p BluRay x265 MOREVA)";
        let shelved = in_order(&[(
            7,
            format!("{dir}/Space Dandy (2014) - S01E14 - I Can't Be the Only One, Baby (1080p BluRay x265 MOREVA).mkv"),
        )]);
        assert_eq!((shelved[0].season, shelved[0].episode), (Some(1), Some(14)));
    }
}
