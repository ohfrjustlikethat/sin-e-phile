//! The AniList client, against a fake transport.
//!
//! These are the paths a live service will not produce on demand: a 429 with a
//! `Retry-After`, a malformed body, a GraphQL error, a transport failure that
//! recovers on the second attempt. A client tested only against the real API is a
//! client whose error handling has never run.

use std::time::Duration;

use sinephile_metadata_api::anilist::AniList;
use sinephile_metadata_api::{FakeTransport, Response};

fn media_json(id: i64, romaji: &str, english: &str, native: &str, format: &str) -> String {
    serde_json::json!({
        "data": { "Media": {
            "id": id,
            "idMal": 11061,
            "title": { "romaji": romaji, "english": english, "native": native },
            "format": format,
            "status": "FINISHED",
            "episodes": 148,
            "seasonYear": 2011,
            "nextAiringEpisode": null,
        }}
    })
    .to_string()
}

#[tokio::test]
async fn the_three_title_forms_come_back_as_facts() {
    // §6.2 names romaji, english and native. From AniList they are asserted, not
    // inferred from the script the way title.akas forces.
    let transport = FakeTransport::new();
    transport.push(Response::new(
        200,
        media_json(
            5114,
            "Hagane no Renkinjutsushi",
            "Fullmetal Alchemist",
            "鋼の錬金術師",
            "TV",
        ),
    ));

    let media = AniList::new(&transport)
        .await
        .media(5114)
        .await
        .expect("query")
        .expect("found");

    assert_eq!(
        media.title.romaji.as_deref(),
        Some("Hagane no Renkinjutsushi")
    );
    assert_eq!(media.title.english.as_deref(), Some("Fullmetal Alchemist"));
    assert_eq!(media.title.native.as_deref(), Some("鋼の錬金術師"));
    assert_eq!(
        media.id_mal,
        Some(11061),
        "the MAL cross-mapping comes free"
    );
    assert_eq!(media.media_kind(), "anime_series");
}

#[tokio::test]
async fn a_movie_is_classified_as_anime_film() {
    let transport = FakeTransport::new();
    transport.push(Response::new(
        200,
        media_json(
            199,
            "Sen to Chihiro",
            "Spirited Away",
            "千と千尋の神隠し",
            "MOVIE",
        ),
    ));

    let media = AniList::new(&transport)
        .await
        .media(199)
        .await
        .expect("query")
        .expect("found");
    assert_eq!(media.media_kind(), "anime_film");
}

#[tokio::test]
async fn an_airing_schedule_comes_back_when_a_series_is_releasing() {
    // The missing half of weekly-TV freshness: knowing an episode is coming before
    // IMDb lists it.
    let transport = FakeTransport::new();
    transport.push(Response::new(
        200,
        serde_json::json!({
            "data": { "Media": {
                "id": 1, "idMal": null,
                "title": { "romaji": "A Series", "english": null, "native": null },
                "format": "TV", "status": "RELEASING", "episodes": 12, "seasonYear": 2026,
                "nextAiringEpisode": {
                    "airingAt": 1_790_000_000_i64,
                    "timeUntilAiring": 86_400,
                    "episode": 7
                }
            }}
        })
        .to_string(),
    ));

    let media = AniList::new(&transport)
        .await
        .media(1)
        .await
        .expect("query")
        .expect("found");

    assert!(media.is_releasing());
    let airing = media
        .next_airing
        .expect("a releasing series has a next episode");
    assert_eq!(airing.episode, 7);
    assert_eq!(airing.time_until_airing, 86_400);
}

#[tokio::test]
async fn a_finished_series_has_no_next_episode_and_that_is_not_a_gap() {
    let transport = FakeTransport::new();
    transport.push(Response::new(200, media_json(5114, "a", "b", "c", "TV")));

    let media = AniList::new(&transport)
        .await
        .media(5114)
        .await
        .expect("query")
        .expect("found");
    assert!(media.next_airing.is_none());
    assert!(!media.is_releasing());
}

#[tokio::test]
async fn a_miss_is_none_rather_than_an_error() {
    // A catalogue full of anime we do not have is normal. Treating every miss as a
    // failure would make enrichment look broken.
    let transport = FakeTransport::new();
    transport.push(Response::new(
        404,
        r#"{"errors":[{"message":"Not Found."}]}"#,
    ));

    let result = AniList::new(&transport)
        .await
        .media(999_999)
        .await
        .expect("query");
    assert!(result.is_none());
}

#[tokio::test]
async fn a_graphql_not_found_error_is_also_a_miss() {
    // AniList reports it both ways depending on the query shape.
    let transport = FakeTransport::new();
    transport.push(Response::new(
        200,
        r#"{"errors":[{"message":"Not Found."}],"data":null}"#,
    ));

    let result = AniList::new(&transport)
        .await
        .search("nothing at all")
        .await
        .expect("query");
    assert!(result.is_none());
}

#[tokio::test]
async fn another_graphql_error_is_reported_rather_than_swallowed() {
    let transport = FakeTransport::new();
    transport.push(Response::new(
        200,
        r#"{"errors":[{"message":"Invalid token"}],"data":null}"#,
    ));

    let error = AniList::new(&transport)
        .await
        .media(1)
        .await
        .expect_err("should surface")
        .to_string();
    assert!(error.contains("Invalid token"), "unhelpful: {error}");
}

