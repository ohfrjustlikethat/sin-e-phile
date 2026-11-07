//! `cargo run -p filename-parser --example parse -- "Some.Film.2010.1080p.BluRay.x264-Group.mkv"`
//! prints what the name says, one field per line. Several names may be given.

fn main() {
    for name in std::env::args().skip(1) {
        let p = filename_parser::parse(&name);
        println!("{name}");
        println!("  title       {:?}", p.title);
        let fields: [(&str, String); 13] = [
            ("year", format!("{:?}", p.year)),
            ("season", format!("{:?}", p.season)),
            ("episodes", format!("{:?}", p.episodes)),
            ("absolute", format!("{:?}", p.absolute_episodes)),
            ("date", format!("{:?}", p.date)),
            ("ep. title", format!("{:?}", p.episode_title)),
            ("resolution", format!("{:?}", p.resolution)),
            ("source", format!("{:?}", p.source.map(|s| s.as_str()))),
            ("codec", format!("{:?}", p.codec.map(|c| c.as_str()))),
            ("audio", format!("{:?} {:?}", p.audio, p.channels)),
            (
                "languages",
                format!("{:?} multi={}", p.languages, p.multi_language),
            ),
            ("edition", format!("{:?} flags={:?}", p.edition, p.flags)),
            ("group", format!("{:?}", p.group)),
        ];
        for (k, v) in fields {
            if !v.starts_with("None")
                && v != "[]"
                && !v.starts_with("[] multi=false")
                && !v.starts_with("[] flags=[]")
                && v != "None None"
            {
                println!("  {k:<11} {v}");
            }
        }
    }
}
