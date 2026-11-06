//! Subtitle timings: read them from SRT, WebVTT and ASS/SSA, and write new ones back
//! **in place**.
//!
//! Only the timestamps are touched. Everything else — text, styling, ASS override
//! tags, positioning, comments, line endings — comes back byte for byte, so an ASS
//! file keeps its typesetting after it has been aligned.

/// The formats the aligner understands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Srt,
    Vtt,
    /// ASS and SSA: the `Dialogue:` lines carry the timings.
    Ass,
}

/// One subtitle's span, in milliseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cue {
    pub start: i64,
    pub end: i64,
}

/// Which format `text` is, from its content rather than a file name.
pub fn detect(text: &str) -> Option<Format> {
    let head = text.trim_start_matches('\u{feff}').trim_start();
    if head.starts_with("WEBVTT") {
        return Some(Format::Vtt);
    }
    if head.starts_with("[Script Info]") || text.contains("\nDialogue:") {
        return Some(Format::Ass);
    }
    text.lines()
        .any(|l| l.contains("-->") && arrow(l, ',').is_some())
        .then_some(Format::Srt)
}

/// Every cue's span, in file order.
pub fn cues(text: &str, format: Format) -> Vec<Cue> {
    let mut out = Vec::new();
    retime_with(text, format, |start, end| {
        out.push(Cue { start, end });
        (start, end)
    });
    out
}

/// The same file with every time `t` replaced by `f(t)`, clamped at zero.
pub fn retime(text: &str, format: Format, f: impl Fn(i64) -> i64) -> String {
    retime_with(text, format, |s, e| (f(s).max(0), f(e).max(0)))
}

fn retime_with(text: &str, format: Format, mut f: impl FnMut(i64, i64) -> (i64, i64)) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        let (body, ending) = split_ending(line);
        let rewritten = match format {
            Format::Srt => timing_line(body, ',', &mut f),
            Format::Vtt => timing_line(body, '.', &mut f),
            Format::Ass => dialogue_line(body, &mut f),
        };
        out.push_str(rewritten.as_deref().unwrap_or(body));
        out.push_str(ending);
    }
    out
}

fn split_ending(line: &str) -> (&str, &str) {
    let body = line.trim_end_matches(['\r', '\n']);
    (body, &line[body.len()..])
}

/// `00:01:02,345 --> 00:01:04,567` and anything after (WebVTT cue settings).
fn arrow(line: &str, sep: char) -> Option<(i64, i64, &str, bool)> {
    let (left, right) = line.split_once("-->")?;
    let left = left.trim();
    let right = right.trim_start();
    let (right_time, rest) = match right.find(char::is_whitespace) {
        Some(i) => (&right[..i], &right[i..]),
        None => (right, ""),
    };
    let start = clock(left, sep)?;
    let end = clock(right_time, sep)?;
    let hours = left.matches(':').count() == 2;
    Some((start, end, rest, hours))
}

fn timing_line(
    body: &str,
    sep: char,
    f: &mut impl FnMut(i64, i64) -> (i64, i64),
) -> Option<String> {
    let (start, end, rest, hours) = arrow(body, sep)?;
    let (s, e) = f(start, end);
    // SRT always writes hours; WebVTT only if the file did, or must.
    let long = sep == ',' || hours || s >= 3_600_000 || e >= 3_600_000;
    Some(format!(
        "{} --> {}{rest}",
        write_clock(s, sep, long),
        write_clock(e, sep, long)
    ))
}

/// `Dialogue: 0,0:01:02.34,0:01:04.56,Default,,0,0,0,,Text`
fn dialogue_line(body: &str, f: &mut impl FnMut(i64, i64) -> (i64, i64)) -> Option<String> {
    let rest = body.strip_prefix("Dialogue:")?;
    let mut fields = rest.splitn(4, ',');
    let layer = fields.next()?;
    let start = ass_clock(fields.next()?.trim())?;
    let end = ass_clock(fields.next()?.trim())?;
    let tail = fields.next().unwrap_or("");
    let (s, e) = f(start, end);
    Some(format!(
        "Dialogue:{layer},{},{},{tail}",
        write_ass_clock(s),
        write_ass_clock(e)
    ))
}

/// `HH:MM:SS,mmm`, `MM:SS.mmm` and the like, to milliseconds.
fn clock(s: &str, sep: char) -> Option<i64> {
    let (whole, frac) = s.split_once(sep)?;
    if frac.is_empty() || frac.len() > 3 || !frac.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let ms: i64 = format!("{frac:0<3}").parse().ok()?;
    let mut secs = 0i64;
    let parts: Vec<&str> = whole.split(':').collect();
    if !(2..=3).contains(&parts.len()) {
        return None;
    }
    for p in parts {
        if p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        secs = secs * 60 + p.parse::<i64>().ok()?;
    }
    Some(secs * 1000 + ms)
}

