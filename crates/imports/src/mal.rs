//! MyAnimeList's XML export, which AniList also writes: one `<anime>` per title with
//! `series_animedb_id` (MAL's id: an exact match, 7,400 catalogue items carry one),
//! `series_title`, `series_type`, `my_score` (0 is unrated), `my_status` and
//! `my_finish_date` (0000-00-00 when unknown). "Completed" is a watch on the finish date;
//! "Plan to Watch" is the watchlist; watching, on hold and dropped carry only their score.

use quick_xml::events::Event;
use quick_xml::Reader;

use crate::{Entry, ImportError, Kind};

#[derive(Default)]
struct Anime {
    id: Option<i64>,
    title: String,
    kind: String,
    score: Option<f64>,
    status: String,
    finished: Option<String>,
}

fn entry(a: Anime) -> Option<Entry> {
    if a.title.is_empty() && a.id.is_none() {
        return None;
    }
    let status = a.status.to_ascii_lowercase();
    let _ = a.kind;
    Some(Entry {
        title: a.title,
        mal: a.id,
        kind: Some(Kind::Anime),
        rating: a.score.filter(|s| *s > 0.0),
        watches: if status == "completed" || status == "2" {
            vec![a.finished]
        } else {
            Vec::new()
        },
        watchlist: status == "plan to watch" || status == "6",
        ..Default::default()
    })
}

pub fn read(xml: &str) -> Result<Vec<Entry>, ImportError> {
    let mut reader = Reader::from_str(xml);
    let mut out = Vec::new();
    let mut current: Option<Anime> = None;
    // A field's text arrives in pieces (text, CDATA, and each entity on its own), so it
    // is collected and read when the field closes.
    let mut text = String::new();
    loop {
        let event = reader
            .read_event()
            .map_err(|e| ImportError::Read("the XML".into(), e.to_string()))?;
        match event {
            Event::Start(e) => {
                text.clear();
                if e.name().as_ref() == b"anime" {
                    current = Some(Anime::default());
                }
            }
            Event::Text(t) => text.push_str(&t.decode().unwrap_or_default()),
            Event::CData(c) => text.push_str(&String::from_utf8_lossy(&c.into_inner())),
            Event::GeneralRef(r) => {
                let name = String::from_utf8_lossy(r.as_ref()).into_owned();
                text.push_str(match name.as_str() {
                    "amp" => "&",
                    "lt" => "<",
                    "gt" => ">",
                    "quot" => "\"",
                    "apos" => "'",
                    _ => "",
                });
                if let Some(code) = name.strip_prefix('#') {
                    let n = match code.strip_prefix('x') {
                        Some(hex) => u32::from_str_radix(hex, 16).ok(),
                        None => code.parse().ok(),
                    };
                    if let Some(c) = n.and_then(char::from_u32) {
                        text.push(c);
                    }
                }
            }
            Event::End(e) => {
                let name = e.name();
                if name.as_ref() == b"anime" {
                    if let Some(a) = current.take().and_then(entry) {
                        out.push(a);
                    }
                } else if let Some(a) = current.as_mut() {
                    let value = text.trim();
                    match name.as_ref() {
                        b"series_animedb_id" => a.id = value.parse().ok(),
                        b"series_title" => a.title = value.to_string(),
                        b"series_type" => a.kind = value.to_string(),
                        b"my_score" => a.score = value.parse().ok(),
                        b"my_status" => a.status = value.to_string(),
                        b"my_finish_date" => {
                            a.finished = (value.len() == 10 && !value.starts_with("0000"))
                                .then(|| value.to_string())
                        }
                        _ => {}
                    }
                }
                text.clear();
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if out.is_empty() && !xml.contains("<anime>") {
        return Err(ImportError::Unknown("no <anime> entries".into()));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = r#"<?xml version="1.0" encoding="UTF-8" ?>
<myanimelist>
  <myinfo><user_name>someone</user_name></myinfo>
  <anime>
    <series_animedb_id>19</series_animedb_id>
    <series_title><![CDATA[Monster]]></series_title>
    <series_type>TV</series_type>
    <my_finish_date>2021-06-30</my_finish_date>
    <my_score>10</my_score>
    <my_status>Completed</my_status>
  </anime>
  <anime>
    <series_animedb_id>164</series_animedb_id>
    <series_title>Mononoke Hime &amp; friends</series_title>
    <series_type>Movie</series_type>
    <my_finish_date>0000-00-00</my_finish_date>
    <my_score>0</my_score>
    <my_status>Plan to Watch</my_status>
  </anime>
</myanimelist>"#;

    #[test]
    fn completed_is_a_dated_watch_and_plan_to_watch_the_watchlist() {
        let e = read(XML).unwrap();
        assert_eq!(e.len(), 2);
        assert_eq!((e[0].mal, e[0].title.as_str()), (Some(19), "Monster"));
        assert_eq!(e[0].rating, Some(10.0));
        assert_eq!(e[0].watches, vec![Some("2021-06-30".to_string())]);
        assert!(!e[0].watchlist);
        assert_eq!(e[1].title, "Mononoke Hime & friends");
        assert_eq!((e[1].rating, e[1].watchlist), (None, true));
        assert!(e[1].watches.is_empty());
    }
}
