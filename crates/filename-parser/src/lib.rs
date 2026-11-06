#![doc = include_str!("../README.md")]

mod tags;

use tags::{Found, Kind};

pub use tags::{Codec, Source};

/// What a filename says about itself. Every field is what the NAME claims, not what the
/// file is: a library checks it against the file's own evidence before believing it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Parsed {
    /// The title as written, cleaned of separators: "The Grand Budapest Hotel".
    pub title: String,
    pub year: Option<u16>,
    pub season: Option<u32>,
    /// Episodes within `season`; more than one for a multi-episode file.
    pub episodes: Vec<u32>,
    /// Episode numbers given WITHOUT a season ("Title - 059", "Show E12"), counted from
    /// wherever the file's own numbering starts. Whether that is from the series' first
    /// episode or the season's is for the library to decide (ADR-0031).
    pub absolute_episodes: Vec<u32>,
    /// A daily show's air date: (year, month, day).
    pub date: Option<(u16, u8, u8)>,
    pub episode_title: Option<String>,
    /// Vertical lines: 2160, 1080, 720 ...
    pub resolution: Option<u16>,
    pub source: Option<Source>,
    pub codec: Option<Codec>,
    /// The audio codec, normalised: "AAC", "DTS-HD MA", "E-AC-3" ...
    pub audio: Option<&'static str>,
    /// "2.0", "5.1", "7.1".
    pub channels: Option<String>,
    /// ISO 639-1 codes the name declares ("FRENCH", "ITA" ...), in order.
    pub languages: Vec<&'static str>,
    /// MULTi, DUAL: more than one audio language, unnamed.
    pub multi_language: bool,
    /// "Extended", "Director's Cut", "Remastered" ...
    pub edition: Vec<&'static str>,
    /// REPACK, PROPER, HDR, 10bit, Atmos ...
    pub flags: Vec<&'static str>,
    /// The release group, from a leading `[Group]` or a trailing `-Group`. Recognised by
    /// where it sits, never by a list of names (SPEC.md §2.1).
    pub group: Option<String>,
    /// The file extension, lower case, when it is a video container.
    pub extension: Option<String>,
}

impl Parsed {
    /// Whether the name describes an episode rather than a film.
    pub fn is_episode(&self) -> bool {
        self.season.is_some()
            || !self.episodes.is_empty()
            || !self.absolute_episodes.is_empty()
            || self.date.is_some()
    }
}

const VIDEO_EXTENSIONS: &[&str] = &[
    "mkv", "mp4", "avi", "m4v", "mov", "wmv", "mpg", "mpeg", "ts", "m2ts", "webm", "flv", "ogv",
    "3gp", "vob", "iso", "divx", "rmvb", "ogm",
];