fn write_clock(ms: i64, sep: char, hours: bool) -> String {
    let (h, m, s, r) = parts(ms);
    if hours {
        format!("{h:02}:{m:02}:{s:02}{sep}{r:03}")
    } else {
        format!("{:02}:{s:02}{sep}{r:03}", h * 60 + m)
    }
}

/// ASS: `H:MM:SS.cc`, centiseconds.
fn ass_clock(s: &str) -> Option<i64> {
    let (whole, cs) = s.split_once('.')?;
    let parts: Vec<i64> = whole
        .split(':')
        .map(|p| p.parse().ok())
        .collect::<Option<_>>()?;
    let [h, m, sec] = parts[..] else {
        return None;
    };
    let cs: i64 = format!("{cs:0<2}").get(..2)?.parse().ok()?;
    Some(((h * 60 + m) * 60 + sec) * 1000 + cs * 10)
}

fn write_ass_clock(ms: i64) -> String {
    let (h, m, s, r) = parts(ms);
    format!("{h}:{m:02}:{s:02}.{:02}", r / 10)
}

fn parts(ms: i64) -> (i64, i64, i64, i64) {
    let ms = ms.max(0);
    (ms / 3_600_000, ms / 60_000 % 60, ms / 1000 % 60, ms % 1000)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRT: &str = "1\r\n00:00:01,500 --> 00:00:03,000\r\n<i>Hello</i>\r\n\r\n2\r\n00:01:02,345 --> 01:00:04,005\r\nThere\r\n";
    const VTT: &str = "WEBVTT\n\n00:01.500 --> 00:03.000 align:start\nHello\n";
    const ASS: &str = "[Script Info]\nTitle: x\n\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\nDialogue: 0,0:00:01.50,0:00:03.00,Default,,0,0,0,,{\\an8\\pos(320,50)}Hello, world\n";

    #[test]
    fn formats_are_told_apart_by_content() {
        assert_eq!(detect(SRT), Some(Format::Srt));
        assert_eq!(detect(VTT), Some(Format::Vtt));
        assert_eq!(detect(ASS), Some(Format::Ass));
        assert_eq!(detect("just some text"), None);
    }

    #[test]
    fn timings_are_read_from_every_format() {
        assert_eq!(
            cues(SRT, Format::Srt),
            [
                Cue {
                    start: 1500,
                    end: 3000
                },
                Cue {
                    start: 62_345,
                    end: 3_604_005
                }
            ]
        );
        assert_eq!(
            cues(VTT, Format::Vtt),
            [Cue {
                start: 1500,
                end: 3000
            }]
        );
        assert_eq!(
            cues(ASS, Format::Ass),
            [Cue {
                start: 1500,
                end: 3000
            }]
        );
    }

    #[test]
    fn retiming_touches_only_the_timestamps() {
        let srt = retime(SRT, Format::Srt, |t| t + 250);
        assert_eq!(
            srt,
            "1\r\n00:00:01,750 --> 00:00:03,250\r\n<i>Hello</i>\r\n\r\n2\r\n00:01:02,595 --> 01:00:04,255\r\nThere\r\n"
        );
        // WebVTT cue settings survive; so does ASS typesetting, commas in the text included.
        assert_eq!(
            retime(VTT, Format::Vtt, |t| t * 2),
            "WEBVTT\n\n00:03.000 --> 00:06.000 align:start\nHello\n"
        );
        let ass = retime(ASS, Format::Ass, |t| t + 1000);
        assert!(ass.ends_with(
            "Dialogue: 0,0:00:02.50,0:00:04.00,Default,,0,0,0,,{\\an8\\pos(320,50)}Hello, world\n"
        ));
        assert!(ass.starts_with("[Script Info]\nTitle: x\n"));
    }

    #[test]
    fn a_shift_before_zero_stops_at_zero() {
        let srt = retime(SRT, Format::Srt, |t| t - 2000);
        assert!(srt.contains("00:00:00,000 --> 00:00:01,000"), "{srt}");
    }

    #[test]
    fn malformed_lines_are_left_alone() {
        let text = "1\n00:00:01,5x0 --> 00:00:03,000\nHi\n";
        assert_eq!(cues(text, Format::Srt), []);
        assert_eq!(retime(text, Format::Srt, |t| t + 1), text);
    }
}
