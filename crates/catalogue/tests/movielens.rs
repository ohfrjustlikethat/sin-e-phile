//! MovieLens joining.
//!
//! Every SQL statement in `movielens.rs` runs against a migrated database here
//! (ADR-0026's standing requirement).
//!
//! The archive is built in the test rather than downloaded. That is not only for
//! speed: `files.grouplens.org` served an expired certificate on 2026-09-04, and a
//! test suite that cannot run while a third party's certificate is broken is a test
//! suite that stops being run.

use std::io::Write;
use std::sync::Arc;

use sinephile_catalogue::movielens as ml;
use sinephile_catalogue::Job;
use sinephile_persistence::Db;

/// A MovieLens archive, shaped like the real one: everything nested under a directory
/// named after the release.
fn archive(dir: &std::path::Path, links: &str, ratings: &str) -> std::path::PathBuf {
    let path = dir.join("ml-test.zip");
    let file = std::fs::File::create(&path).expect("create");
    let mut zip = zip::ZipWriter::new(file);
    let options: zip::write::FileOptions<()> =
        zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    zip.start_file("ml-test/links.csv", options).expect("entry");
    zip.write_all(links.as_bytes()).expect("write");
    zip.start_file("ml-test/ratings.csv", options)
        .expect("entry");
    zip.write_all(ratings.as_bytes()).expect("write");
    // Every real archive has one; the join reads its Children's films.
    zip.start_file("ml-test/movies.csv", options)
        .expect("entry");
    zip.write_all(
        b"movieId,title,genres
1,Toy Story (1995),Animation|Children
2,Jumanji (1995),Adventure
",
    )
    .expect("write");
    zip.finish().expect("finish");
    path
}

fn links_csv() -> String {
    // imdbId has no `tt` and no zero padding, exactly as GroupLens writes it.
    "movieId,imdbId,tmdbId\n\
     1,114709,862\n\
     2,113497,8844\n\
     3,,\n\
     4,999999,111\n"
        .to_string()
}

fn ratings_csv() -> String {
    let mut out = String::from("userId,movieId,rating,timestamp\n");
    for user in 1..=3 {
        for movie in 1..=4 {
            out.push_str(&format!("{user},{movie},4.0,1112486027\n"));
        }
    }
    out
}

async fn film(db: &Db, tconst: &str, title: &str) -> i64 {
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO media_items (kind, primary_title) VALUES ('film', ?) RETURNING id",
    )
    .bind(title)
    .fetch_one(db.pool())
    .await
    .expect("insert film");
    sqlx::query(
        "INSERT INTO external_ids (media_item_id, source, external_id) VALUES (?, 'imdb', ?)",
    )
    .bind(id)
    .bind(tconst)
    .execute(db.pool())
    .await
    .expect("insert id");
    id
}

#[tokio::test]
async fn links_are_read_with_imdb_ids_unpadded() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = archive(dir.path(), &links_csv(), &ratings_csv());

    let links = ml::links(&path).expect("links");
    assert_eq!(links.len(), 3, "the row with a blank imdbId is skipped");
    assert_eq!(links[0].movielens_id, 1);
    assert_eq!(
        links[0].imdb_id, 114709,
        "links.csv stores 0114709 as 114709"
    );
}

#[tokio::test]
async fn a_changed_header_is_an_error_rather_than_silent_misparsing() {
    // If GroupLens reorders the columns, imdbId would silently become tmdbId and every
    // film would join to the wrong catalogue entry.
    let dir = tempfile::tempdir().expect("tempdir");
    let path = archive(
        dir.path(),
        "movieId,tmdbId,imdbId\n1,862,114709\n",
        &ratings_csv(),
    );
    assert!(ml::links(&path).is_err());
}

