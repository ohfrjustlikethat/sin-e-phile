//! The tags a filename carries, found by pattern with explicit boundaries.
//!
//! Every pattern is matched anywhere, then kept only if a separator (or an end) sits on
//! both sides of it: "x264" counts, "fox264" does not. Overlapping finds are settled by
//! keeping the earliest, then the longest ("WEB-DL" beats "WEB").

use std::sync::OnceLock;

use regex::{Captures, Regex};

/// Where a copy came from, least to most faithful.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Source {
    Cam,
    Telesync,
    Screener,
    Hdtv,
    Dvd,
    HdRip,
    WebRip,
    WebDl,
    BluRay,
    Remux,
}

impl Source {
    pub fn as_str(self) -> &'static str {
        match self {
            Source::Cam => "CAM",
            Source::Telesync => "TS",
            Source::Screener => "SCR",
            Source::Hdtv => "HDTV",
            Source::Dvd => "DVD",
            Source::HdRip => "HDRip",
            Source::WebRip => "WEBRip",
            Source::WebDl => "WEB-DL",
            Source::BluRay => "BluRay",
            Source::Remux => "Remux",
        }
    }

    /// Which wins when a name carries two ("BluRay.REMUX"): the more specific.
    pub(crate) fn rank(self) -> u8 {
        self as u8
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Codec {
    H264,
    H265,
    Av1,
    Vp9,
    Xvid,
    Divx,
    Mpeg2,
    Vc1,
}

impl Codec {
    pub fn as_str(self) -> &'static str {
        match self {
            Codec::H264 => "H.264",
            Codec::H265 => "H.265",
            Codec::Av1 => "AV1",
            Codec::Vp9 => "VP9",
            Codec::Xvid => "XviD",
            Codec::Divx => "DivX",
            Codec::Mpeg2 => "MPEG-2",
            Codec::Vc1 => "VC-1",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Kind {
    /// `absolute`: numbered without a season.
    Episode {
        season: Option<u32>,
        episodes: Vec<u32>,
        absolute: bool,
    },
    Season(u32),
    Year(u16),
    Date(u16, u8, u8),
    Resolution(u16),
    Source(Source),
    Codec(Codec),
    Audio(&'static str, Option<String>),
    Channels(String),
    Language(&'static str),
    Multi,
    Edition(&'static str),
    Flag(&'static str),
    /// A checksum in brackets: not the title, not the group, nothing.
    Junk,
}

impl Kind {
    /// Strong tags end the title; weak ones only count after it.
    pub(crate) fn is_strong(&self) -> bool {
        matches!(
            self,
            Kind::Episode { .. }
                | Kind::Season(_)
                | Kind::Year(_)
                | Kind::Date(..)
                | Kind::Resolution(_)
                | Kind::Source(_)
                | Kind::Codec(_)
        )
    }

    pub(crate) fn is_episode_marker(&self) -> bool {
        matches!(self, Kind::Episode { .. } | Kind::Date(..))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Found {
    pub start: usize,
    pub end: usize,
    pub kind: Kind,
    /// A source that is also an ordinary word ("Web", "Cam", "TS"): it counts after the
    /// title, but never ends one ("Charlotte's Web 2006").
    pub ambiguous: bool,
}

impl Found {
    pub(crate) fn is_strong(&self) -> bool {
        self.kind.is_strong() && !self.ambiguous
    }
}

const AMBIGUOUS_WORDS: &[&str] = &["web", "cam", "ts", "tc", "scr", "r5", "dvd", "bd"];

pub(crate) fn is_sep(c: char) -> bool {
    matches!(
        c,
        ' ' | '.' | '_' | '-' | '[' | ']' | '(' | ')' | '{' | '}' | '+' | ',' | '~'
    )
}

pub(crate) fn is_year(n: u32) -> bool {
    (1890..=2039).contains(&n)
}

fn sep_before(s: &str, at: usize) -> bool {
    s[..at].chars().next_back().is_none_or(is_sep)
}

fn sep_after(s: &str, at: usize) -> bool {
    s[at..].chars().next().is_none_or(is_sep)
}

/// How one pattern turns its match into a tag. `None` declines the match.
type Make = fn(&Captures, &Ctx) -> Option<Kind>;

struct Spec {
    re: Regex,
    /// The anime " - 05" pattern carries its own leading separator.
    check_before: bool,
    make: Make,
}

struct Ctx {
    anime: bool,
}

fn re(pattern: &str) -> Regex {
    Regex::new(&format!("(?i){pattern}")).expect("tag pattern")
}

fn num<T: std::str::FromStr>(c: &Captures, i: usize) -> Option<T> {
    c.get(i)?.as_str().parse().ok()
}

fn specs() -> &'static [Spec] {
    static SPECS: OnceLock<Vec<Spec>> = OnceLock::new();
    SPECS.get_or_init(|| {
        let spec = |pattern: &str, make: Make| Spec {
            re: re(pattern),
            check_before: true,
            make,
        };
        vec![
            // S01E02; its continuation (E03, -E04, -05) is read by `episode_tail`.
            spec(r"s(\d{1,3})[ ._-]?e(\d{1,4})", |c, _| {
                Some(Kind::Episode {
                    season: num(c, 1),
                    episodes: vec![num(c, 2)?],
                    absolute: false,
                })
            }),
            // 1x02
            spec(r"(\d{1,2})x(\d{2,3})", |c, _| {
                Some(Kind::Episode {
                    season: num(c, 1),
                    episodes: vec![num(c, 2)?],
                    absolute: false,
                })
            }),
            // A season on its own: a pack, or anime's "S2 - 05".
            spec(r"(?:s|season[ ._-]?|saison[ ._-]?)(\d{1,2})", |c, _| {
                Some(Kind::Season(num(c, 1)?))
            }),
            // Episode 5, Ep05, E05: numbered without a season.
            spec(r"(?:episode[ ._-]?|ep[ ._-]?|e)(\d{1,4})", |c, _| {
                let whole = c.get(0)?.as_str();
                // A bare "E" needs two digits ("E05"); one is too often a word.
                if whole.len() == 2 && whole.as_bytes()[0].eq_ignore_ascii_case(&b'e') {
                    return None;
                }
                Some(Kind::Episode {
                    season: None,
                    episodes: vec![num(c, 1)?],
                    absolute: true,
                })
            }),
            // Anime: "Title - 05", "Title - 05v2", "Title - 01-02".
            Spec {
                re: re(r"[ _]-[ _](\d{1,4})(?:v\d)?(?:[-~](\d{1,4})(?:v\d)?)?"),
                check_before: false,
                make: |c, ctx| {
                    let first: u32 = num(c, 1)?;
                    if is_year(first) && !ctx.anime {
                        return None;
                    }
                    let mut episodes = vec![first];
                    if let Some(last) = num::<u32>(c, 2) {
                        if last > first && last - first <= 60 {
                            episodes = (first..=last).collect();
                        }
                    }
                    Some(Kind::Episode {
                        season: None,
                        episodes,
                        absolute: true,
                    })
                },
            },
            // A daily show: 2024.03.05
            spec(r"(\d{4})[ ._-](\d{2})[ ._-](\d{2})", |c, _| {
                let (y, m, d): (u16, u8, u8) = (num(c, 1)?, num(c, 2)?, num(c, 3)?);
                (is_year(y.into()) && (1..=12).contains(&m) && (1..=31).contains(&d))
                    .then_some(Kind::Date(y, m, d))
            }),
            spec(r"(1[89]\d{2}|20[0-3]\d)", |c, _| Some(Kind::Year(num(c, 1)?))),
            spec(
                r"(2160|1440|1080|720|576|540|480|360|240)[pi]|(4k|uhd)|\d{3,4}x(\d{3,4})",
                |c, _| {
                    if c.get(2).is_some() {
                        return Some(Kind::Resolution(2160));
                    }
                    Some(Kind::Resolution(num(c, 1).or_else(|| num(c, 3))?))
                },
            ),
            spec(r"(?:bd|uhd)?[ ._-]?remux", |_, _| Some(Kind::Source(Source::Remux))),
            spec(
                r"blu[ ._-]?ray|bd[ ._-]?rip|br[ ._-]?rip|bdmv|bd(?:25|50)?",
                |_, _| Some(Kind::Source(Source::BluRay)),
            ),
            spec(r"web[ ._-]?rip", |_, _| Some(Kind::Source(Source::WebRip))),
            spec(r"web[ ._-]?dl|web", |_, _| Some(Kind::Source(Source::WebDl))),
            spec(r"hd[ ._-]?rip", |_, _| Some(Kind::Source(Source::HdRip))),
            spec(r"hdtv(?:rip)?|pdtv|sdtv|dsr(?:ip)?|tv[ ._-]?rip", |_, _| {
                Some(Kind::Source(Source::Hdtv))
            }),
            spec(r"dvd[ ._-]?scr|screener|scr|r5", |_, _| {
                Some(Kind::Source(Source::Screener))
            }),
            spec(r"dvd[ ._-]?rip|dvd[ ._-]?r|dvd(?:5|9)?", |_, _| {
                Some(Kind::Source(Source::Dvd))
            }),
            spec(r"hd[ ._-]?ts|telesync|ts|hd[ ._-]?tc|telecine|tc", |_, _| {
                Some(Kind::Source(Source::Telesync))
            }),
            spec(r"hd[ ._-]?cam|cam[ ._-]?rip|cam", |_, _| {
                Some(Kind::Source(Source::Cam))
            }),
            spec(
                r"([xh])[ ._]?(264|265)|(avc)|(hevc)|(av1)|(vp9)|(xvid)|(divx)|(mpeg[ ._-]?2)|(vc[ ._-]?1)",
                |c, _| {
                    let codec = if let Some(n) = c.get(2) {
                        if n.as_str() == "264" {
                            Codec::H264
                        } else {
                            Codec::H265
                        }
                    } else if c.get(3).is_some() {
                        Codec::H264
                    } else if c.get(4).is_some() {
                        Codec::H265
                    } else if c.get(5).is_some() {
                        Codec::Av1
                    } else if c.get(6).is_some() {
                        Codec::Vp9
                    } else if c.get(7).is_some() {
                        Codec::Xvid
                    } else if c.get(8).is_some() {
                        Codec::Divx
                    } else if c.get(9).is_some() {
                        Codec::Mpeg2
                    } else {
                        Codec::Vc1
                    };
                    Some(Kind::Codec(codec))
                },
            ),
            spec(
                r"(dts[ ._-]?hd[ ._-]?ma|dts[ ._-]?hd|dts[ ._-]?x|dts|truehd|atmos|e[ ._-]?ac[ ._-]?3|ddp|dd\+|dd|ac[ ._-]?3|aac|flac|mp3|opus|l?pcm)(?:[ ._-]?([1-7])[ ._]([01]))?",
                |c, _| {
                    let raw = c.get(1)?.as_str().to_ascii_lowercase();
                    let squashed: String = raw.chars().filter(char::is_ascii_alphanumeric).collect();
                    let audio = match squashed.as_str() {
                        "dtshdma" => "DTS-HD MA",
                        "dtshd" => "DTS-HD",
                        "dtsx" => "DTS:X",
                        "dts" => "DTS",
                        "truehd" => "TrueHD",
                        "atmos" => "Atmos",
                        "eac3" | "ddp" => "E-AC-3",
                        "dd" if raw.ends_with('+') => "E-AC-3",
                        "dd" | "ac3" => "AC-3",
                        "aac" => "AAC",
                        "flac" => "FLAC",
                        "mp3" => "MP3",
                        "opus" => "Opus",
                        _ => "PCM",
                    };
                    let channels = match (c.get(2), c.get(3)) {
                        (Some(a), Some(b)) => Some(format!("{}.{}", a.as_str(), b.as_str())),
                        _ => None,
                    };
                    Some(Kind::Audio(audio, channels))
                },
            ),
            spec(r"([2-7])\.([01])(?:ch)?", |c, _| {
                Some(Kind::Channels(format!(
                    "{}.{}",
                    c.get(1)?.as_str(),
                    c.get(2)?.as_str()
                )))
            }),
            spec(
                r"hdr10\+|hdr10|hdr|dovi|dolby[ ._]?vision|dv|10[ ._-]?bit|8[ ._-]?bit|hi10p?|sdr|3d|h?sbs",
                |c, _| {
                    let raw: String = c
                        .get(0)?
                        .as_str()
                        .to_ascii_lowercase()
                        .chars()
                        .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '+')
                        .collect();
                    let flag = match raw.as_str() {
                        "hdr10+" => "HDR10+",
                        "hdr10" | "hdr" => "HDR",
                        "dovi" | "dolbyvision" | "dv" => "Dolby Vision",
                        "10bit" | "hi10" | "hi10p" => "10bit",
                        "8bit" => "8bit",
                        "sdr" => "SDR",
                        _ => "3D",
                    };
                    Some(Kind::Flag(flag))
                },
            ),
            spec(
                r"extended(?:[ ._-](?:cut|edition))?|director'?s[ ._-]cut|unrated|uncut|remastered|imax|theatrical(?:[ ._-]cut)?|final[ ._-]cut|special[ ._-]edition|criterion|open[ ._-]matte",
                |c, _| {
                    let raw = c.get(0)?.as_str().to_ascii_lowercase();
                    let edition = if raw.starts_with("extended") {
                        "Extended"
                    } else if raw.starts_with("director") {
                        "Director's Cut"
                    } else if raw == "unrated" {
                        "Unrated"
                    } else if raw == "uncut" {
                        "Uncut"
                    } else if raw == "remastered" {
                        "Remastered"
                    } else if raw == "imax" {
                        "IMAX"
                    } else if raw.starts_with("theatrical") {
                        "Theatrical"
                    } else if raw.starts_with("final") {
                        "Final Cut"
                    } else if raw.starts_with("special") {
                        "Special Edition"
                    } else if raw == "criterion" {
                        "Criterion"
                    } else {
                        "Open Matte"
                    };
                    Some(Kind::Edition(edition))
                },
            ),
            spec(
                r"repack|proper|rerip|internal|limited|complete|read[ ._-]?nfo|dubbed|subbed|hardsubs?|hc",
                |c, _| {
                    let raw = c.get(0)?.as_str().to_ascii_lowercase();
                    let flag = match raw.as_str() {
                        "repack" => "REPACK",
                        "proper" => "PROPER",
                        "rerip" => "RERIP",
                        "internal" => "INTERNAL",
                        "limited" => "LIMITED",
                        "complete" => "COMPLETE",
                        "dubbed" => "Dubbed",
                        "subbed" => "Subbed",
                        "hc" | "hardsub" | "hardsubs" => "Hardsub",
                        _ => "READNFO",
                    };
                    Some(Kind::Flag(flag))
                },
            ),
            spec(r"multi(?:[ ._-]?audio)?|dual(?:[ ._-]?audio)?", |_, _| {
                Some(Kind::Multi)
            }),
            spec(LANGUAGE_PATTERN, |c, _| {
                Some(Kind::Language(language(&c.get(0)?.as_str().to_ascii_lowercase())?))
            }),
        ]
    })
}

const LANGUAGE_PATTERN: &str = r"english|eng|truefrench|french|vostfr|vff|vfq|fre|fra|german|ger|deu|italian|ita|spanish|castellano|latino|spa|esp|portuguese|por|russian|rus|japanese|jpn|jap|korean|kor|chinese|mandarin|cantonese|chi|chs|cht|hindi|hin|tamil|telugu|dutch|swedish|swe|danish|norwegian|finnish|polish|pol|turkish|tur|arabic|ara|hebrew|heb|greek|czech|hungarian|thai|vietnamese|indonesian|ukrainian|ukr|persian|farsi";

fn language(word: &str) -> Option<&'static str> {
    Some(match word {
        "english" | "eng" => "en",
        "truefrench" | "french" | "vostfr" | "vff" | "vfq" | "fre" | "fra" => "fr",
        "german" | "ger" | "deu" => "de",
        "italian" | "ita" => "it",
        "spanish" | "castellano" | "latino" | "spa" | "esp" => "es",
        "portuguese" | "por" => "pt",
        "russian" | "rus" => "ru",
        "japanese" | "jpn" | "jap" => "ja",
        "korean" | "kor" => "ko",
        "chinese" | "mandarin" | "cantonese" | "chi" | "chs" | "cht" => "zh",
        "hindi" | "hin" => "hi",
        "tamil" => "ta",
        "telugu" => "te",
        "dutch" => "nl",
        "swedish" | "swe" => "sv",
        "danish" => "da",
        "norwegian" => "no",
        "finnish" => "fi",
        "polish" | "pol" => "pl",
        "turkish" | "tur" => "tr",
        "arabic" | "ara" => "ar",
        "hebrew" | "heb" => "he",
        "greek" => "el",
        "czech" => "cs",
        "hungarian" => "hu",
        "thai" => "th",
        "vietnamese" => "vi",
        "indonesian" => "id",
        "ukrainian" | "ukr" => "uk",
        "persian" | "farsi" => "fa",
        _ => return None,
    })
}

/// Every tag in `s`, overlaps settled, and the year chosen among the candidates.
pub(crate) fn find(s: &str, anime: bool) -> Vec<Found> {
    let ctx = Ctx { anime };
    let mut all = Vec::new();
    for spec in specs() {
        let mut at = 0;
        while at <= s.len() {
            let Some(c) = spec.re.captures_at(s, at) else {
                break;
            };
            let m = c.get(0).expect("whole match");
            let before = !spec.check_before || sep_before(s, m.start());
            if before && !m.as_str().is_empty() {
                if let Some(mut kind) = (spec.make)(&c, &ctx) {
                    // An episode's continuation ("E01E02") is read before the boundary is
                    // checked, or the first number alone would fail it.
                    let end = match &mut kind {
                        Kind::Episode { episodes, .. } => episode_tail(s, m.end(), episodes),
                        _ => m.end(),
                    };
                    if sep_after(s, end) {
                        let word = m.as_str().to_ascii_lowercase();
                        let ambiguous = matches!(kind, Kind::Source(_))
                            && AMBIGUOUS_WORDS.contains(&word.as_str());
                        all.push(Found {
                            start: m.start(),
                            end,
                            kind,
                            ambiguous,
                        });
                    }
                }
            }
            // Step one character on, not past the match: a rejected find must not hide a
            // good one that starts inside it.
            at = m.start() + s[m.start()..].chars().next().map_or(1, char::len_utf8);
        }
    }
    for t in all.iter_mut() {
        // A checksum in brackets is junk, whatever it otherwise looked like.
        let inner = &s[t.start..t.end];
        let bracketed = s[..t.start].ends_with('[') && s[t.end..].starts_with(']');
        if bracketed && inner.len() == 8 && inner.chars().all(|c| c.is_ascii_hexdigit()) {
            t.kind = Kind::Junk;
        }
    }
    junk_checksums(s, &mut all);

    // Earliest first, then longest; drop whatever overlaps something already kept.
    all.sort_by_key(|t| (t.start, std::cmp::Reverse(t.end)));
    let mut kept: Vec<Found> = Vec::new();
    for t in all {
        if kept.iter().all(|k| t.end <= k.start || t.start >= k.end) {
            kept.push(t);
        }
    }
    choose_year(s, &mut kept);
    kept
}

/// "[ABCD1234]": an 8-hex checksum that no pattern matched.
fn junk_checksums(s: &str, all: &mut Vec<Found>) {
    static CRC: OnceLock<Regex> = OnceLock::new();
    let crc = CRC.get_or_init(|| re(r"[\[(]([0-9a-f]{8})[\])]"));
    for c in crc.captures_iter(s) {
        let m = c.get(1).expect("group");
        all.push(Found {
            start: m.start(),
            end: m.end(),
            kind: Kind::Junk,
            ambiguous: false,
        });
    }
}

/// More episodes after the first: "E03", "-E04", "-05", "-S01E06" (a range when a dash
/// joins them). Returns where the episode tag now ends.
fn episode_tail(s: &str, mut end: usize, episodes: &mut Vec<u32>) -> usize {
    static TAIL: OnceLock<Regex> = OnceLock::new();
    let tail = TAIL.get_or_init(|| re(r"^(?:([ ._]?-[ ._]?)(?:s\d{1,3})?e?|e)(\d{1,4})"));
    while let Some(c) = tail.captures(&s[end..]) {
        let m = c.get(0).expect("whole");
        let after = end + m.end();
        // The next piece may itself continue ("E01E02E03"), so an 'e' also bounds it.
        let next = s[after..].chars().next();
        if !(next.is_none_or(is_sep) || next.is_some_and(|c| c.eq_ignore_ascii_case(&'e'))) {
            break;
        }
        let Some(n) = num::<u32>(&c, 2) else { break };
        let last = *episodes.last().unwrap_or(&0);
        if c.get(1).is_some() && n > last && n - last <= 60 {
            episodes.extend(last + 1..=n);
        } else if n > last && n - last <= 60 {
            episodes.push(n);
        } else {
            break;
        }
        end = after;
    }
    end
}

/// Of the year-shaped numbers, which is the year? Never one that opens the name ("1917",
/// "2001: A Space Odyssey ..."); a bracketed one if there is one; otherwise the last one
/// before the other strong tags begin ("Wonder Woman 1984 2020" is 2020). The rest are
/// title text and are removed.
fn choose_year(s: &str, kept: &mut Vec<Found>) {
    let first_other = kept
        .iter()
        .filter(|t| t.is_strong() && !matches!(t.kind, Kind::Year(_)))
        .map(|t| t.start)
        .min()
        .unwrap_or(usize::MAX);
    let leading = |t: &Found| s[..t.start].trim_matches(is_sep).is_empty();
    let candidates: Vec<usize> = kept
        .iter()
        .enumerate()
        .filter(|(_, t)| matches!(t.kind, Kind::Year(_)) && !leading(t))
        .map(|(i, _)| i)
        .collect();
    let bracketed =
        |t: &Found| s[..t.start].ends_with(['(', '[']) || s[t.end..].starts_with([')', ']']);
    let chosen = candidates
        .iter()
        .rev()
        .find(|&&i| bracketed(&kept[i]))
        .or_else(|| {
            candidates
                .iter()
                .rev()
                .find(|&&i| kept[i].start < first_other)
        })
        .or_else(|| candidates.last())
        .copied();
    let mut i = 0;
    kept.retain(|t| {
        let keep = !matches!(t.kind, Kind::Year(_)) || Some(i) == chosen;
        i += 1;
        keep
    });
}

/// "www.example.org - Title ..." or "[ www.example.org ] Title ...": a site's own tag, not
/// the title. Recognised by shape, never by name.
pub(crate) fn strip_site_prefix(s: &str) -> &str {
    static SITE: OnceLock<Regex> = OnceLock::new();
    let site = SITE.get_or_init(|| {
        re(r"^\[?\s*(?:www\.)[a-z0-9-]+(?:\.[a-z0-9-]+)*\.[a-z]{2,6}\s*\]?[\s_.\-–]*")
    });
    match site.find(s) {
        Some(m) => &s[m.end()..],
        None => s,
    }
}

/// Whether bracketed text is a tag ("1080p", "BD 1080p") rather than a group's name.
pub(crate) fn is_tag_text(inner: &str) -> bool {
    let found = find(inner, false);
    let covered: usize = found.iter().map(|t| t.end - t.start).sum();
    let letters = inner.chars().filter(|c| !is_sep(*c)).count();
    !found.is_empty() && covered * 2 >= letters
}

/// The release group at the end: "...x264-Group", or a final "[Group]" that is no tag.
/// Returns where it starts and the name.
///
/// `spaced` allows " - Group" too, which is only safe after a technical tag: after an
/// episode number, " - Rose" is the episode's title.
pub(crate) fn trailing_group(s: &str, after: usize, spaced: bool) -> Option<(usize, String)> {
    static DASH: OnceLock<Regex> = OnceLock::new();
    static SPACED: OnceLock<Regex> = OnceLock::new();
    static BRACKET: OnceLock<Regex> = OnceLock::new();
    let dash = if spaced {
        SPACED.get_or_init(|| re(r"\s*-\s*([a-z0-9][a-z0-9_@]*)\s*(?:\[[^\]]*\])?\s*$"))
    } else {
        DASH.get_or_init(|| re(r"-([a-z0-9][a-z0-9_@]*)\s*(?:\[[^\]]*\])?\s*$"))
    };
    let bracket = BRACKET.get_or_init(|| re(r"\[([^\]\[]+)\]\s*$"));
    if let Some(c) = dash.captures(s) {
        let m = c.get(0).expect("whole");
        let name = c.get(1).expect("name").as_str();
        if m.start() >= after && !is_tag_text(name) && !name.chars().all(|c| c.is_ascii_digit()) {
            return Some((m.start(), name.to_string()));
        }
    }
    if let Some(c) = bracket.captures(s) {
        let m = c.get(0).expect("whole");
        let name = c.get(1).expect("name").as_str().trim();
        let checksum = name.len() == 8 && name.chars().all(|c| c.is_ascii_hexdigit());
        if m.start() >= after && !name.is_empty() && !checksum && !is_tag_text(name) {
            return Some((m.start(), name.to_string()));
        }
    }
    None
}
