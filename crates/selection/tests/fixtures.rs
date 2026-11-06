//! Candidate lists with a known right answer (SPEC.md Phase 9 E1). Each test is one
//! situation a viewer could be in, and what they should get.

mod common;

use common::*;
use sinephile_selection::{
    offers, rank, resolution_code, resolution_from_code, score, Context, Machine,
};
use source_protocol::{Resolution::*, SourceCandidate, VideoCodec};

/// A 140-minute film on a Tier 0 machine, nothing chosen.
fn film() -> Context {
    Context {
        runtime_minutes: Some(140),
        ..Context::default()
    }
}

fn order(list: Vec<SourceCandidate>, ctx: &Context) -> Vec<String> {
    rank(list, ctx).into_iter().map(|(c, _)| c.key).collect()
}

#[test]
fn a_700_mb_4k_file_is_a_lie() {
    let ctx = Context {
        machine: Machine {
            max_resolution: Uhd,
            ..Machine::default()
        },
        ..film()
    };
    let fake = swarm("fake", 900).res(Uhd).size(700 * MB).done();
    let honest = swarm("honest", 40)
        .res(FullHd)
        .codec(VideoCodec::H264)
        .size(4 * GB)
        .done();
    let ranked = rank(vec![fake, honest], &ctx);
    assert_eq!(ranked[0].0.key, "honest");
    let sunk = ranked[1].1.sunk.as_deref().unwrap();
    assert!(sunk.contains("too small to be 4K"), "{sunk}");
}

#[test]
fn a_60_minute_file_for_a_140_minute_film_is_a_lie() {
    let short = http("short").res(FullHd).minutes(60).done();
    let whole = http("whole").res(Hd).minutes(141).done();
    let ranked = rank(vec![short, whole], &film());
    assert_eq!(ranked[0].0.key, "whole");
    assert_eq!(
        ranked[1].1.sunk.as_deref(),
        Some("plays 60 minutes; this runs 140")
    );
}

#[test]
fn an_extended_cut_is_not_a_lie() {
    let s = score(&http("long").minutes(170).done(), &film());
    assert_eq!(s.sunk, None);
}

#[test]
fn your_own_file_beats_a_better_stream() {
    let mine = local("mine").res(Hd).done();
    let best = swarm("best", 2000)
        .res(FullHd)
        .codec(VideoCodec::H264)
        .size(8 * GB)
        .minutes(140)
        .done();
    assert_eq!(order(vec![best, mine], &film()), ["mine", "best"]);
}

#[test]
fn a_fake_on_this_machine_still_sinks() {
    let exe = local("C:/Films/Film.2019.1080p.exe").res(FullHd).done();
    let http = http("stream").res(Sd).done();
    let ranked = rank(vec![exe, http], &film());
    assert_eq!(ranked[0].0.key, "stream");
    assert_eq!(ranked[1].1.sunk.as_deref(), Some("not a video file (.exe)"));
}

#[test]
fn choosing_hindi_plays_hindi_even_at_lower_quality() {
    let ctx = Context {
        audio_language: Some("hi".into()),
        ..film()
    };
    let english = swarm("en", 500).res(FullHd).audio(&["en"]).done();
    let hindi = swarm("hi", 20).res(Hd).audio(&["hi-IN", "en"]).done();
    let ranked = rank(vec![english, hindi], &ctx);
    assert_eq!(ranked[0].0.key, "hi");
    assert_eq!(ranked[0].1.unmet, None);
    assert_eq!(ranked[1].1.unmet.as_deref(), Some("carries en, not hi"));
}

#[test]
fn when_nothing_carries_the_language_every_copy_says_so() {
    let ctx = Context {
        audio_language: Some("hi".into()),
        ..film()
    };
    let ranked = rank(
        vec![
            http("a").audio(&["en"]).done(),
            http("b").audio(&["ja"]).done(),
        ],
        &ctx,
    );
    assert!(ranked.iter().all(|(_, s)| s.unmet.is_some()));
}

#[test]
fn an_unstated_language_ranks_above_a_known_wrong_one() {
    let ctx = Context {
        audio_language: Some("hi".into()),
        ..film()
    };
    let wrong = http("wrong").res(FullHd).audio(&["en"]).done();
    let unknown = http("unknown").res(Sd).done();
    assert_eq!(order(vec![wrong, unknown], &ctx), ["unknown", "wrong"]);
}

#[test]
fn choosing_4k_prefers_4k_and_says_when_there_is_none() {
    let ctx = Context {
        wanted: Some(Uhd),
        ..film()
    };
    let uhd = swarm("uhd", 30)
        .res(Uhd)
        .codec(VideoCodec::H265)
        .size(20 * GB)
        .done();
    let fhd = swarm("fhd", 300).res(FullHd).done();
    let ranked = rank(vec![fhd.clone(), uhd], &ctx);
    assert_eq!(ranked[0].0.key, "uhd");
    assert_eq!(ranked[1].1.unmet.as_deref(), Some("1080p, not 4K"));

    let none = rank(vec![fhd], &ctx);
    assert_eq!(none[0].1.unmet.as_deref(), Some("1080p, not 4K"));
}