#[tokio::test]
async fn a_malformed_body_names_itself_rather_than_panicking() {
    let transport = FakeTransport::new();
    transport.push(Response::new(200, "<html>502 Bad Gateway</html>"));

    let error = AniList::new(&transport)
        .await
        .media(1)
        .await
        .expect_err("should surface")
        .to_string();
    assert!(error.contains("not the shape"), "unhelpful: {error}");
    assert!(
        error.contains("502"),
        "the body is quoted so it can be diagnosed: {error}"
    );
}

#[tokio::test(start_paused = true)]
async fn a_server_error_is_retried_and_can_succeed() {
    // Queued LIFO by the fake, so this is: 500 first, then the good one.
    let transport = FakeTransport::new();
    transport.push(Response::new(200, media_json(1, "a", "b", "c", "TV")));
    transport.push(Response::new(500, "upstream is unwell"));

    let media = AniList::new(&transport)
        .await
        .media(1)
        .await
        .expect("query")
        .expect("found");
    assert_eq!(media.id, 1);
    assert_eq!(transport.request_count(), 2, "it retried exactly once");
}

#[tokio::test(start_paused = true)]
async fn a_404_is_not_retried_because_it_will_not_be_there_later() {
    // Retrying a 404 spends the rate-limit allowance a request that WOULD succeed
    // needed.
    let transport = FakeTransport::new();
    transport.push(Response::new(
        404,
        r#"{"errors":[{"message":"Not Found."}]}"#,
    ));

    let result = AniList::new(&transport)
        .await
        .media(1)
        .await
        .expect("query");
    assert!(result.is_none());
    assert_eq!(transport.request_count(), 1);
}

#[tokio::test(start_paused = true)]
async fn a_transport_failure_is_retried() {
    use sinephile_metadata_api::TransportError;
    let transport = FakeTransport::new();
    transport.push(Response::new(200, media_json(1, "a", "b", "c", "TV")));
    transport.push_error(TransportError::Network("connection refused".into()));

    let media = AniList::new(&transport)
        .await
        .media(1)
        .await
        .expect("query")
        .expect("found");
    assert_eq!(media.id, 1);
    assert_eq!(transport.request_count(), 2);
}

#[tokio::test(start_paused = true)]
async fn a_persistent_failure_gives_up_rather_than_hammering() {
    let transport = FakeTransport::new();
    for _ in 0..10 {
        transport.push(Response::new(503, "still unwell"));
    }

    let error = AniList::new(&transport)
        .await
        .media(1)
        .await
        .expect_err("gives up");
    assert!(error.to_string().contains("503"), "{error}");
    assert_eq!(
        transport.request_count(),
        4,
        "max_attempts is a ceiling, not a suggestion"
    );
}

#[tokio::test(start_paused = true)]
async fn a_429_makes_the_next_request_wait_for_the_server_named_delay() {
    // Honouring Retry-After is not politeness; ignoring it turns a 429 into a ban.
    let transport = FakeTransport::new();
    transport.push(Response::new(200, media_json(1, "a", "b", "c", "TV")));
    transport.push(Response::new(429, "slow down").with_header("Retry-After", "30"));

    let start = tokio::time::Instant::now();
    let media = AniList::new(&transport)
        .await
        .media(1)
        .await
        .expect("query")
        .expect("found");

    assert_eq!(media.id, 1);
    assert!(
        start.elapsed() >= Duration::from_secs(30),
        "the retry ignored Retry-After: {:?}",
        start.elapsed()
    );
}

#[tokio::test]
async fn the_request_is_a_json_post_to_the_graphql_endpoint() {
    let transport = FakeTransport::new();
    transport.push(Response::new(200, media_json(1, "a", "b", "c", "TV")));
    AniList::new(&transport)
        .await
        .search("cowboy bebop")
        .await
        .expect("query");

    let request = transport.last_request().expect("recorded");
    assert_eq!(request.url, "https://graphql.anilist.co");
    assert_eq!(request.host(), "graphql.anilist.co");

    let body = request.body.expect("a GraphQL POST has a body");
    let parsed: serde_json::Value = serde_json::from_str(&body).expect("valid JSON");
    assert!(parsed["query"].as_str().expect("query").contains("Media"));
    assert_eq!(parsed["variables"]["q"], "cowboy bebop");
}

#[tokio::test]
async fn no_api_key_is_sent_because_anilist_does_not_need_one() {
    // The reason this client is first: it works for everybody on first run
    // (ADR-0027).
    let transport = FakeTransport::new();
    transport.push(Response::new(200, media_json(1, "a", "b", "c", "TV")));
    AniList::new(&transport)
        .await
        .media(1)
        .await
        .expect("query");

    let request = transport.last_request().expect("recorded");
    for (name, value) in &request.headers {
        let lower = name.to_ascii_lowercase();
        assert!(
            !lower.contains("authorization") && !lower.contains("key"),
            "unexpected credential header {name}: {value}"
        );
    }
}
