//! `plan` against a catalogue row and fake sources.

use std::sync::Arc;

use sinephile_persistence::repositories::profiles::PlaybackPosition;
use sinephile_persistence::repositories::{MediaRepository, ProfileRepository};
use sinephile_persistence::{Db, NewMediaItem};
use sinephile_playback::{plan, plan_with, Context, PlayError};
use sinephile_sources::{Entry, Resolver};
use source_protocol::{
    async_trait, BackendCapabilities, Health, MediaKind, MediaQuery, PlayableStream, Quality,
    Resolution, SourceBackend, SourceCandidate, SourceError, Transport,
};

/// A source with one HTTP candidate that either resolves or does not.
struct Fake {
    id: &'static str,
    resolves: bool,
    url: &'static str,
    /// How long the source says its file plays.
    minutes: Option<u32>,
    /// The audio languages and resolution it states.
    audio: &'static [&'static str],
    res: Resolution,
}

#[async_trait]
impl SourceBackend for Fake {
    fn id(&self) -> &str {
        self.id
    }
    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            media: vec![MediaKind::Film],
            transports: vec![Transport::Http],
            network: false,
        }
    }
    async fn find(&self, q: &MediaQuery) -> Result<Vec<SourceCandidate>, SourceError> {
        assert_eq!(q.title, "Sunrise");
        assert_eq!(q.year, Some(1927));
        Ok(vec![SourceCandidate {
            key: self.url.into(),
            title: "Sunrise".into(),
            year: Some(1927),
            transport: Transport::Http,
            quality: Quality {
                resolution: self.res,
                hdr: false,
            },
            languages: self.audio.iter().map(|l| l.to_string()).collect(),
            subtitles: vec![],
            codec: Default::default(),
            size_bytes: None,
            duration_seconds: self.minutes.map(|m| m * 60),
            health: Health::Direct,
            locator: Some(self.url.into()),
            provenance: Default::default(),
        }])
    }
    async fn resolve(&self, c: &SourceCandidate) -> Result<PlayableStream, SourceError> {
        if self.resolves {
            Ok(PlayableStream::Http {
                url: c.key.clone(),
                content_type: None,
                size_bytes: None,
            })
        } else {
            Err(SourceError::Unavailable("gone".into()))
        }
    }
}

fn resolver(backends: Vec<Fake>) -> Resolver {
    Resolver::new(
        backends
            .into_iter()
            .map(|b| Entry::new(Arc::new(b)))
            .collect(),
    )
}

async fn film(db: &Db) -> i64 {
    MediaRepository::new(db)
        .insert(&NewMediaItem::film("Sunrise", 1927))
        .await
        .unwrap()
}

#[tokio::test]
async fn the_position_survives_a_restart() {
    // Phase 8 E4: the player saves through `save_position`; the app closes; a new
    // process opens the same database file and plans the same film.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sinephile.db");
    let id = {
        let db = Db::open(&path).await.unwrap();
        let id = film(&db).await;
        let profile = ProfileRepository::new(&db).ensure_default().await.unwrap();
        ProfileRepository::new(&db)
            .save_position(
                profile,
                &PlaybackPosition {
                    media_item_id: id,
                    position_seconds: 2712,
                    duration_seconds: Some(5700),
                    audio_language: None,
                    subtitle_language: None,
                },
            )
            .await
            .unwrap();
        db.pool().close().await;
        id
    };
    let db = Db::open(&path).await.unwrap();
    let r = resolver(vec![Fake {
        id: "a",
        resolves: true,
        url: "https://films.example/sunrise.mp4",
        minutes: None,
        audio: &[],
        res: Resolution::Unknown,
    }]);
    let p = plan(&db, &r, None, id).await.unwrap();
    assert_eq!(p.start, Some(2712.0), "resumes where the last run stopped");
}

#[tokio::test]
async fn a_playable_source_and_the_saved_position() {
    let db = Db::in_memory().await.unwrap();
    let id = film(&db).await;
    let profile = ProfileRepository::new(&db).ensure_default().await.unwrap();
    ProfileRepository::new(&db)
        .save_position(
            profile,
            &PlaybackPosition {
                media_item_id: id,
                position_seconds: 1200,
                duration_seconds: Some(5700),
                audio_language: None,
                subtitle_language: None,
            },
        )
        .await
        .unwrap();
    let r = resolver(vec![Fake {
        id: "a",
        resolves: true,
        url: "https://films.example/sunrise.mp4",
        minutes: None,
        audio: &[],
        res: Resolution::Unknown,
    }]);
    let p = plan(&db, &r, None, id).await.unwrap();
    assert_eq!(p.open, "https://films.example/sunrise.mp4");
    assert_eq!(p.start, Some(1200.0));
    assert_eq!(p.backend, "a");
}

#[tokio::test]
async fn a_source_that_cannot_be_opened_is_passed_over() {
    let db = Db::in_memory().await.unwrap();
    let id = film(&db).await;
    let r = resolver(vec![
        Fake {
            id: "broken",
            resolves: false,
            url: "https://a.example/x.mp4",
            minutes: None,
            audio: &[],
            res: Resolution::Unknown,
        },
        Fake {
            id: "works",
            resolves: true,
            url: "https://b.example/y.mp4",
            minutes: None,
            audio: &[],
            res: Resolution::Unknown,
        },
    ]);
    let p = plan(&db, &r, None, id).await.unwrap();
    assert_eq!((p.backend.as_str(), p.offered), ("works", 2));
    assert_eq!(p.start, None, "never watched");
}

