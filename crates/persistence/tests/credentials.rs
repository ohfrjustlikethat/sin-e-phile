//! Per-profile credentials.
//!
//! Every SQL statement in `repositories/credentials.rs` runs against a freshly
//! migrated database here — ADR-0026's standing requirement, and the whole of the
//! protection given that the `query!` macros are not used.

use sinephile_persistence::repositories::{CredentialRepository, ProfileRepository, TmdbAccess};
use sinephile_persistence::Db;

const KEY: &str = "abcdef0123456789abcdef0123456789";
const OTHER: &str = "00112233445566778899aabbccddeeff";

async fn setup() -> (tempfile::TempDir, Db, i64, i64) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Db::open_in(dir.path()).await.expect("open");
    let profiles = ProfileRepository::new(&db);
    let alice = profiles.create("Alice", true).await.expect("alice");
    let bob = profiles.create("Bob", false).await.expect("bob");
    (dir, db, alice, bob)
}

#[tokio::test]
async fn a_profile_with_no_key_is_absent_not_empty() {
    // The default state of the application, and the one SPEC.md 9.4 is designed around.
    let (_dir, db, alice, _) = setup().await;
    let creds = CredentialRepository::new(&db);

    assert_eq!(
        creds.tmdb_access(alice).await.expect("access"),
        TmdbAccess::Absent
    );
    assert_eq!(creds.profiles_with_a_key().await.expect("count"), 0);
}

#[tokio::test]
async fn a_key_round_trips_and_is_not_stored_in_the_clear() {
    let (dir, db, alice, _) = setup().await;
    let creds = CredentialRepository::new(&db);
    creds.set_tmdb_key(alice, KEY).await.expect("set");

    assert_eq!(
        creds.tmdb_access(alice).await.expect("access"),
        TmdbAccess::Configured(KEY.to_string())
    );

    // The point of wrapping it: `data/` is portable and gets copied, so a grep of the
    // database file must not find the key.
    let bytes = std::fs::read(dir.path().join("sinephile.db")).expect("read the db file");
    assert!(
        !bytes.windows(KEY.len()).any(|w| w == KEY.as_bytes()),
        "the key is present in plaintext in the database file"
    );
}

#[tokio::test]
async fn keys_are_per_profile_and_do_not_leak_between_them() {
    // A household profile is a different person, and their key is theirs.
    let (_dir, db, alice, bob) = setup().await;
    let creds = CredentialRepository::new(&db);

    creds.set_tmdb_key(alice, KEY).await.expect("alice");
    assert_eq!(
        creds.tmdb_access(bob).await.expect("bob"),
        TmdbAccess::Absent,
        "Bob must not inherit Alice's key"
    );

    creds.set_tmdb_key(bob, OTHER).await.expect("bob set");
    assert_eq!(
        creds.tmdb_access(alice).await.expect("alice"),
        TmdbAccess::Configured(KEY.to_string())
    );
    assert_eq!(
        creds.tmdb_access(bob).await.expect("bob"),
        TmdbAccess::Configured(OTHER.to_string())
    );
    assert_eq!(creds.profiles_with_a_key().await.expect("count"), 2);
}

#[tokio::test]
async fn replacing_a_key_replaces_it_rather_than_adding_a_second() {
    let (_dir, db, alice, _) = setup().await;
    let creds = CredentialRepository::new(&db);

    creds.set_tmdb_key(alice, KEY).await.expect("first");
    creds.set_tmdb_key(alice, OTHER).await.expect("second");

    assert_eq!(
        creds.tmdb_access(alice).await.expect("access"),
        TmdbAccess::Configured(OTHER.to_string())
    );
    let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM profile_settings")
        .fetch_one(db.pool())
        .await
        .expect("count");
    assert_eq!(rows, 1);
}