/// Read a filename. A path is fine: only its last component is read.
pub fn parse(input: &str) -> Parsed {
    let mut out = Parsed::default();
    let name = input.rsplit(['/', '\\']).next().unwrap_or(input).trim();

    // The extension, when it is a container we know (a title can contain a dot).
    let mut s = name;
    if let Some(dot) = s.rfind('.') {
        let ext = s[dot + 1..].to_ascii_lowercase();
        if VIDEO_EXTENSIONS.contains(&ext.as_str()) {
            out.extension = Some(ext);
            s = &s[..dot];
        }
    }

    // A site tag in front ("www.example.org - "), then an anime-style [Group].
    s = tags::strip_site_prefix(s);
    let mut anime = false;
    if let Some(rest) = s.strip_prefix('[') {
        if let Some(close) = rest.find(']') {
            let inner = rest[..close].trim();
            if !inner.is_empty() && !tags::is_tag_text(inner) {
                out.group = Some(inner.to_string());
                anime = true;
                s = rest[close + 1..].trim_start_matches([' ', '_', '.', '-']);
            }
        }
    }

    let mut found = tags::find(s, anime);

    // "Episode 50 (2011)": an episode word opening the name and followed by a year is a
    // film's title, not an episode number.
    let first = found
        .iter()
        .filter(|t| t.is_strong())
        .min_by_key(|t| t.start)
        .cloned();
    if let Some(first) = first {
        let opens = s[..first.start].trim_matches(tags::is_sep).is_empty();
        let next_is_year = found
            .iter()
            .filter(|t| t.is_strong() && t.start > first.start)
            .min_by_key(|t| t.start)
            .is_some_and(|t| matches!(t.kind, Kind::Year(_)));
        let word = matches!(first.kind, Kind::Episode { absolute: true, .. })
            && s[first.start..]
                .chars()
                .next()
                .is_some_and(|c| c.eq_ignore_ascii_case(&'e'));
        if opens && next_is_year && word {
            found.retain(|t| t != &first);
        }
    }

    // The title runs up to the first strong tag (an episode, the year, resolution,
    // source, codec); weak tags (edition, language, audio) right before that are trimmed
    // off it, but a weak word inside a title ("The French Connection") stays.
    let ender = found
        .iter()
        .filter(|t| t.is_strong())
        .min_by_key(|t| t.start);
    let mut title_end = ender.map_or(s.len(), |t| t.start);
    // Not before a year, though: "Charlotte's Web 2006" keeps its Web.
    if !matches!(ender.map(|t| &t.kind), Some(Kind::Year(_))) {
        loop {
            let trimmed = s[..title_end].trim_end_matches(tags::is_sep);
            let at = trimmed.len();
            match found
                .iter()
                .find(|t| !t.is_strong() && t.end == at && t.start < at)
            {
                Some(t) if t.start > 0 => title_end = t.start,
                _ => break,
            }
        }
    }

    for t in &found {
        // A weak tag inside the title is part of the title.
        if !t.is_strong() && t.start < title_end {
            continue;
        }
        apply(&mut out, t);
    }

    // The group at the end: "-Group" after the last tag, else a final "[Group]".
    let last = found.iter().max_by_key(|t| t.end);
    let last_tag_end = last.map_or(0, |t| t.end).max(title_end);
    let technical = last.is_some_and(|t| {
        matches!(
            t.kind,
            Kind::Resolution(_)
                | Kind::Source(_)
                | Kind::Codec(_)
                | Kind::Audio(..)
                | Kind::Channels(_)
        )
    });
    let mut group_at = s.len();
    if out.group.is_none() {
        if let Some((at, g)) = tags::trailing_group(s, last_tag_end, technical) {
            out.group = Some(g);
            group_at = at;
        }
    }

    // Anime without a dash: "[Group] Title 05 [720p]".
    let mut title = clean_title(&s[..title_end]);
    if anime && !out.is_episode() {
        if let Some((rest, n)) = trailing_number(&title) {
            out.absolute_episodes = vec![n];
            title = rest;
        }
    }
    // A bare number: "05", "03 - The Buys". An episode, the title presumably the folder's.
    if !out.is_episode() && out.year.is_none() {
        if let Some((n, rest)) = leading_number(&title) {
            out.absolute_episodes = vec![n];
            if !rest.is_empty() {
                out.episode_title = Some(rest);
            }
            title = String::new();
        }
    }
    out.title = title;

    // The episode's own title sits between its number and the next tag.
    if let Some(ep) = found.iter().find(|t| t.kind.is_episode_marker()) {
        let next = found
            .iter()
            .filter(|t| t.start >= ep.end)
            .map(|t| t.start)
            .min()
            .unwrap_or(s.len())
            .min(group_at);
        if next > ep.end {
            let text = clean_title(&s[ep.end..next]);
            if text.chars().any(char::is_alphabetic) {
                out.episode_title = Some(text);
            }
        }
    }
    out
}

fn apply(out: &mut Parsed, t: &Found) {
    match &t.kind {
        Kind::Episode {
            season,
            episodes,
            absolute,
        } => {
            if let Some(s) = season {
                out.season.get_or_insert(*s);
            }
            if *absolute && out.season.is_none() {
                extend(&mut out.absolute_episodes, episodes);
            } else {
                extend(&mut out.episodes, episodes);
            }
        }
        Kind::Season(n) => {
            out.season.get_or_insert(*n);
            // A season given apart from its episode ("S2 - 05"): the episode is within it.
            if !out.absolute_episodes.is_empty() {
                let moved = std::mem::take(&mut out.absolute_episodes);
                extend(&mut out.episodes, &moved);
            }
        }
        Kind::Year(y) => {
            out.year.get_or_insert(*y);
        }
        Kind::Date(y, m, d) => {
            out.date.get_or_insert((*y, *m, *d));
        }
        Kind::Resolution(r) => {
            out.resolution.get_or_insert(*r);
        }
        Kind::Source(s) => {
            // Several can appear ("BluRay.REMUX"): the more specific one wins.
            if out.source.is_none_or(|old| s.rank() > old.rank()) {
                out.source = Some(*s);
            }
        }
        Kind::Codec(c) => {
            out.codec.get_or_insert(*c);
        }
        Kind::Audio(a, ch) => {
            if *a == "Atmos" {
                push_unique(&mut out.flags, "Atmos");
            } else {
                out.audio.get_or_insert(a);
            }
            if let Some(ch) = ch {
                out.channels.get_or_insert(ch.clone());
            }
        }
        Kind::Channels(ch) => {
            out.channels.get_or_insert(ch.clone());
        }
        Kind::Language(code) => push_unique(&mut out.languages, code),
        Kind::Multi => out.multi_language = true,
        Kind::Edition(e) => push_unique(&mut out.edition, e),
        Kind::Flag(f) => push_unique(&mut out.flags, f),
        Kind::Junk => {}
    }
    // However the season and the numbers were ordered in the name, numbers with a season
    // are episodes of it.
    if out.season.is_some() && !out.absolute_episodes.is_empty() && out.episodes.is_empty() {
        out.episodes = std::mem::take(&mut out.absolute_episodes);
    }
}