#[tokio::test]
async fn only_films_we_hold_are_joined() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Db::open_in(dir.path()).await.expect("open");
    let toy = film(&db, "tt0114709", "Toy Story").await;
    film(&db, "tt0113497", "Jumanji").await;

    let path = archive(dir.path(), &links_csv(), &ratings_csv());
    let links = ml::links(&path).expect("links");
    let catalogue = ml::catalogue_by_imdb(&db).await.expect("catalogue");

    let pairs: Vec<(i64, i64)> = links
        .iter()
        .filter_map(|l| catalogue.get(&l.imdb_id).map(|id| (*id, l.movielens_id)))
        .collect();
    assert_eq!(pairs.len(), 2, "tt9999999 is not in the catalogue");

    let mut job = Job::begin(&db, "movielens").await.expect("begin");
    ml::load(&mut job, Arc::new(pairs)).await.expect("load");
    job.finish().await.expect("finish");

    assert_eq!(ml::mapped(&db).await.expect("mapped"), 2);
    let id: String = sqlx::query_scalar(
        "SELECT external_id FROM external_ids WHERE media_item_id = ? AND source = 'movielens'",
    )
    .bind(toy)
    .fetch_one(db.pool())
    .await
    .expect("id");
    assert_eq!(id, "1");
}

#[tokio::test]
async fn two_movielens_ids_for_one_film_do_not_fail_the_batch() {
    // (source, external_id) is unique and (media_item_id, source) is the primary key.
    // MovieLens genuinely lists a film twice where IMDb later merged two entries, and
    // that must not take the whole load down.
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Db::open_in(dir.path()).await.expect("open");
    let toy = film(&db, "tt0114709", "Toy Story").await;

    let mut job = Job::begin(&db, "movielens").await.expect("begin");
    ml::load(&mut job, Arc::new(vec![(toy, 1), (toy, 4242)]))
        .await
        .expect("load survives the collision");
    job.finish().await.expect("finish");

    let ids: Vec<String> = sqlx::query_scalar(
        "SELECT external_id FROM external_ids WHERE source = 'movielens' ORDER BY external_id",
    )
    .fetch_all(db.pool())
    .await
    .expect("ids");
    assert_eq!(
        ids,
        vec!["1"],
        "the first one wins, and it is deterministic"
    );
}

#[tokio::test]
async fn the_load_is_re_runnable() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Db::open_in(dir.path()).await.expect("open");
    let toy = film(&db, "tt0114709", "Toy Story").await;

    for _ in 0..2 {
        Job::reset(&db, "movielens").await.expect("reset");
        let mut job = Job::begin(&db, "movielens").await.expect("begin");
        ml::load(&mut job, Arc::new(vec![(toy, 1)]))
            .await
            .expect("load");
        job.finish().await.expect("finish");
    }
    assert_eq!(ml::mapped(&db).await.expect("mapped"), 1);
}

#[tokio::test]
async fn ratings_are_counted_without_being_stored() {
    // The ratings are an INPUT to the on-device matrix (ADR-0019), not catalogue data.
    // 25 million of them would not fit in R4's headroom and do not need to.
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Db::open_in(dir.path()).await.expect("open");
    let path = archive(dir.path(), &links_csv(), &ratings_csv());

    let (count, seconds) = ml::scan_ratings(&path).expect("scan");
    assert_eq!(count, 12, "3 users x 4 films, header excluded");
    assert!(seconds >= 0.0);

    let tables: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name LIKE '%rating%'",
    )
    .fetch_all(db.pool())
    .await
    .expect("tables");
    // The one rating table is the viewer's own (migration 0020, Phase 11): a profile's
    // 1-5 stars, keyed by profile. Nothing shaped like MovieLens's (user, film, score)
    // rows exists to store 25 million of them in.
    assert_eq!(tables, ["ratings"], "only the viewer's own ratings");
    let columns: Vec<String> = sqlx::query_scalar("SELECT name FROM pragma_table_info('ratings')")
        .fetch_all(db.pool())
        .await
        .expect("columns");
    assert!(columns.contains(&"profile_id".to_string()), "{columns:?}");
}

