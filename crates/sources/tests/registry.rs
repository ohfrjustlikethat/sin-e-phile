//! The registry decides who is asked; the resolver only asks.

use std::sync::Arc;
use std::time::Duration;

use sinephile_persistence::repositories::SourceRepository;
use sinephile_persistence::Db;
use sinephile_sources::registry::{record, resolver_from};
use sinephile_sources::Outcome;
use source_protocol::{
    async_trait, BackendCapabilities, MediaKind, MediaQuery, PlayableStream, SourceBackend,
    SourceCandidate, SourceError, Transport,
};

struct Named(&'static str);

#[async_trait]
impl SourceBackend for Named {
    fn id(&self) -> &str {
        self.0
    }
    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            media: vec![MediaKind::Film],
            transports: vec![Transport::Http],
            network: false,
        }
    }
    async fn find(&self, _q: &MediaQuery) -> Result<Vec<SourceCandidate>, SourceError> {
        Ok(vec![])
    }
    async fn resolve(&self, _c: &SourceCandidate) -> Result<PlayableStream, SourceError> {
        Err(SourceError::NotFound("nothing".into()))
    }
}

fn available(ids: &[&'static str]) -> Vec<Arc<dyn SourceBackend>> {
    ids.iter()
        .map(|id| Arc::new(Named(id)) as Arc<dyn SourceBackend>)
        .collect()
}

#[tokio::test]
async fn a_switched_off_backend_is_never_asked_and_order_is_the_users() {
    let db = Db::in_memory().await.unwrap();
    let r = resolver_from(&db, available(&["a", "b", "c"]))
        .await
        .unwrap();
    assert_eq!(
        r.ids(),
        ["a", "b", "c"],
        "first run: registered in the order given"
    );

    let repo = SourceRepository::new(&db);
    repo.set_enabled("b", false).await.unwrap();
    repo.reorder(&["c", "b", "a"].map(String::from))
        .await
        .unwrap();
    let r = resolver_from(&db, available(&["a", "b", "c"]))
        .await
        .unwrap();
    assert_eq!(r.ids(), ["c", "a"]);

    let found = r
        .find_all(&MediaQuery::film("Metropolis", Some(1927)))
        .await;
    let asked: Vec<&str> = found.outcomes.iter().map(|o| o.backend.as_str()).collect();
    assert!(!asked.contains(&"b"), "asked {asked:?}");
}

#[tokio::test]
async fn a_registered_backend_that_is_not_available_is_skipped() {
    let db = Db::in_memory().await.unwrap();
    resolver_from(&db, available(&["a", "addon:gone"]))
        .await
        .unwrap();
    let r = resolver_from(&db, available(&["a"])).await.unwrap();
    assert_eq!(r.ids(), ["a"]);
}

#[tokio::test]
async fn outcomes_become_health() {
    let db = Db::in_memory().await.unwrap();
    resolver_from(&db, available(&["a", "b", "c"]))
        .await
        .unwrap();
    let outcome = |backend: &str, result| Outcome {
        backend: backend.into(),
        elapsed: Duration::from_millis(40),
        result,
    };
    record(
        &db,
        &[
            outcome("a", Ok(3)),
            outcome("b", Err(SourceError::Timeout(8000))),
            outcome("c", Err(SourceError::Unavailable("refused".into()))),
        ],
    )
    .await
    .unwrap();
    let rows = SourceRepository::new(&db).all().await.unwrap();
    assert_eq!((rows[0].successes, rows[0].latency_ms), (1, Some(40.0)));
    assert_eq!(rows[1].timeouts, 1);
    assert_eq!(rows[2].failures, 1);
    assert_eq!(rows[2].last_error.as_deref(), Some("unavailable: refused"));
}

#[tokio::test]
async fn the_overview_names_built_ins_in_words_and_hides_dead_addons() {
    use sinephile_sources::backends::{all, Context};
    use sinephile_sources::overview::overview;

    let db = Db::in_memory().await.unwrap();
    // A stored addon whose manifest is gone from the context, as a refused one would be.
    SourceRepository::new(&db)
        .install_addon("addon:gone", "https://gone.example/m.json", "{}")
        .await
        .unwrap();
    let backends = all(&Context::default());
    let rows = overview(&db, &backends, &[]).await.unwrap();
    let names: Vec<&str> = rows.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, ["Your files", "Internet Archive", "Your links"]);
    assert!(rows.iter().all(|r| r.health == "Not asked yet."));
    assert!(rows.iter().all(|r| r.addon_host.is_none()));
}
