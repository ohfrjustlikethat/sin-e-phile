//! Where a film's subtitles come from — SPEC.md §14 and Phase 10.
//!
//! The chain, in the order it is tried, cheapest and surest first:
//!
//! 1. **Embedded** tracks in the file: perfectly in step by construction, and free. The
//!    player lists them; nothing here touches the network for them.
//! 2. **Files beside the film**, or **in its torrent** ([`files`]).
//! 3. **OpenSubtitles**, matched by the file's hash ([`hash`], [`opensubtitles`]), with the
//!    user's own key, spending a small daily quota only when 1 and 2 found nothing
//!    ([`fetch`]).
//! 4. Other providers, behind [`fetch::Provider`]. None ships: each needs its own look at
//!    whether it is legal to use (§2.1).
//!
//! Whatever is not embedded may then be aligned (`subtitle-align`, Tier 1+).

pub mod fetch;
pub mod files;
pub mod hash;
pub mod opensubtitles;
pub mod plan;

/// Where a subtitle is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origin {
    /// A track inside the media file, by the player's track id.
    Embedded { track_id: i64 },
    /// A file on disk beside the film.
    Sidecar { path: String },
    /// A file in the film's torrent.
    Torrent { file_index: u32, path: String },
    /// Downloaded, and cached under the data folder.
    Downloaded {
        provider: String,
        file_id: String,
        relative_path: String,
    },
}

impl Origin {
    /// The chain's order: lower is tried first.
    fn rank(&self) -> u8 {
        match self {
            Self::Embedded { .. } => 0,
            Self::Sidecar { .. } => 1,
            Self::Torrent { .. } => 2,
            Self::Downloaded { .. } => 3,
        }
    }

    /// Which subtitle this is, for the nudge saved against it (migration 0018). Stable
    /// across runs: a track id, a file name, a provider's file id.
    pub fn key(&self) -> String {
        match self {
            Self::Embedded { track_id } => format!("embedded:{track_id}"),
            Self::Sidecar { path } => {
                format!("file:{}", path.rsplit(['/', '\\']).next().unwrap_or(path))
            }
            Self::Torrent { path, .. } => format!("torrent:{path}"),
            Self::Downloaded {
                provider, file_id, ..
            } => format!("{provider}:{file_id}"),
        }
    }

    /// Embedded tracks are in step by construction; anything else may need aligning.
    pub fn needs_alignment(&self) -> bool {
        !matches!(self, Self::Embedded { .. })
    }
}

/// A subtitle that exists for this film.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub origin: Origin,
    /// BCP 47 primary subtag, if known.
    pub language: Option<String>,
    /// Forced: only the foreign-language lines, not a full subtitle.
    pub forced: bool,
}

/// The subtitle to show, given the viewer's languages in order of preference.
///
/// The best language wins first, then the chain's order: an embedded English track beats
/// a downloaded English file, but a downloaded English file beats an embedded French one
/// when English is preferred. A forced track is chosen only if no full one is in a
/// preferred language. No preference matching means no subtitle: showing a language the
/// viewer did not ask for is worse than showing none.
pub fn choose<'a>(found: &'a [Found], languages: &[String]) -> Option<&'a Found> {
    let preference = |f: &Found| {
        f.language
            .as_deref()
            .and_then(|l| languages.iter().position(|p| p.eq_ignore_ascii_case(l)))
    };
    found
        .iter()
        .filter_map(|f| preference(f).map(|p| (p, f)))
        .min_by_key(|(p, f)| (*p, f.forced, f.origin.rank()))
        .map(|(_, f)| f)
}

/// The order to try sources in: is anything still worth fetching? Only when nothing
/// already here is in a preferred language (§14: spend the quota only where 1 and 2
/// failed).
pub fn worth_fetching(found: &[Found], languages: &[String]) -> bool {
    choose(found, languages).is_none_or(|f| f.forced)
}

/// The UTC date of a Unix time, `YYYY-MM-DD`: the day a provider's quota counts in.
/// (Howard Hinnant's civil-from-days; no date library for one function.)
pub fn utc_day(unix_seconds: i64) -> String {
    let z = unix_seconds.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(origin: Origin, lang: Option<&str>, forced: bool) -> Found {
        Found {
            origin,
            language: lang.map(String::from),
            forced,
        }
    }

    fn langs(l: &[&str]) -> Vec<String> {
        l.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn the_preferred_language_comes_before_the_chain_order() {
        let all = [
            found(Origin::Embedded { track_id: 3 }, Some("fr"), false),
            found(
                Origin::Sidecar {
                    path: "x.en.srt".into(),
                },
                Some("en"),
                false,
            ),
            found(Origin::Embedded { track_id: 4 }, Some("en"), false),
        ];
        assert_eq!(
            choose(&all, &langs(&["en", "fr"])),
            Some(&all[2]),
            "embedded English"
        );
        assert_eq!(choose(&all, &langs(&["fr"])), Some(&all[0]));
        assert_eq!(
            choose(&all, &langs(&["de"])),
            None,
            "not a language they asked for"
        );
    }

    #[test]
    fn a_forced_track_only_when_nothing_full_will_do() {
        let all = [
            found(Origin::Embedded { track_id: 2 }, Some("en"), true),
            found(
                Origin::Sidecar {
                    path: "x.en.srt".into(),
                },
                Some("en"),
                false,
            ),
        ];
        assert_eq!(choose(&all, &langs(&["en"])), Some(&all[1]));
        assert_eq!(choose(&all[..1], &langs(&["en"])), Some(&all[0]));
        assert!(
            worth_fetching(&all[..1], &langs(&["en"])),
            "a forced track is not enough"
        );
        assert!(!worth_fetching(&all, &langs(&["en"])));
    }

    #[test]
    fn the_quota_day_is_the_utc_date() {
        assert_eq!(utc_day(0), "1970-01-01");
        assert_eq!(utc_day(951_782_400), "2000-02-29");
        assert_eq!(utc_day(1_794_009_599), "2026-11-06"); // 23:59:59 UTC
        assert_eq!(utc_day(1_794_009_600), "2026-11-07");
    }

    #[test]
    fn a_nudge_key_survives_the_folder_moving() {
        let a = Origin::Sidecar {
            path: "C:/a/Film.en.srt".into(),
        };
        let b = Origin::Sidecar {
            path: "D:/b/Film.en.srt".into(),
        };
        assert_eq!(a.key(), b.key());
        assert_eq!(Origin::Embedded { track_id: 3 }.key(), "embedded:3");
        assert!(!Origin::Embedded { track_id: 3 }.needs_alignment());
    }
}