#[tokio::test]
async fn films_filed_under_children_get_a_children_genre_and_no_others_do() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("ml-kids.zip");
    {
        let file = std::fs::File::create(&path).expect("create");
        let mut zip = zip::ZipWriter::new(file);
        let options: zip::write::FileOptions<()> = zip::write::FileOptions::default();
        zip.start_file("ml-kids/movies.csv", options)
            .expect("entry");
        // A quoted title with commas: the genres are the LAST field, the id the first.
        zip.write_all(
            b"movieId,title,genres\n\
              1,Toy Story (1995),Adventure|Animation|Children|Comedy|Fantasy\n\
              2,\"Party, Sausage (2016)\",Animation|Comedy\n\
              3,\"Lion King, The (1994)\",Adventure|Animation|Children|Drama|Musical|IMAX\n\
              4,Childrens Hour (1961),Drama\n",
        )
        .expect("write");
        zip.finish().expect("finish");
    }
    assert_eq!(ml::children(&path).expect("children"), vec![1, 3]);

    let db = Db::in_memory().await.expect("db");
    let toy = film(&db, "tt0114709", "Toy Story").await;
    let party = film(&db, "tt1700841", "Sausage Party").await;
    for (id, ml_id) in [(toy, "1"), (party, "2")] {
        sqlx::query(
            "INSERT INTO external_ids (media_item_id, source, external_id) VALUES (?, 'movielens', ?)",
        )
        .bind(id)
        .bind(ml_id)
        .execute(db.pool())
        .await
        .expect("movielens id");
    }
    let mut job = Job::begin(&db, "movielens").await.expect("job");
    // 3 is filed under Children but not in this catalogue: nothing to mark.
    ml::load_children(&mut job, Arc::new(vec![1, 3]))
        .await
        .expect("load_children");
    job.finish().await.expect("finish");

    let children = |id: i64| {
        let db = db.clone();
        async move {
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM media_genres mg JOIN genres g ON g.id = mg.genre_id
                 WHERE mg.media_item_id = ? AND g.name = 'Children'",
            )
            .bind(id)
            .fetch_one(db.pool())
            .await
            .expect("count")
        }
    };
    assert_eq!(children(toy).await, 1);
    assert_eq!(
        children(party).await,
        0,
        "same IMDb genres, not a children's film"
    );
}

#[test]
fn ratings_are_streamed_in_order_and_a_bad_line_is_skipped() {
    // Phase 17 builds the item-item matrix from this stream.
    let dir = tempfile::tempdir().expect("tempdir");
    let csv = "userId,movieId,rating,timestamp
1,10,4.5,1
1,20,3.0,2
not,a,rating,3
2,10,5.0,4
";
    let path = archive(dir.path(), &links_csv(), csv);
    let mut seen = Vec::new();
    let read =
        ml::ratings(&path, |user, movie, stars| seen.push((user, movie, stars))).expect("read");
    assert_eq!(read, 3);
    assert_eq!(seen, vec![(1, 10, 4.5), (1, 20, 3.0), (2, 10, 5.0)]);
}

#[tokio::test]
async fn the_matrix_is_built_over_the_films_we_hold_and_stored() {
    // Phase 17, 17.1: ratings -> likes over catalogue titles -> neighbours, stored.
    use sinephile_persistence::repositories::NeighbourRepository;
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Db::open_in(dir.path()).await.expect("open");
    let toy = film(&db, "tt0114709", "Toy Story").await;
    let jumanji = film(&db, "tt0113497", "Jumanji").await;

    // Six people like both films we hold; they also like movie 3, which we do not hold, and
    // movie 4's IMDb id is not in the catalogue. Three-star ratings are not likes.
    let mut csv = String::from(
        "userId,movieId,rating,timestamp
",
    );
    for user in 1..=6 {
        csv.push_str(&format!(
            "{user},1,4.5,1
{user},2,4.0,2
{user},3,5.0,3
{user},4,3.0,4
"
        ));
    }
    let path = archive(dir.path(), &links_csv(), &csv);
    let links = ml::links(&path).expect("links");
    let catalogue = ml::catalogue_by_imdb(&db).await.expect("catalogue");
    let pairs: Vec<(i64, i64)> = links
        .iter()
        .filter_map(|l| catalogue.get(&l.imdb_id).map(|id| (*id, l.movielens_id)))
        .collect();
    let mut job = Job::begin(&db, "movielens").await.expect("begin");
    ml::load(&mut job, Arc::new(pairs)).await.expect("load");
    job.finish().await.expect("finish");

    let built = sinephile_catalogue::neighbours::build(&db, &path)
        .await
        .expect("build");
    assert_eq!(built.ratings, 24);
    assert_eq!(built.likes, 12, "six people x the two films we hold");
    assert_eq!((built.people, built.titles, built.rows), (6, 2, 2));

    let repo = NeighbourRepository::new(&db);
    let toy_row = repo.of(toy, 10).await.expect("of");
    assert_eq!(toy_row.len(), 1);
    assert_eq!(toy_row[0].0, jumanji);
    assert!(toy_row[0].1 > 0.0);
    assert_eq!(repo.of(jumanji, 10).await.expect("of")[0].0, toy);
}

