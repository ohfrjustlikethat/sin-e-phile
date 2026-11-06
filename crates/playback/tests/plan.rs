//! `plan` against a catalogue row and fake sources.

use std::sync::Arc;

use sinephile_persistence::repositories::profiles::PlaybackPosition;
use sinephile_persistence::repositories::{MediaRepository, ProfileRepository};
use sinephile_persistence::{Db, NewMediaItem};
use sinephile_playback::{plan, PlayError};
use sinephile_sources::{Entry, Resolver};
use source_protocol::{
    async_trait, BackendCapabilities, Health, MediaKind, MediaQuery, PlayableStream, SourceBackend,
    SourceCandidate, SourceError, Transport,
};

/// A source with one HTTP candidate that either resolves or does not.
struct Fake {
    id: &'static str,
    resolves: bool,
    url: &'static str,
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
            quality: Default::default(),
            languages: vec![],
            subtitles: vec![],
            codec: Default::default(),
            size_bytes: None,
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
        },
        Fake {
            id: "works",
            resolves: true,
            url: "https://b.example/y.mp4",
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
    }]);
    let err = plan(&db, &r, None, id).await.unwrap_err();
    assert!(err.to_string().contains("none could be opened"), "{err}");
    assert!(matches!(
        plan(&db, &r, None, 999_999).await,
        Err(PlayError::Unknown)
    ));
}
