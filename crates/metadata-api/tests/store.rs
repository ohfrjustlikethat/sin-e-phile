//! The cache store against a migrated database (ADR-0026's standing requirement).

use sinephile_metadata_api::{CacheStore, Freshness, Resource, Store};
use sinephile_persistence::Db;

/// Backdate an entry, so freshness can be tested without waiting.
async fn age_entry(db: &Db, url: &str, seconds: i64) {
    sqlx::query(
        "UPDATE http_cache SET fetched_at = datetime('now', '-' || ? || ' seconds') WHERE url = ?",
    )
    .bind(seconds)
    .bind(url)
    .execute(db.pool())
    .await
    .expect("backdate");
}

const URL: &str = "https://example.test/film/1";
const BODY: &str = r#"{"title":"Ran"}"#;

async fn stored() -> Db {
    let db = Db::in_memory().await.expect("open");
    CacheStore::new(&db)
        .put(Store::ok(URL, "tmdb", Resource::Detail, BODY))
        .await
        .expect("put");
    db
}

#[tokio::test]
async fn a_fresh_entry_is_served() {
    let db = stored().await;
    let hit = CacheStore::new(&db)
        .get(URL, Resource::Detail, false, false)
        .await
        .expect("get")
        .expect("present");

    assert_eq!(hit.freshness, Freshness::Fresh);
    assert_eq!(hit.body, BODY);
    assert_eq!(hit.status, 200);
}

#[tokio::test]
async fn a_stale_entry_is_withheld_online_and_served_offline() {
    // The whole reason expiry and deletion are different things. "Graceful offline
    // behaviour" means a month-old cast list beats an empty screen.
    let db = stored().await;
    age_entry(&db, URL, 60 * 60 * 24 * 40).await; // 40 days; Detail's TTL is 30

    let store = CacheStore::new(&db);
    assert!(
        store
            .get(URL, Resource::Detail, false, false)
            .await
            .expect("get")
            .is_none(),
        "online, this should be refreshed rather than served"
    );

    let offline = store
        .get(URL, Resource::Detail, true, false)
        .await
        .expect("get")
        .expect("served offline");
    assert_eq!(offline.freshness, Freshness::Stale);
}

#[tokio::test]
async fn an_entry_past_max_age_is_not_served_even_offline() {
    let db = stored().await;
    age_entry(&db, URL, 60 * 60 * 24 * 400).await;

    assert!(CacheStore::new(&db)
        .get(URL, Resource::Detail, true, false)
        .await
        .expect("get")
        .is_none());
}

#[tokio::test]
async fn include_stale_returns_an_entry_for_its_validators() {
    // A conditional refresh needs the ETag of an entry it will not serve.
    let db = Db::in_memory().await.expect("open");
    CacheStore::new(&db)
        .put(Store::ok(URL, "tmdb", Resource::Detail, BODY).with_etag(Some("W/\"abc\"")))
        .await
        .expect("put with etag");
    age_entry(&db, URL, 60 * 60 * 24 * 40).await;

    let entry = CacheStore::new(&db)
        .get(URL, Resource::Detail, false, true)
        .await
        .expect("get")
        .expect("returned for its validators");
    assert_eq!(entry.freshness, Freshness::Stale);
    assert_eq!(entry.validators(), Some((Some("W/\"abc\""), None)));
}

#[tokio::test]
async fn an_entry_with_no_validators_offers_none() {
    let db = stored().await;
    let entry = CacheStore::new(&db)
        .get(URL, Resource::Detail, false, false)
        .await
        .expect("get")
        .expect("present");
    assert_eq!(
        entry.validators(),
        None,
        "a conditional request would be pointless"
    );
}

#[tokio::test]
async fn touch_refreshes_the_clock_without_rewriting_the_body() {
    // What a 304 means: unchanged, so only the clock moves.
    let db = stored().await;
    age_entry(&db, URL, 60 * 60 * 24 * 40).await;

    CacheStore::new(&db)
        .touch(URL, Resource::Detail)
        .await
        .expect("touch");

    let entry = CacheStore::new(&db)
        .get(URL, Resource::Detail, false, false)
        .await
        .expect("get")
        .expect("fresh again");
    assert_eq!(entry.freshness, Freshness::Fresh);
    assert_eq!(entry.body, BODY, "the body was not rewritten");
}

#[tokio::test]
async fn a_second_put_replaces_rather_than_duplicating() {
    let db = stored().await;
    CacheStore::new(&db)
        .put(Store::ok(URL, "tmdb", Resource::Detail, "updated"))
        .await
        .expect("put again");

    assert_eq!(CacheStore::new(&db).len().await.expect("len"), 1);
    let entry = CacheStore::new(&db)
        .get(URL, Resource::Detail, false, false)
        .await
        .expect("get")
        .expect("present");
    assert_eq!(entry.body, "updated");
}

