//! The fields the corpus does not label: languages, edition, flags, audio, dates, episode
//! titles, the extension.

use filename_parser::{comparable, parse};

#[test]
fn languages_count_only_after_the_title() {
    let p = parse("Amores.Perros.2000.SPANISH.ENG.1080p.BluRay.x264-GROUP.mkv");
    assert_eq!(p.languages, vec!["es", "en"]);
    let french = parse("The.French.Connection.1971.1080p.BluRay.x264-GROUP.mkv");
    assert!(french.languages.is_empty(), "French is in the title here");
    assert!(parse("Film.2010.MULTi.1080p.mkv").multi_language);
}

#[test]
fn edition_flags_and_audio_are_normalised() {
    let p =
        parse("Film.2010.Directors.Cut.REPACK.2160p.UHD.BluRay.HDR10+.DTS-HD.MA.7.1.x265-GROUP");
    assert_eq!(p.edition, vec!["Director's Cut"]);
    assert_eq!(p.flags, vec!["REPACK", "HDR10+"]);
    assert_eq!(p.audio, Some("DTS-HD MA"));
    assert_eq!(p.channels.as_deref(), Some("7.1"));

    let atmos = parse("Film.2010.2160p.WEB-DL.DDP5.1.Atmos.H.265-GROUP");
    assert_eq!(atmos.audio, Some("E-AC-3"));
    assert_eq!(atmos.channels.as_deref(), Some("5.1"));
    assert!(atmos.flags.contains(&"Atmos"));
}

#[test]
fn a_daily_show_has_a_date_and_a_title_for_the_day() {
    let p = parse("The.Show.2024.03.05.Guest.Name.720p.WEB.h264-GROUP.mkv");
    assert_eq!(p.title, "The Show");
    assert_eq!(p.date, Some((2024, 3, 5)));
    assert_eq!(p.year, None, "the date's year is not a release year");
    assert_eq!(p.episode_title.as_deref(), Some("Guest Name"));
    assert!(p.is_episode());
}

#[test]
fn an_episode_title_stops_at_the_next_tag_or_the_group() {
    let p = parse("Show.S05E14.The.Long.Night.720p.HDTV.x264-GROUP.mkv");
    assert_eq!(p.episode_title.as_deref(), Some("The Long Night"));
    let plain = parse("Show (2005) - 1x01 - Rose.avi");
    assert_eq!(plain.episode_title.as_deref(), Some("Rose"));
    assert_eq!(
        plain.group, None,
        "after an episode, ' - Rose' is a title, not a group"
    );
}

#[test]
fn the_extension_is_only_a_known_container() {
    assert_eq!(parse("Film.2010.mkv").extension.as_deref(), Some("mkv"));
    assert_eq!(parse("Film.2010.720p").extension, None);
    let not_ext = parse("Mr.Robot.S01E01.eps1.0_hellofriend.mov.720p.WEB-DL.x264-GROUP.mkv");
    assert_eq!(not_ext.extension.as_deref(), Some("mkv"));
    assert_eq!(not_ext.title, "Mr Robot");
}

#[test]
fn a_path_is_read_by_its_last_part() {
    let p = parse(r"D:\Films\Heat (1995)\Heat.1995.1080p.BluRay.x264-GROUP.mkv");
    assert_eq!((p.title.as_str(), p.year), ("Heat", Some(1995)));
}

#[test]
fn titles_compare_without_case_accents_or_punctuation() {
    assert_eq!(comparable("Amélie"), comparable("amelie"));
    assert_eq!(
        comparable("Spider-Man: Into the Spider-Verse"),
        "spider man into the spider verse"
    );
    assert_eq!(
        comparable("Schindler's List"),
        comparable("Schindlers List")
    );
    assert_eq!(comparable("Fast & Furious"), "fast and furious");
}
