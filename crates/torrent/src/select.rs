//! Which file in a torrent to play — SPEC.md Phase 7: "pick the episode, ignore the
//! sample and the readme".
//!
//! A pure function over the file list, because the rules are the kind that are easy to
//! state and easy to get wrong: a sample is often the second-largest video and sometimes
//! the only one with a clean name; a season pack holds ten videos of nearly equal size;
//! and "largest file" alone picks a bundled extras disc over the episode asked for.

/// One file as the torrent lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TorrentFile {
    pub index: usize,
    /// The path inside the torrent, `/`-separated.
    pub path: String,
    pub len: u64,
}

/// What the viewer asked for.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Wanted {
    pub season: Option<u32>,
    pub episode: Option<u32>,
}

const VIDEO: &[&str] = &[
    "mkv", "mp4", "m4v", "avi", "mov", "webm", "ts", "m2ts", "mpg", "mpeg", "wmv", "ogv",
];

/// Words that mark a file as not the feature, matched as whole path segments or words.
const NOT_THE_FEATURE: &[&str] = &[
    "sample",
    "trailer",
    "featurette",
    "extras",
    "extra",
    "bonus",
    "behind the scenes",
    "deleted scenes",
    "interview",
    "promo",
    "teaser",
    "making of",
];

pub fn is_video(path: &str) -> bool {
    path.rsplit_once('.')
        .map(|(_, ext)| VIDEO.contains(&ext.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

fn words(path: &str) -> String {
    path.to_ascii_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { ' ' })
        .collect::<String>()
}

fn is_extra(path: &str) -> bool {
    let w = format!(
        " {} ",
        words(path).split_whitespace().collect::<Vec<_>>().join(" ")
    );
    NOT_THE_FEATURE
        .iter()
        .any(|x| w.contains(&format!(" {x} ")))
}

/// S01E02, 1x02, "Season 1 … Episode 2", or an E02 after a season folder.
pub fn episode_of(path: &str) -> Option<(u32, u32)> {
    let w = words(path);
    let tokens: Vec<&str> = w.split_whitespace().collect();
    for t in &tokens {
        // s01e02 (possibly s01e02e03 — the first episode counts)
        if let Some(rest) = t.strip_prefix('s') {
            if let Some((s, e)) = rest.split_once('e') {
                let e: String = e.chars().take_while(|c| c.is_ascii_digit()).collect();
                if let (Ok(s), Ok(e)) = (s.parse(), e.parse()) {
                    return Some((s, e));
                }
            }
        }
        // 1x02
        if let Some((s, e)) = t.split_once('x') {
            if !s.is_empty() && s.len() <= 2 && e.len() >= 2 && e.len() <= 3 {
                if let (Ok(s), Ok(e)) = (s.parse(), e.parse()) {
                    return Some((s, e));
                }
            }
        }
    }
    // "season 1 … episode 2"
    let find = |key: &str| {
        tokens
            .iter()
            .position(|t| *t == key)
            .and_then(|i| tokens.get(i + 1))
            .and_then(|n| n.parse::<u32>().ok())
    };
    match (find("season"), find("episode")) {
        (Some(s), Some(e)) => Some((s, e)),
        _ => None,
    }
}

/// The file to play, or `None` if the torrent holds no video at all.
///
/// 1. Only videos count, and never an extra (sample, trailer, featurette…) unless the
///    torrent holds nothing else.
/// 2. If an episode was asked for, the file naming that episode wins — and if none
///    names it, the answer is `None`, not some other episode.
/// 3. Otherwise the largest remaining video.
pub fn choose(files: &[TorrentFile], wanted: Wanted) -> Option<usize> {
    let videos: Vec<&TorrentFile> = files.iter().filter(|f| is_video(&f.path)).collect();
    let features: Vec<&TorrentFile> = videos
        .iter()
        .copied()
        .filter(|f| !is_extra(&f.path))
        .collect();
    let pool = if features.is_empty() {
        &videos
    } else {
        &features
    };

    if let Some(episode) = wanted.episode {
        return pool
            .iter()
            .filter(|f| match episode_of(&f.path) {
                Some((s, e)) => e == episode && wanted.season.is_none_or(|ws| ws == s),
                None => false,
            })
            .max_by_key(|f| f.len)
            .map(|f| f.index);
    }
    pool.iter().max_by_key(|f| f.len).map(|f| f.index)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout(files: &[(&str, u64)]) -> Vec<TorrentFile> {
        files
            .iter()
            .enumerate()
            .map(|(index, (path, len))| TorrentFile {
                index,
                path: path.to_string(),
                len: *len,
            })
            .collect()
    }

    const MB: u64 = 1 << 20;

    #[test]
    fn a_film_with_a_sample_and_a_readme() {
        let f = layout(&[
            ("Film.Title.1922/readme.txt", 2_000),
            ("Film.Title.1922/Sample/film.title.1922.sample.mkv", 60 * MB),
            ("Film.Title.1922/Film.Title.1922.1080p.mkv", 7_000 * MB),
        ]);
        assert_eq!(choose(&f, Wanted::default()), Some(2));
    }

    #[test]
    fn a_sample_larger_than_a_tiny_feature_still_loses() {
        // Low-bitrate old films can be smaller than a modern sample.
        let f = layout(&[
            ("Old.Film/old.film.sample.mp4", 300 * MB),
            ("Old.Film/old.film.mp4", 250 * MB),
        ]);
        assert_eq!(choose(&f, Wanted::default()), Some(1));
    }

    #[test]
    fn extras_are_ignored_but_a_lone_extra_is_better_than_nothing() {
        let f = layout(&[
            ("Show/Featurettes/Making Of.mkv", 900 * MB),
            ("Show/Show.mkv", 800 * MB),
        ]);
        assert_eq!(choose(&f, Wanted::default()), Some(1));
        let only = layout(&[("Trailer/trailer.mp4", 40 * MB), ("cover.jpg", MB)]);
        assert_eq!(choose(&only, Wanted::default()), Some(0));
    }

    #[test]
    fn the_episode_asked_for_wins_in_a_season_pack() {
        let f = layout(&[
            ("Series.S02/Series.S02E01.mkv", 1_200 * MB),
            ("Series.S02/Series.S02E02.mkv", 1_100 * MB),
            ("Series.S02/Series.S02E03.mkv", 1_300 * MB),
            ("Series.S02/Extras/Series.S02.Bonus.mkv", 2_000 * MB),
        ]);
        let want = |s, e| Wanted {
            season: Some(s),
            episode: Some(e),
        };
        assert_eq!(choose(&f, want(2, 2)), Some(1));
        assert_eq!(choose(&f, want(2, 9)), None, "not some other episode");
        assert_eq!(choose(&f, want(1, 2)), None, "wrong season");
    }

    #[test]
    fn episode_numbering_styles() {
        assert_eq!(episode_of("Show.S01E02.1080p.mkv"), Some((1, 2)));
        assert_eq!(episode_of("show 1x02 title.avi"), Some((1, 2)));
        assert_eq!(
            episode_of("Show/Season 3/Episode 10 - Name.mkv"),
            Some((3, 10))
        );
        assert_eq!(episode_of("Show.S01E02E03.mkv"), Some((1, 2)));
        assert_eq!(
            episode_of("Film.1920x1080.mkv"),
            None,
            "a resolution is not an episode"
        );
        assert_eq!(episode_of("Film.2019.mkv"), None);
    }

    #[test]
    fn no_video_no_answer() {
        assert_eq!(
            choose(&layout(&[("a.txt", 1), ("b.nfo", 2)]), Wanted::default()),
            None
        );
    }
}
