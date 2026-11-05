//! ADR-0030 layer 3: a search that misses, a source that finds it anyway, and the next
//! search finding it in the catalogue.

use std::sync::Arc;

use sinephile_persistence::repositories::{BackfillRepository, SearchRepository, SourceRepository};
use sinephile_persistence::Db;
use sinephile_sources::backfill::backfill;
use sinephile_sources::{Entry, Resolver};
use source_protocol::{
    async_trait, BackendCapabilities, MediaKind, MediaQuery, PlayableStream, SourceBackend,
    SourceCandidate, SourceError, Transport,
};

/// A source that knows one film.
struct Knows(&'static str, Option<u16>);

#[async_trait]
impl SourceBackend for Knows {
    fn id(&self) -> &str {
        "test-source"
    }
    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            media: vec![MediaKind::Film],
            transports: vec![Transport::Http],
            network: false,
        }
    }
    async fn find(&self, q: &MediaQuery) -> Result<Vec<SourceCandidate>, SourceError> {
        if !self.0.to_lowercase().contains(&q.title.to_lowercase()) {
            return Ok(vec![]);
        }
        Ok(vec![SourceCandidate {
            key: "k".into(),
            title: self.0.into(),
            year: self.1,
            transport: Transport::Http,
            quality: Default::default(),
            languages: vec![],
            subtitles: vec![],
            codec: Default::default(),
            size_bytes: None,
            health: Default::default(),
            locator: Some("https://films.example/k.mp4".into()),
            provenance: Default::default(),
        }])
    }
    async fn resolve(&self, _c: &SourceCandidate) -> Result<PlayableStream, SourceError> {
        Err(SourceError::NotFound("x".into()))
    }
}

fn resolver(title: &'static str, year: Option<u16>) -> Resolver {
    Resolver::new(vec![Entry::new(Arc::new(Knows(title, year)))])
}

#[tokio::test]
async fn a_miss_found_by_a_source_is_in_the_catalogue_for_the_next_search() {
    let db = Db::in_memory().await.unwrap();
    SourceRepository::new(&db)
        .ensure(&["test-source"])
        .await
        .unwrap();
    let search = SearchRepository::new(&db);
    assert!(
        search
            .search("The Phantom Carriage", 10)
            .await
            .unwrap()
            .is_empty(),
        "a miss"
    );

    let r = resolver("The Phantom Carriage", Some(1921));
    let made = backfill(&db, &r, "The Phantom Carriage")
        .await
        .unwrap()
        .expect("backfilled");
    assert_eq!(
        (made.title.as_str(), made.year, made.from.as_str()),
        ("The Phantom Carriage", 1921, "test-source")
    );

    let hits = search.search("The Phantom Carriage", 10).await.unwrap();
    assert_eq!(
        hits.first().map(|h| h.media_item_id),
        Some(made.id),
        "found next time"
    );

    // The same search again creates nothing: the catalogue has it now.
    assert_eq!(
        backfill(&db, &r, "the phantom carriage").await.unwrap(),
        None
    );
    assert_eq!(BackfillRepository::new(&db).all().await.unwrap().len(), 1);

    // And the source's answer counted toward its health.
    let row = &SourceRepository::new(&db).all().await.unwrap()[0];
    assert_eq!(row.successes, 2);
}

#[tokio::test]
async fn nothing_is_written_without_a_year_or_for_a_query_still_being_typed() {
    let db = Db::in_memory().await.unwrap();
    assert_eq!(
        backfill(&db, &resolver("Häxan", None), "Häxan")
            .await
            .unwrap(),
        None
    );
    assert_eq!(
        backfill(&db, &resolver("Häxan", Some(1922)), "Hä")
            .await
            .unwrap(),
        None
    );
    assert!(BackfillRepository::new(&db).all().await.unwrap().is_empty());
}