#[test]
fn with_no_choice_a_modest_machine_prefers_1080p_to_4k() {
    let uhd = swarm("uhd", 300).res(Uhd).done();
    let fhd = swarm("fhd", 300).res(FullHd).done();
    assert_eq!(order(vec![uhd, fhd], &film()), ["fhd", "uhd"]);
}

#[test]
fn a_dead_swarm_loses_to_a_modest_stream() {
    let dead = swarm("dead", 0).res(FullHd).done();
    let stream = http("stream").res(Hd).done();
    assert_eq!(order(vec![dead, stream], &film()), ["stream", "dead"]);
}

#[test]
fn h265_without_hardware_decode_loses_to_h264() {
    let hevc = swarm("newer", 100)
        .res(FullHd)
        .codec(VideoCodec::H265)
        .done();
    let avc = swarm("avc", 100).res(FullHd).codec(VideoCodec::H264).done();
    assert_eq!(
        order(vec![hevc.clone(), avc.clone()], &film()),
        ["avc", "newer"]
    );
    // With hardware decode the codec stops mattering. (Keys double as titles here, so
    // neither may contain a codec tag.)
    let ctx = Context {
        machine: Machine {
            hevc: true,
            ..Machine::default()
        },
        ..film()
    };
    let s = score(&hevc, &ctx);
    assert_eq!(s.total, score(&avc, &ctx).total);
}

#[test]
fn a_camera_recording_ranks_below_a_disc_encode() {
    let cam = swarm("cam", 800)
        .res(FullHd)
        .titled("Film 2026 HDCAM")
        .done();
    let disc = swarm("disc", 50)
        .res(FullHd)
        .titled("Film 2026 1080p BluRay x264")
        .done();
    assert_eq!(order(vec![cam, disc], &film()), ["disc", "cam"]);
}

#[test]
fn a_name_that_contradicts_its_listing_is_a_mislabel() {
    let s = score(
        &http("x").res(Uhd).titled("Film 720p WEBRip").done(),
        &film(),
    );
    assert_eq!(s.sunk.as_deref(), Some("named 720p, listed as 4K"));
}

#[test]
fn a_stream_faster_than_the_connection_loses() {
    let ctx = Context {
        throughput: Some(1_000_000), // 8 Mbit/s
        ..film()
    };
    // 20 GB over 140 minutes is about 19 Mbit/s; 3 GB is about 2.9.
    let heavy = http("heavy").res(FullHd).size(20 * GB).done();
    let light = http("light").res(FullHd).size(3 * GB).done();
    let ranked = rank(vec![heavy, light], &ctx);
    assert_eq!(ranked[0].0.key, "light");
    let why = &ranked[1]
        .1
        .terms
        .iter()
        .find(|t| t.name == "throughput")
        .unwrap()
        .why;
    assert_eq!(why, "needs 19.0 Mbit/s, measured 8.0");
}

#[test]
fn wanted_subtitles_tip_a_tie() {
    let ctx = Context {
        subtitle_language: Some("en".into()),
        ..film()
    };
    let bare = http("a").res(FullHd).done();
    let subbed = http("b").res(FullHd).subs(&["en"]).done();
    assert_eq!(order(vec![bare, subbed], &ctx), ["b", "a"]);
}

#[test]
fn hdr_on_an_sdr_display_costs_a_little() {
    let hdr = score(
        &http("h").res(FullHd).codec(VideoCodec::H264).hdr().done(),
        &film(),
    );
    let sdr = score(
        &http("s").res(FullHd).codec(VideoCodec::H264).done(),
        &film(),
    );
    assert!(hdr.total < sdr.total);
    assert_eq!(hdr.sunk, None);
}

#[test]
fn every_term_explains_itself() {
    let s = score(
        &swarm("x", 12).res(FullHd).size(3 * GB).minutes(139).done(),
        &film(),
    );
    let names: Vec<_> = s.terms.iter().map(|t| t.name).collect();
    assert_eq!(
        names,
        [
            "language",
            "quality",
            "plausibility",
            "duration",
            "encoding",
            "health",
            "compatibility",
            "throughput",
            "subtitles"
        ]
    );
    assert!(s.terms.iter().all(|t| !t.why.is_empty()));
}

#[test]
fn the_choices_offered_are_what_honest_copies_carry() {
    let ranked = rank(
        vec![
            swarm("a", 50).res(FullHd).audio(&["ja", "en"]).done(),
            http("b").res(Hd).audio(&["ja-JP"]).done(),
            http("c").res(FullHd).done(),
            // A fake's claims offer nothing: choosing "fr" or 4K could only play the fake.
            http("fake")
                .res(Uhd)
                .size(100 * MB)
                .minutes(140)
                .audio(&["fr"])
                .done(),
        ],
        &film(),
    );
    let o = offers(&ranked);
    assert_eq!(o.languages, ["ja", "en"]);
    assert_eq!(o.qualities, [FullHd, Hd]);
}

#[test]
fn a_quality_choice_round_trips_through_its_wire_name() {
    for r in [Sd, Hd, FullHd, Uhd] {
        assert_eq!(resolution_from_code(resolution_code(r)), Some(r));
        // The same names the addon protocol uses.
        let wire = serde_json::to_string(&r).unwrap();
        assert_eq!(wire, format!("\"{}\"", resolution_code(r)));
    }
    assert_eq!(resolution_from_code("8k"), None);
}
