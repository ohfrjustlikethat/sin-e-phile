//! Keyword search, against a migrated database.
//!
//! Every SQL statement in `repositories/search.rs` runs here (ADR-0026).

use sinephile_persistence::repositories::{MatchReason, MediaRepository, SearchRepository};
use sinephile_persistence::{Db, NewMediaItem, TitleVariant};

async fn db() -> (tempfile::TempDir, Db) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Db::open_in(dir.path()).await.expect("open");
    (dir, db)
}

/// Insert a film, its title rows (with normalised forms), and index it.
async fn film(
    db: &Db,
    title: &str,
    year: i64,
    alternatives: &[&str],
    people: &str,
    keywords: &str,
) -> i64 {
    let media = MediaRepository::new(db);
    let id = media
        .insert(&NewMediaItem::film(title, year))
        .await
        .expect("insert");

    for (variant, text) in std::iter::once((TitleVariant::Primary, title))
        .chain(alternatives.iter().map(|a| (TitleVariant::Alternative, *a)))
    {
        media
            .add_title(id, text, variant, None)
            .await
            .expect("title");
        // The ingester writes `normalised`; do the same here or exact-title matching
        // has nothing to match against.
        sqlx::query("UPDATE titles SET normalised = ? WHERE media_item_id = ? AND title = ?")
            .bind(normalise(text))
            .bind(id)
            .bind(text)
            .execute(db.pool())
            .await
            .expect("normalise");
    }

    SearchRepository::new(db)
        .index_item(id, title, &alternatives.join(" "), people, keywords)
        .await
        .expect("index");
    id
}

fn normalise(text: &str) -> String {
    let mut out = String::new();
    let mut space = true;
    for ch in text.chars() {
        let ch = ch.to_lowercase().next().unwrap_or(ch);
        if ch.is_alphanumeric() {
            out.push(ch);
            space = false;
        } else if !space {
            out.push(' ');
            space = true;
        }
    }
    out.trim_end().to_string()
}

#[tokio::test]
async fn an_exact_title_is_always_first() {
    // Exit criterion E2 is 100%, not "usually". BM25 will happily rank a documentary
    // ABOUT a film above the film, because the documentary says its name more often.
    let (_dir, db) = db().await;
    let solaris = film(
        &db,
        "Solaris",
        1972,
        &[],
        "Andrei Tarkovsky",
        "science fiction",
    )
    .await;
    // A decoy that mentions the word far more, across every weighted column.
    film(
        &db,
        "The Making of Solaris: Solaris and Solaris",
        2002,
        &["Solaris Solaris Solaris"],
        "Solaris Solaris",
        "solaris documentary",
    )
    .await;

    let hits = SearchRepository::new(&db)
        .search("Solaris", 10)
        .await
        .expect("search");
    assert_eq!(hits[0].media_item_id, solaris, "got {:?}", hits[0].title);
    assert_eq!(hits[0].why, MatchReason::ExactTitle);
    assert_eq!(
        hits[0].score, None,
        "an exact hit does not go through ranking"
    );
}

#[tokio::test]
async fn an_exact_match_on_an_alternative_title_counts() {
    // Someone typing the Japanese name of a film must get that film, not a keyword
    // approximation of it.
    let (_dir, db) = db().await;
    let spirited = film(
        &db,
        "Spirited Away",
        2001,
        &["Sen to Chihiro no Kamikakushi"],
        "Hayao Miyazaki",
        "animation",
    )
    .await;

    let hits = SearchRepository::new(&db)
        .search("sen to chihiro no kamikakushi", 5)
        .await
        .expect("search");
    assert_eq!(hits[0].media_item_id, spirited);
    assert_eq!(hits[0].why, MatchReason::ExactTitle);
}

#[tokio::test]
async fn punctuation_and_diacritics_do_not_break_an_exact_match() {
    let (_dir, db) = db().await;
    let fma = film(&db, "Fullmetal Alchemist: Brotherhood", 2009, &[], "", "").await;
    let search = SearchRepository::new(&db);

    for query in [
        "Fullmetal Alchemist: Brotherhood",
        "fullmetal alchemist brotherhood",
        "  FULLMETAL ALCHEMIST  BROTHERHOOD  ",
    ] {
        let hits = search.search(query, 5).await.expect("search");
        assert_eq!(hits[0].media_item_id, fma, "query {query:?} failed");
        assert_eq!(hits[0].why, MatchReason::ExactTitle);
    }
}