#[tokio::test]
async fn removing_a_key_also_discards_what_was_fetched_with_it() {
    // ADR-0027 point 3. Leaving the cache behind would mean "remove my key" visibly did
    // nothing — the posters would still be there.
    let (_dir, db, alice, _) = setup().await;
    let creds = CredentialRepository::new(&db);
    creds.set_tmdb_key(alice, KEY).await.expect("set");

    for (url, source) in [
        ("https://api.example.test/3/movie/1", "tmdb"),
        ("https://api.example.test/3/movie/2", "tmdb"),
        ("https://graphql.anilist.co#Media(id:1)", "anilist"),
    ] {
        sqlx::query(
            "INSERT INTO http_cache (url, source, resource, body, status, fetched_at, expires_at)
             VALUES (?, ?, 'detail', '{}', 200, datetime('now'), datetime('now', '+7 days'))",
        )
        .bind(url)
        .bind(source)
        .execute(db.pool())
        .await
        .expect("cache row");
    }

    let (removed, discarded) = creds.clear_tmdb_key(alice).await.expect("clear");
    assert!(removed);
    assert_eq!(discarded, 2, "both TMDB responses");

    assert_eq!(
        creds.tmdb_access(alice).await.expect("access"),
        TmdbAccess::Absent
    );
    let remaining: Vec<String> = sqlx::query_scalar("SELECT source FROM http_cache")
        .fetch_all(db.pool())
        .await
        .expect("remaining");
    assert_eq!(
        remaining,
        vec!["anilist"],
        "AniList needs no key and its cache must survive"
    );
}

#[tokio::test]
async fn removing_a_key_that_was_never_set_is_not_an_error() {
    // ADR-0027 calls removal "a supported action, not an edge case".
    let (_dir, db, alice, _) = setup().await;
    let creds = CredentialRepository::new(&db);
    let (removed, _) = creds.clear_tmdb_key(alice).await.expect("clear");
    assert!(!removed);
}

#[tokio::test]
async fn a_malformed_key_is_refused_at_the_point_of_entry() {
    // Otherwise a user pastes their username, artwork silently never appears, and
    // nothing can attribute the failure to the key rather than the network.
    let (_dir, db, alice, _) = setup().await;
    let creds = CredentialRepository::new(&db);

    for bad in [
        "",
        "   ",
        "my-tmdb-username",
        "abcdef0123456789abcdef012345678",
    ] {
        let err = creds
            .set_tmdb_key(alice, bad)
            .await
            .expect_err("must be refused");
        assert!(
            matches!(err, sinephile_persistence::DbError::Invalid(_)),
            "{bad:?} gave {err:?}"
        );
    }
    assert_eq!(
        creds.tmdb_access(alice).await.expect("access"),
        TmdbAccess::Absent,
        "nothing was stored"
    );
}

#[tokio::test]
async fn a_key_is_trimmed_because_people_paste_with_whitespace() {
    let (_dir, db, alice, _) = setup().await;
    let creds = CredentialRepository::new(&db);
    creds
        .set_tmdb_key(alice, &format!("  {KEY}\n"))
        .await
        .expect("set");
    assert_eq!(
        creds.tmdb_access(alice).await.expect("access"),
        TmdbAccess::Configured(KEY.to_string())
    );
}

#[tokio::test]
async fn deleting_a_profile_takes_its_key_with_it() {
    // A secret that outlived the profile it belonged to would be a secret nobody owns.
    let (_dir, db, alice, _) = setup().await;
    let creds = CredentialRepository::new(&db);
    creds.set_tmdb_key(alice, KEY).await.expect("set");

    sqlx::query("DELETE FROM profiles WHERE id = ?")
        .bind(alice)
        .execute(db.pool())
        .await
        .expect("delete profile");

    let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM profile_settings")
        .fetch_one(db.pool())
        .await
        .expect("count");
    assert_eq!(rows, 0, "ON DELETE CASCADE must carry the key away");
}

#[tokio::test]
async fn a_secret_cannot_be_read_through_the_ordinary_accessor() {
    // Without this, a renamed key hands ciphertext to a caller expecting a string and
    // the first thing to notice is a failing API call a long way from here.
    let (_dir, db, alice, _) = setup().await;
    let creds = CredentialRepository::new(&db);
    creds.set_tmdb_key(alice, KEY).await.expect("set");

    let err = creds
        .get(alice, "tmdb.api_key")
        .await
        .expect_err("must refuse");
    assert!(matches!(err, sinephile_persistence::DbError::Invalid(_)));
}

#[tokio::test]
async fn ordinary_settings_still_work_alongside_secrets() {
    let (_dir, db, alice, _) = setup().await;
    let creds = CredentialRepository::new(&db);

    assert_eq!(creds.get(alice, "ui.theme").await.expect("absent"), None);
    creds.set(alice, "ui.theme", "dark").await.expect("set");
    assert_eq!(
        creds.get(alice, "ui.theme").await.expect("get"),
        Some("dark".to_string())
    );
    creds.set(alice, "ui.theme", "light").await.expect("update");
    assert_eq!(
        creds.get(alice, "ui.theme").await.expect("get"),
        Some("light".to_string())
    );
}