fn extend(into: &mut Vec<u32>, from: &[u32]) {
    for n in from {
        if !into.contains(n) {
            into.push(*n);
        }
    }
}

fn push_unique<T: PartialEq + Copy>(into: &mut Vec<T>, v: T) {
    if !into.contains(&v) {
        into.push(v);
    }
}

/// "[Group] Title 05" -> ("Title", 5): a bare number of two to four digits ending an
/// anime-style title. One digit is too often part of the name ("Steins;Gate 0").
fn trailing_number(title: &str) -> Option<(String, u32)> {
    let (rest, last) = title.rsplit_once(' ')?;
    let digits = last.strip_suffix(|c| c == 'v').unwrap_or(last);
    let digits = digits.split('v').next()?;
    if (2..=4).contains(&digits.len()) && digits.bytes().all(|b| b.is_ascii_digit()) {
        let n: u32 = digits.parse().ok()?;
        let rest = rest.trim_end_matches([' ', '-']).to_string();
        if !rest.is_empty() && !tags::is_year(n) {
            return Some((rest, n));
        }
    }
    None
}

/// "05" -> 5; "03 - The Buys" -> (3, "The Buys"). One to three digits.
fn leading_number(title: &str) -> Option<(u32, String)> {
    let (digits, rest) = match title.split_once(" - ") {
        Some((d, r)) => (d.trim(), r.trim()),
        None => (title.trim(), ""),
    };
    if (1..=3).contains(&digits.len()) && digits.bytes().all(|b| b.is_ascii_digit()) {
        return Some((digits.parse().ok()?, rest.to_string()));
    }
    None
}

/// Separators to spaces, acronyms kept ("S.W.A.T."), ends trimmed.
pub fn clean_title(raw: &str) -> String {
    let raw = raw.replace('_', " ");
    let raw = raw.trim_matches(|c: char| matches!(c, ' ' | '.' | '-' | '_'));
    let spaced = if raw.contains(' ') {
        raw.to_string()
    } else {
        // Dotted names: every dot is a separator, except inside an acronym.
        let words: Vec<&str> = raw.split('.').collect();
        let mut out = String::new();
        let mut i = 0;
        while i < words.len() {
            let single = |w: &str| w.chars().count() == 1 && w.chars().all(char::is_alphabetic);
            let mut j = i;
            while j < words.len() && single(words[j]) {
                j += 1;
            }
            if !out.is_empty() {
                out.push(' ');
            }
            if j - i >= 2 {
                for w in &words[i..j] {
                    out.push_str(w);
                    out.push('.');
                }
                i = j;
            } else {
                out.push_str(words[i]);
                i += 1;
            }
        }
        out
    };
    let mut collapsed = String::with_capacity(spaced.len());
    for word in spaced.split_whitespace() {
        if !collapsed.is_empty() {
            collapsed.push(' ');
        }
        collapsed.push_str(word);
    }
    collapsed
        .trim_matches(|c: char| matches!(c, ' ' | '-' | '(' | '[' | '{' | ',' | '~' | '+'))
        .trim_end_matches(" -")
        .trim()
        .to_string()
}

/// A title reduced for comparison: lower case, "&" as "and", punctuation and accents
/// that ASCII can express removed, words single-spaced. "Amélie" and "Amelie" agree;
/// "Spider-Man" and "Spider Man" agree.
pub fn comparable(title: &str) -> String {
    let mut out = String::with_capacity(title.len());
    for c in title.chars().flat_map(char::to_lowercase) {
        let c = fold(c);
        if c == '&' {
            out.push_str(" and ");
        } else if c.is_alphanumeric() {
            out.push(c);
        } else if c == '\'' || c == '’' {
            // "Schindler's" and "Schindlers" agree.
        } else {
            out.push(' ');
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The common Latin accents, folded to ASCII.
fn fold(c: char) -> char {
    match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => 'a',
        'ç' => 'c',
        'è' | 'é' | 'ê' | 'ë' => 'e',
        'ì' | 'í' | 'î' | 'ï' => 'i',
        'ñ' => 'n',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' => 'o',
        'ù' | 'ú' | 'û' | 'ü' => 'u',
        'ý' | 'ÿ' => 'y',
        _ => c,
    }
}