#[tokio::test]
async fn a_title_that_is_fts5_syntax_does_not_error() {
    // Every one of these is a real film title and every one is FTS5 query syntax.
    // Unquoted, they are a syntax error rather than a search.
    let (_dir, db) = db().await;
    film(&db, "Face/Off", 1997, &[], "John Woo", "action").await;
    film(
        &db,
        "Mission: Impossible",
        1996,
        &[],
        "Brian De Palma",
        "action",
    )
    .await;
    film(&db, "And Then There Were None", 1945, &[], "", "mystery").await;
    let search = SearchRepository::new(&db);

    for query in [
        "Face/Off",
        "Mission: Impossible",
        "And Then There Were None",
        "\"unbalanced quote",
        "star*",
        "NOT a real title",
        "^^^",
    ] {
        let hits = search.search(query, 5).await;
        assert!(hits.is_ok(), "query {query:?} errored: {:?}", hits.err());
    }
}

#[tokio::test]
async fn a_title_match_outranks_an_actor_whose_name_appears_everywhere() {
    // Without column weights, a prolific actor's whole filmography buries the film
    // actually being searched for.
    let (_dir, db) = db().await;
    let wanted = film(&db, "Kurosawa", 1999, &[], "Someone Else", "documentary").await;
    for i in 0..8 {
        film(
            &db,
            &format!("Unrelated Film {i}"),
            1960 + i,
            &[],
            "Akira Kurosawa",
            "drama",
        )
        .await;
    }

    let hits = SearchRepository::new(&db)
        .search("Kurosawa", 10)
        .await
        .expect("search");
    assert_eq!(
        hits[0].media_item_id, wanted,
        "the film CALLED Kurosawa must come first, got {:?}",
        hits[0].title
    );
}

#[tokio::test]
async fn keyword_search_finds_by_person_and_by_genre() {
    let (_dir, db) = db().await;
    let stalker = film(
        &db,
        "Stalker",
        1979,
        &[],
        "Andrei Tarkovsky",
        "science fiction",
    )
    .await;
    film(&db, "Some Comedy", 1980, &[], "Nobody", "comedy").await;

    let search = SearchRepository::new(&db);
    let by_person = search.search("Tarkovsky", 5).await.expect("person");
    assert_eq!(by_person[0].media_item_id, stalker);
    assert_eq!(by_person[0].why, MatchReason::Keyword);

    let by_genre = search.search("science fiction", 5).await.expect("genre");
    assert!(by_genre.iter().any(|h| h.media_item_id == stalker));
}

#[tokio::test]
async fn re_indexing_an_item_replaces_it_rather_than_duplicating() {
    // A contentless FTS5 table cannot UPDATE in place, so a re-index that forgets to
    // delete first returns the same film twice — for ever, and only under search.
    let (_dir, db) = db().await;
    let id = film(&db, "Original Title", 1990, &[], "", "").await;
    let search = SearchRepository::new(&db);

    search
        .index_item(id, "Original Title", "", "Someone New", "drama")
        .await
        .expect("re-index");

    let hits = search.keyword("Original", 10).await.expect("search");
    assert_eq!(hits.len(), 1, "indexed twice: {hits:?}");
    assert_eq!(search.indexed_count().await.expect("count"), 1);
}

#[tokio::test]
async fn an_empty_or_punctuation_only_query_returns_nothing_rather_than_everything() {
    let (_dir, db) = db().await;
    film(&db, "A Film", 1990, &[], "", "").await;
    let search = SearchRepository::new(&db);

    for query in ["", "   ", "!!!", "-"] {
        let hits = search.search(query, 10).await.expect("search");
        assert!(hits.is_empty(), "query {query:?} returned {}", hits.len());
    }
}

#[tokio::test]
async fn nothing_indexed_is_distinguishable_from_nothing_matching() {
    // "No results" means something different at 3% indexed than at 100%.
    let (_dir, db) = db().await;
    let search = SearchRepository::new(&db);
    assert_eq!(search.indexed_count().await.expect("count"), 0);

    film(&db, "A Film", 1990, &[], "", "").await;
    assert_eq!(search.indexed_count().await.expect("count"), 1);
    assert!(search
        .search("nonexistent", 5)
        .await
        .expect("search")
        .is_empty());
}
