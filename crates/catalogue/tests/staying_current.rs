//! Phase 17 E7 (ADR-0054): a title the catalogue gains at a launch can be recommended on that
//! launch, with a reason that is true. End to end: a fixture refresh adds a film through the
//! refresh's own load; the meaning job describes it (Wikipedia, faked) and gives it a vector
//! in the companion graph beside the published one; Home's taste rail, over both graphs,
//! reaches it and says why.

use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::sync::Arc;

use sinephile_catalogue::embed::DocumentEmbedder;
use sinephile_catalogue::imdb::CatalogueScope;
use sinephile_catalogue::meaning::{self, Sources};
use sinephile_catalogue::{load, refresh, Job, JobError};
use sinephile_embedding::{artefact, quantise, Artefact, Header, Quantisation};
use sinephile_metadata_api::wikipedia::Wikipedia;
use sinephile_metadata_api::{FakeTransport, Response};
use sinephile_persistence::repositories::ProfileRepository;
use sinephile_persistence::Db;
use sinephile_recommend::rank::Reason;
use sinephile_taste::axes::Axes;
use sinephile_taste::hybrid::{self, Meaning};
use sinephile_vector_index::{Companion, VectorIndex};

const DIM: usize = 4;

/// The published file's vectors, by title: the director's films point one way, the
/// blockbuster another.
fn published_vector(title: &str) -> [i8; DIM] {
    match title {
        "Stalker" => [120, 20, 0, 0],
        "Solaris" => [115, 35, 0, 0],
        "Mirror" => [110, 30, 10, 0],
        _ => [0, 0, 120, 30],
    }
}

/// The model, faked: a document about the Zone points where the director's films do.
struct Fake;

impl DocumentEmbedder for Fake {
    fn identity(&self) -> &str {
        "fake-model-int8"
    }
    fn dimension(&self) -> u16 {
        DIM as u16
    }
    fn embed(&mut self, text: &str) -> Result<Vec<f32>, JobError> {
        Ok(if text.contains("Zone") {
            vec![0.95, 0.2, 0.05, 0.0]
        } else {
            vec![0.0, 0.1, 0.9, 0.3]
        })
    }
}

/// The meaning index as the app opens it (both graphs), for the rail.
struct Graph(VectorIndex);

impl Meaning for Graph {
    fn nearest(&self, direction: &[f32], k: usize, exclude: &HashSet<i64>) -> Vec<i64> {
        self.0
            .search(&quantise(direction).values, k + exclude.len())
            .expect("search")
            .into_iter()
            .map(|n| n.media_item_id)
            .filter(|id| !exclude.contains(id))
            .take(k)
            .collect()
    }
    fn vector(&self, id: i64) -> Option<Vec<f32>> {
        self.0
            .vector(id)
            .map(|v| v.iter().map(|x| f32::from(*x)).collect())
    }
}

fn basics(rows: &[(u32, &str, u32)]) -> String {
    let null = "\\N";
    let mut out = vec![
        "tconst\ttitleType\tprimaryTitle\toriginalTitle\tisAdult\tstartYear\tendYear\truntimeMinutes\tgenres"
            .to_string(),
    ];
    for (id, title, year) in rows {
        out.push(format!(
            "tt{id:07}\tmovie\t{title}\t{title}\t0\t{year}\t{null}\t160\tDrama"
        ));
    }
    out.join("\n") + "\n"
}

/// Load titles through the refresh's own path: everything past the watermark.
async fn load(db: &Db, dir: &std::path::Path, name: &str, rows: &[(u32, &str, u32)]) {
    let path = dir.join(name);
    let mut gz = flate2::write::GzEncoder::new(
        std::fs::File::create(&path).expect("create"),
        flate2::Compression::fast(),
    );
    gz.write_all(basics(rows).as_bytes()).expect("write");
    gz.finish().expect("finish");
    let votes: HashMap<u32, i64> = rows.iter().map(|(id, _, _)| (*id, 50_000)).collect();
    let averages: HashMap<u32, i64> = rows.iter().map(|(id, _, _)| (*id, 80)).collect();
    let watermark = refresh::watermark(db).await.expect("watermark");
    let mut job = Job::begin(db, name).await.expect("job");
    load::load_titles(
        &mut job,
        path,
        Arc::new(votes),
        Arc::new(averages),
        CatalogueScope::DEFAULT,
        watermark,
    )
    .await
    .expect("load");
    job.finish().await.expect("finish");
}

async fn id_of(db: &Db, title: &str) -> i64 {
    sqlx::query_scalar("SELECT id FROM media_items WHERE primary_title = ?")
        .bind(title)
        .fetch_one(db.pool())
        .await
        .expect(title)
}