#[tokio::test]
async fn an_api_key_never_reaches_the_table() {
    // §2.4 copies this database with the app folder; ADR-0027 makes keys per-user.
    // A key stored here would travel to wherever the folder went.
    let db = Db::in_memory().await.expect("open");
    let with_key = "https://example.test/film/1?api_key=SECRET&language=en";
    CacheStore::new(&db)
        .put(Store::ok(with_key, "tmdb", Resource::Detail, "body"))
        .await
        .expect("put");

    let stored_url: String = sqlx::query_scalar("SELECT url FROM http_cache")
        .fetch_one(db.pool())
        .await
        .expect("query");
    assert!(
        !stored_url.contains("SECRET"),
        "the key leaked: {stored_url}"
    );
    assert_eq!(stored_url, "https://example.test/film/1?language=en");

    // And a lookup WITH the key still finds it, because both sides are stripped.
    assert!(CacheStore::new(&db)
        .get(with_key, Resource::Detail, false, false)
        .await
        .expect("get")
        .is_some());
}

#[tokio::test]
async fn purge_removes_only_what_is_past_max_age() {
    // Purging on expires_at would throw away exactly the entries that make offline
    // behaviour graceful.
    let db = Db::in_memory().await.expect("open");
    let store = CacheStore::new(&db);

    store
        .put(Store::ok(
            "https://example.test/a",
            "tmdb",
            Resource::Detail,
            "a",
        ))
        .await
        .expect("a");
    store
        .put(Store::ok(
            "https://example.test/b",
            "tmdb",
            Resource::Detail,
            "b",
        ))
        .await
        .expect("b");

    // 'a' is stale but still useful; 'b' is beyond max_age.
    age_entry(&db, "https://example.test/a", 60 * 60 * 24 * 60).await;
    age_entry(&db, "https://example.test/b", 60 * 60 * 24 * 400).await;

    assert_eq!(store.purge().await.expect("purge"), 1);
    assert_eq!(store.len().await.expect("len"), 1);
    assert!(store
        .get("https://example.test/a", Resource::Detail, true, false)
        .await
        .expect("get")
        .is_some());
}

#[tokio::test]
async fn a_schedule_expires_far_sooner_than_a_detail() {
    // A wrong airing time is worse than a missing one; a month-old cast list is not.
    let db = Db::in_memory().await.expect("open");
    let store = CacheStore::new(&db);
    let url = "https://example.test/schedule";
    store
        .put(Store::ok(url, "anilist", Resource::Schedule, "{}"))
        .await
        .expect("put");

    age_entry(&db, url, 60 * 60 * 8).await; // 8 hours; Schedule's TTL is 6
    let entry = store
        .get(url, Resource::Schedule, true, false)
        .await
        .expect("get")
        .expect("still serveable offline");
    assert_eq!(entry.freshness, Freshness::Stale);

    age_entry(&db, url, 60 * 60 * 24 * 30).await;
    assert!(
        store
            .get(url, Resource::Schedule, true, false)
            .await
            .expect("get")
            .is_none(),
        "a month-old schedule is not worth serving"
    );
}

#[tokio::test]
async fn a_clock_that_went_backwards_does_not_make_an_entry_fresh_forever() {
    let db = stored().await;
    // Fetched "in the future".
    sqlx::query("UPDATE http_cache SET fetched_at = datetime('now', '+1 day')")
        .execute(db.pool())
        .await
        .expect("skew");

    let entry = CacheStore::new(&db)
        .get(URL, Resource::Detail, false, false)
        .await
        .expect("get")
        .expect("present");
    assert_eq!(
        entry.freshness,
        Freshness::Fresh,
        "a negative age clamps to zero rather than overflowing into nonsense"
    );
}

#[tokio::test]
async fn a_non_200_response_is_cacheable_too() {
    // A cached 404 is a 404 not re-requested a hundred times. The backoff policy
    // already says it is not worth retrying; the cache is what makes that stick
    // across restarts.
    let db = Db::in_memory().await.expect("open");
    let missing = "https://example.test/film/999";
    CacheStore::new(&db)
        .put(Store {
            url: missing,
            source: "tmdb",
            resource: Resource::Detail,
            status: 404,
            body: "",
            etag: None,
            last_modified: None,
        })
        .await
        .expect("put");

    let entry = CacheStore::new(&db)
        .get(missing, Resource::Detail, false, false)
        .await
        .expect("get")
        .expect("present");
    assert_eq!(entry.status, 404);
}