/// Serve `body` to every GET on a thread, counting the GETs. HEAD is not served here: the
/// launch check asks through the transport, which the test fakes.
fn serve(body: Vec<u8>) -> (String, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
    use std::io::{BufRead, BufReader};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let url = format!("http://{}/ml-latest.zip", listener.local_addr().unwrap());
    let gets = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counted = gets.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            while reader.read_line(&mut line).unwrap_or(0) > 0 && line != "\r\n" {
                line.clear();
            }
            counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(head.as_bytes());
            let _ = stream.write_all(&body);
        }
    });
    (url, gets)
}

#[tokio::test]
async fn the_matrix_follows_movielens_newest_release_at_every_launch() {
    // ADR-0054: one HEAD when nothing changed; a new release is fetched, joined and built;
    // offline keeps what there is; no validator, no download.
    use sinephile_catalogue::neighbours::{keep_current_from, Outcome};
    use sinephile_metadata_api::{FakeTransport, Response, TransportError};
    use std::sync::atomic::Ordering;
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Db::open_in(dir.path()).await.expect("open");
    film(&db, "tt0114709", "Toy Story").await;
    film(&db, "tt0113497", "Jumanji").await;
    let mut csv = String::from("userId,movieId,rating,timestamp\n");
    for user in 1..=6 {
        csv.push_str(&format!("{user},1,4.5,1\n{user},2,4.0,2\n"));
    }
    let archive_path = archive(dir.path(), &links_csv(), &csv);
    let (url, gets) = serve(std::fs::read(&archive_path).expect("read"));
    let datasets = dir.path().join("datasets");
    std::fs::create_dir_all(&datasets).unwrap();
    let head = |modified: &str| Response::new(200, "").with_header("last-modified", modified);
    let builds = std::sync::atomic::AtomicUsize::new(0);
    let on_build = || {
        builds.fetch_add(1, Ordering::SeqCst);
    };

    // No validator to be had: nothing fetched.
    let offline = FakeTransport::new();
    offline.push_error(TransportError::Network("offline".into()));
    let outcome = keep_current_from(&db, &datasets, &offline, &url, &on_build).await;
    assert_eq!(outcome, Outcome::Unreachable);
    assert_eq!(gets.load(Ordering::SeqCst), 0);

    // The first launch that can ask: fetched, joined, built, and the archive not kept.
    let first = FakeTransport::new();
    first.push(head("Tue, 01 Jul 2025 00:00:00 GMT"));
    let outcome = keep_current_from(&db, &datasets, &first, &url, &on_build).await;
    let Outcome::Built(built) = outcome else {
        panic!("expected a build, got {outcome:?}");
    };
    assert_eq!((built.titles, built.rows), (2, 2));
    assert_eq!(
        (gets.load(Ordering::SeqCst), builds.load(Ordering::SeqCst)),
        (1, 1)
    );
    assert!(
        !datasets.join("ml-latest.zip").exists(),
        "the archive is not kept"
    );

    // The next launch, the same release: one HEAD, nothing fetched.
    let same = FakeTransport::new();
    same.push(head("Tue, 01 Jul 2025 00:00:00 GMT"));
    let outcome = keep_current_from(&db, &datasets, &same, &url, &on_build).await;
    assert_eq!(outcome, Outcome::Current);
    assert_eq!((same.request_count(), gets.load(Ordering::SeqCst)), (1, 1));

    // GroupLens publishes anew: built again.
    let newer = FakeTransport::new();
    newer.push(head("Mon, 01 Jun 2026 00:00:00 GMT"));
    let outcome = keep_current_from(&db, &datasets, &newer, &url, &on_build).await;
    assert!(matches!(outcome, Outcome::Built(_)), "{outcome:?}");
    assert_eq!(
        (gets.load(Ordering::SeqCst), builds.load(Ordering::SeqCst)),
        (2, 2)
    );

    // Offline, with a matrix: it stays.
    let gone = FakeTransport::new();
    gone.push_error(TransportError::Network("offline".into()));
    let outcome = keep_current_from(&db, &datasets, &gone, &url, &on_build).await;
    assert_eq!(outcome, Outcome::Unreachable);
    assert_eq!(gets.load(Ordering::SeqCst), 2);
}