#[tokio::test]
async fn a_title_gained_at_a_launch_is_recommended_on_that_launch_with_a_true_reason() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Db::open_in(dir.path()).await.expect("open");

    // The catalogue as the published meaning file knew it.
    let published = [
        (100u32, "Stalker", 1979u32),
        (101, "Solaris", 1972),
        (102, "Mirror", 1975),
        (103, "Blockbuster", 2008),
    ];
    load(&db, dir.path(), "basics.tsv.gz", &published).await;
    let mut ids = Vec::new();
    let mut vectors = Vec::new();
    for (_, title, _) in published {
        let id = id_of(&db, title).await;
        sqlx::query("UPDATE media_items SET synopsis = ?, in_core = 1 WHERE id = ?")
            .bind(format!(
                "{title}. {}",
                "A long description of the film. ".repeat(12)
            ))
            .bind(id)
            .execute(db.pool())
            .await
            .expect("synopsis");
        ids.push(Some(id));
        vectors.push(published_vector(title).to_vec());
    }
    let watermark = ids.iter().flatten().copied().max().expect("ids");
    let header = Header {
        model: "fake-model-int8".into(),
        dimension: DIM as u16,
        quantisation: Quantisation::Int8,
        document_builder_version: 1,
        snapshot_date: "2026-11-01".into(),
        text_source: "wikipedia".into(),
        count: vectors.len() as u64,
        watermark,
        id_digest: [0; 32],
    };
    let mut bytes = Vec::new();
    artefact::write(&mut bytes, &header, vectors).expect("artefact");
    let index_path = dir.path().join("vector-index-fake.usearch");
    VectorIndex::build(
        &Artefact::read(&mut bytes.as_slice()).expect("read"),
        &ids,
        &index_path,
        |_| {},
    )
    .expect("published graph");

    // Who made what, and a profile that loved two of the director's films.
    let tarkovsky: i64 =
        sqlx::query_scalar("INSERT INTO people (name) VALUES ('Andrei Tarkovsky') RETURNING id")
            .fetch_one(db.pool())
            .await
            .expect("person");
    let credit = |item: i64| {
        let db = &db;
        async move {
            sqlx::query(
                "INSERT INTO credits (media_item_id, person_id, role) VALUES (?, ?, 'director')",
            )
            .bind(item)
            .bind(tarkovsky)
            .execute(db.pool())
            .await
            .expect("credit");
        }
    };
    let (stalker, solaris, mirror) = (
        id_of(&db, "Stalker").await,
        id_of(&db, "Solaris").await,
        id_of(&db, "Mirror").await,
    );
    for id in [stalker, solaris, mirror] {
        credit(id).await;
    }
    let profiles = ProfileRepository::new(&db);
    let me = profiles.ensure_default().await.expect("profile");
    for id in [stalker, solaris] {
        profiles.rate(me, id, Some(5)).await.expect("rate");
    }

    // The launch: the catalogue's refresh gains a film, credited as the credits step would.
    load(
        &db,
        dir.path(),
        "refresh.tsv.gz",
        &[(200, "The Zone Returns", 2026)],
    )
    .await;
    let new = id_of(&db, "The Zone Returns").await;
    assert!(new > watermark, "past the published file");
    credit(new).await;

    // Then its meaning: Wikipedia describes it, the model embeds it, the companion keeps it.
    let wiki_fake = FakeTransport::new();
    wiki_fake.push(Response::new(
        200,
        format!(
            r#"{{"query":{{"pages":[{{"title":"The Zone Returns","extract":"The Zone Returns is a 2026 film. {}"}}]}}}}"#,
            "Three travellers cross the Zone again. ".repeat(10)
        ),
    ));
    wiki_fake.push(Response::new(
        200,
        r#"{"results":{"bindings":[{"imdb":{"value":"tt0000200"},"article":{"value":"https://en.wikipedia.org/wiki/The_Zone_Returns"}}]}}"#,
    ));
    let wikipedia = Wikipedia::new(&wiki_fake).await;
    let mut companion = Companion::open(&index_path, DIM).expect("companion");
    let report = meaning::keep_current(
        &db,
        &mut Fake,
        &mut companion,
        watermark,
        &Sources {
            wikipedia: Some(&wikipedia),
            ..Default::default()
        },
        meaning::PER_LAUNCH,
    )
    .await
    .expect("meaning");
    assert_eq!((report.wikipedia, report.embedded), (1, 1), "{report:?}");

    // Home's taste rail, over both graphs, as the app opens them.
    let mut graph = Graph(VectorIndex::open(&index_path).expect("open"));
    assert!(graph.vector(new).is_some(), "the new title has its meaning");
    let liked = [(stalker, 1.0), (solaris, 1.0)];
    let with_vectors: Vec<(i64, Vec<f32>, f64)> = liked
        .iter()
        .map(|(id, w)| (*id, graph.vector(*id).expect("vector"), *w))
        .collect();
    let modes = sinephile_taste::modes::modes(&with_vectors);
    let met: HashSet<i64> = [stalker, solaris].into();
    let (picks, makers) = hybrid::rail(
        &db,
        &mut graph,
        &modes,
        &met,
        &liked,
        &Axes::default(),
        &HashSet::new(),
        10,
    )
    .await
    .expect("rail");

    let pick = picks
        .iter()
        .find(|p| p.id == new)
        .unwrap_or_else(|| panic!("the new title is in the rail: {picks:?}"));
    // Its reason is true: the maker of what was loved, or closeness to what was loved.
    match &pick.reason {
        Reason::MadeBy { person, loved } => {
            assert_eq!(*person, tarkovsky);
            assert!(
                loved.iter().all(|l| [stalker, solaris].contains(l)),
                "{loved:?}"
            );
        }
        Reason::Mode(_) => {}
        other => panic!("not a reason the new title has: {other:?}"),
    }
    let titles: HashMap<i64, String> = [(stalker, "Stalker"), (solaris, "Solaris")]
        .map(|(id, t)| (id, t.to_string()))
        .into();
    let said = hybrid::note(pick, &modes, &titles, &makers);
    assert!(
        said == "By Andrei Tarkovsky, who made Stalker and Solaris"
            || said == "By Andrei Tarkovsky, who made Solaris and Stalker"
            || said.starts_with("Close to Stalker")
            || said.starts_with("Close to Solaris"),
        "{said}"
    );
}