#[tokio::test]
async fn nothing_to_play_is_said_plainly() {
    let db = Db::in_memory().await.unwrap();
    let id = film(&db).await;
    assert!(matches!(
        plan(&db, &resolver(vec![]), None, id).await,
        Err(PlayError::NoSources)
    ));
    let r = resolver(vec![Fake {
        id: "broken",
        resolves: false,
        url: "https://a.example/x.mp4",
        minutes: None,
        audio: &[],
        res: Resolution::Unknown,
    }]);
    let err = plan(&db, &r, None, id).await.unwrap_err();
    assert!(err.to_string().contains("none could be opened"), "{err}");
    assert!(matches!(
        plan(&db, &r, None, 999_999).await,
        Err(PlayError::Unknown)
    ));
}

#[tokio::test]
async fn the_scorer_chooses_not_the_order_sources_answer_in() {
    // A 20-minute file for a 94-minute film is sunk (Phase 9), even though its source
    // answers first and resolves.
    let db = Db::in_memory().await.unwrap();
    let id = MediaRepository::new(&db)
        .insert(&NewMediaItem {
            runtime_minutes: Some(94),
            ..NewMediaItem::film("Sunrise", 1927)
        })
        .await
        .unwrap();
    let r = resolver(vec![
        Fake {
            id: "short",
            resolves: true,
            url: "https://a.example/trailer.mp4",
            minutes: Some(20),
            audio: &[],
            res: Resolution::Unknown,
        },
        Fake {
            id: "whole",
            resolves: true,
            url: "https://b.example/film.mp4",
            minutes: Some(94),
            audio: &[],
            res: Resolution::Unknown,
        },
    ]);
    let p = plan(&db, &r, None, id).await.unwrap();
    assert_eq!(p.backend, "whole");
    assert_eq!(p.score.sunk, None);
}

fn two(a: Fake, b: Fake) -> Resolver {
    resolver(vec![a, b])
}

fn fake(
    id: &'static str,
    url: &'static str,
    audio: &'static [&'static str],
    res: Resolution,
) -> Fake {
    Fake {
        id,
        resolves: true,
        url,
        minutes: None,
        audio,
        res,
    }
}

#[tokio::test]
async fn choosing_hindi_opens_the_copy_with_hindi_audio() {
    // Phase 9 E2.
    let db = Db::in_memory().await.unwrap();
    let id = film(&db).await;
    let r = two(
        fake(
            "en",
            "https://a.example/en.mp4",
            &["en"],
            Resolution::FullHd,
        ),
        fake("hi", "https://b.example/hi.mp4", &["hi"], Resolution::Hd),
    );
    let hindi = Context {
        audio_language: Some("hi".into()),
        ..Context::default()
    };
    let p = plan_with(&db, &r, None, id, hindi.clone()).await.unwrap();
    assert_eq!(
        (p.open.as_str(), p.score.unmet.as_deref()),
        ("https://b.example/hi.mp4", None)
    );
    assert_eq!(p.offers.languages, ["en", "hi"]);

    // No Hindi anywhere: it still plays, and says why the copy is not what was asked.
    let r = resolver(vec![fake(
        "en",
        "https://a.example/en.mp4",
        &["en"],
        Resolution::FullHd,
    )]);
    let p = plan_with(&db, &r, None, id, hindi).await.unwrap();
    assert_eq!(p.score.unmet.as_deref(), Some("carries en, not hi"));
}

#[tokio::test]
async fn choosing_4k_opens_the_4k_copy_or_says_there_is_none() {
    // Phase 9 E3.
    let db = Db::in_memory().await.unwrap();
    let id = film(&db).await;
    let r = two(
        fake("fhd", "https://a.example/fhd.mp4", &[], Resolution::FullHd),
        fake("uhd", "https://b.example/uhd.mp4", &[], Resolution::Uhd),
    );
    let uhd = Context {
        wanted: Some(Resolution::Uhd),
        ..Context::default()
    };
    let p = plan_with(&db, &r, None, id, uhd.clone()).await.unwrap();
    assert_eq!(p.open, "https://b.example/uhd.mp4");
    // With nothing chosen, a Tier 0 machine is given 1080p.
    let p = plan(&db, &r, None, id).await.unwrap();
    assert_eq!(p.open, "https://a.example/fhd.mp4");

    let r = resolver(vec![fake(
        "fhd",
        "https://a.example/fhd.mp4",
        &[],
        Resolution::FullHd,
    )]);
    let p = plan_with(&db, &r, None, id, uhd).await.unwrap();
    assert_eq!(p.score.unmet.as_deref(), Some("1080p, not 4K"));
}

#[tokio::test]
async fn an_episode_is_asked_for_as_its_series_season_and_number() {
    use sinephile_persistence::repositories::EpisodeRepository;
    use sinephile_persistence::MediaKind as Kind;
    let db = Db::in_memory().await.unwrap();
    let media = MediaRepository::new(&db);
    let show = media
        .insert(&NewMediaItem {
            kind: Some(Kind::Series),
            primary_title: "Twin Peaks".into(),
            release_year: Some(1990),
            ..Default::default()
        })
        .await
        .unwrap();
    let ep = media
        .insert(&NewMediaItem {
            kind: Some(Kind::Episode),
            primary_title: "Lonely Souls".into(),
            ..Default::default()
        })
        .await
        .unwrap();
    let eps = EpisodeRepository::new(&db);
    eps.create_series(show, false).await.unwrap();
    eps.create_episode(ep, show, None, Some(2), Some(7), None)
        .await
        .unwrap();
    let q = sinephile_playback::query_for(&db, ep)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        (q.kind, q.title.as_str(), q.year, q.season, q.episode),
        (
            MediaKind::Episode,
            "Twin Peaks",
            Some(1990),
            Some(2),
            Some(7)
        )
    );
}
