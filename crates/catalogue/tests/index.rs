//! Deriving the index from a downloaded artefact, and refusing when the catalogue has
//! moved under it.
//!
//! The hazard these exercise is silent by nature: the artefact is positional, so a title
//! entering the core tier partway along shifts every vector after it onto a different
//! film, and every offset still reads a *valid* vector. Nothing throws. Search simply
//! starts returning the wrong titles, confidently.
//!
//! `VectorIndex::build` cannot see it — the count merely goes up, exactly as it does for
//! a harmless append — so it is checked here, by content.

use sinephile_catalogue::embed::{self, DocumentEmbedder};
use sinephile_catalogue::index;
use sinephile_catalogue::{Job, JobError};
use sinephile_embedding::Artefact;
use sinephile_persistence::repositories::CatalogueRepository;
use sinephile_persistence::{Db, NewMediaItem};

/// Deterministic vectors derived from the text, so a document that changed produces a
/// detectably different vector — which is the entire mechanism under test.
struct Fake {
    dimension: u16,
}

impl DocumentEmbedder for Fake {
    fn identity(&self) -> &str {
        "fake-model-int8"
    }

    fn dimension(&self) -> u16 {
        self.dimension
    }

    fn embed(&mut self, text: &str) -> Result<Vec<f32>, JobError> {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in text.as_bytes() {
            hash ^= *byte as u64;
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        // splitmix64 per dimension, so every dimension depends on the whole hash. The
        // first version multiplied `hash + i` by a 32-bit constant and kept the top bits,
        // which came out the same for every text: every vector was [127; 8], and the
        // sampled re-embed in `verify_prefix` could never fail in these tests.
        let raw: Vec<f32> = (0..self.dimension)
            .map(|i| {
                let mut z = hash ^ (u64::from(i) + 1).wrapping_mul(0x9e37_79b9_7f4a_7c15);
                z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
                z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
                z ^= z >> 31;
                (z >> 11) as f32 / (1u64 << 53) as f32 * 2.0 - 1.0
            })
            .collect();
        let norm = raw.iter().map(|v| v * v).sum::<f32>().sqrt().max(1e-12);
        Ok(raw.into_iter().map(|v| v / norm).collect())
    }
}

/// `count` core films, plus `spare` films left OUT of the core tier — the ones a later
/// refresh could promote.
async fn catalogue(db: &Db, count: usize, spare: usize) -> Vec<i64> {
    let media = sinephile_persistence::repositories::MediaRepository::new(db);
    let mut spares = Vec::new();
    for i in 0..count + spare {
        let id = media
            .insert(&NewMediaItem::film(format!("Film {i:03}"), 1950 + i as i64))
            .await
            .expect("insert");
        // A synopsis long enough to clear MIN_SYNOPSIS, so the title is actually indexed
        // rather than skipped as too sparse to embed well.
        sqlx::query("UPDATE media_items SET synopsis = ? WHERE id = ?")
            .bind(format!(
                "A film about number {i}. {}",
                "Detail. ".repeat(50)
            ))
            .bind(id)
            .execute(db.pool())
            .await
            .expect("synopsis");

        if i % 2 == 0 || spares.len() >= spare {
            sqlx::query("UPDATE media_items SET in_core = 1 WHERE id = ?")
                .bind(id)
                .execute(db.pool())
                .await
                .expect("core");
        } else {
            spares.push(id);
        }
    }
    spares
}

async fn produce(db: &Db, embedder: &mut dyn DocumentEmbedder, path: &std::path::Path) {
    let mut job = Job::begin(db, "embed").await.expect("begin");
    embed::produce(&mut job, db, embedder, path, "2026-09-22", "test-source")
        .await
        .expect("produce");
    job.finish().await.expect("finish");
}

fn artefact_at(path: &std::path::Path) -> Artefact {
    let mut file = std::fs::File::open(path).expect("open");
    Artefact::read(&mut file).expect("read")
}

#[tokio::test]
async fn an_artefact_matching_its_catalogue_verifies() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Db::open_in(dir.path()).await.expect("open");
    catalogue(&db, 20, 0).await;

    let path = sinephile_embedding::artefact_path(dir.path());
    produce(&db, &mut Fake { dimension: 8 }, &path).await;

    let ids = CatalogueRepository::new(&db).core_ids().await.expect("ids");
    index::verify_prefix(&db, &mut Fake { dimension: 8 }, &artefact_at(&path), &ids)
        .await
        .expect("an untouched catalogue must verify");
}

#[tokio::test]
async fn a_catalogue_that_grew_at_the_tail_still_verifies() {
    // The ordinary case since subtask 5.7: the app refreshed itself and inserted new
    // titles, which take higher ids and therefore land AFTER every position the artefact
    // covers. Nothing has shifted, and refusing this is what made the download inert.
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Db::open_in(dir.path()).await.expect("open");
    catalogue(&db, 20, 0).await;

    let path = sinephile_embedding::artefact_path(dir.path());
    produce(&db, &mut Fake { dimension: 8 }, &path).await;
    let artefact = artefact_at(&path);

    let media = sinephile_persistence::repositories::MediaRepository::new(&db);
    for i in 0..9 {
        let id = media
            .insert(&NewMediaItem::film(format!("Arrived Later {i}"), 2026))
            .await
            .expect("insert");
        sqlx::query("UPDATE media_items SET in_core = 1, synopsis = ? WHERE id = ?")
            .bind("Freshly refreshed. ".repeat(30))
            .bind(id)
            .execute(db.pool())
            .await
            .expect("core");
    }

    let ids = CatalogueRepository::new(&db).core_ids().await.expect("ids");
    assert!(
        ids.len() > artefact.header.count as usize,
        "the catalogue must have grown"
    );

    index::verify_prefix(&db, &mut Fake { dimension: 8 }, &artefact, &ids)
        .await
        .expect("growth at the tail shifts nothing and must be allowed");
}

#[tokio::test]
async fn a_title_promoted_into_the_middle_of_the_core_tier_is_refused() {
    // THE FAILURE THIS MODULE EXISTS FOR, and the one length cannot see.
    //
    // `in_core` is a function of vote count, and the ratings refresh updates votes. The
    // day it also recomputes membership, a title crossing the popularity threshold enters
    // the sequence at its OWN id — in the middle — and every vector after it belongs to a
    // different film. The count goes UP, exactly as a harmless append does.
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Db::open_in(dir.path()).await.expect("open");
    let spares = catalogue(&db, 20, 10).await;

    let path = sinephile_embedding::artefact_path(dir.path());
    produce(&db, &mut Fake { dimension: 8 }, &path).await;
    let artefact = artefact_at(&path);

    // One title, from the middle, promoted after the artefact was built.
    let promoted = spares[spares.len() / 2];
    sqlx::query("UPDATE media_items SET in_core = 1 WHERE id = ?")
        .bind(promoted)
        .execute(db.pool())
        .await
        .expect("promote");

    let ids = CatalogueRepository::new(&db).core_ids().await.expect("ids");
    assert_eq!(
        ids.len(),
        artefact.header.count as usize + 1,
        "one more id, which is all a length check would ever see"
    );

    let err = index::verify_prefix(&db, &mut Fake { dimension: 8 }, &artefact, &ids)
        .await
        .expect_err("a shifted catalogue must be refused");
    let message = err.to_string();
    assert!(
        message.contains("are not this catalogue's first"),
        "refused for the wrong reason: {message}"
    );
}

#[tokio::test]
async fn a_title_leaving_and_another_entering_below_the_watermark_is_refused() {
    // D45 first proposed a watermark alone, and this is what it misses: one title leaves
    // the core tier, another enters, both below the last position. The count and the
    // last id are unchanged; every position between the two names a different film.
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Db::open_in(dir.path()).await.expect("open");
    let spares = catalogue(&db, 20, 10).await;

    let path = sinephile_embedding::artefact_path(dir.path());
    produce(&db, &mut Fake { dimension: 8 }, &path).await;
    let artefact = artefact_at(&path);

    let before = CatalogueRepository::new(&db).core_ids().await.expect("ids");
    let leaving = before[1];
    let entering = spares[0];
    assert!(
        entering < artefact.header.watermark,
        "the swap must sit below the watermark"
    );
    for (id, in_core) in [(leaving, 0), (entering, 1)] {
        sqlx::query("UPDATE media_items SET in_core = ? WHERE id = ?")
            .bind(in_core)
            .bind(id)
            .execute(db.pool())
            .await
            .expect("swap");
    }

    let ids = CatalogueRepository::new(&db).core_ids().await.expect("ids");
    let count = artefact.header.count as usize;
    assert_eq!(ids.len(), before.len(), "same length");
    assert_eq!(ids[count - 1], artefact.header.watermark, "same last id");

    let err = index::verify_prefix(&db, &mut Fake { dimension: 8 }, &artefact, &ids)
        .await
        .expect_err("a swap below the watermark must be refused");
    let message = err.to_string();
    assert!(
        message.contains("a title left the core tier and another entered"),
        "refused for the wrong reason: {message}"
    );
}

#[tokio::test]
async fn a_covered_title_whose_text_changed_is_refused_by_the_sampled_re_embed() {
    // The same ids, so the exact check passes, but the sentence behind the last position
    // is different. That is what the sampled re-embed is still for.
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Db::open_in(dir.path()).await.expect("open");
    catalogue(&db, 20, 0).await;

    let path = sinephile_embedding::artefact_path(dir.path());
    produce(&db, &mut Fake { dimension: 8 }, &path).await;
    let artefact = artefact_at(&path);

    sqlx::query("UPDATE media_items SET synopsis = ? WHERE id = ?")
        .bind(format!(
            "A different film entirely. {}",
            "Other. ".repeat(60)
        ))
        .bind(artefact.header.watermark)
        .execute(db.pool())
        .await
        .expect("rewrite");

    let ids = CatalogueRepository::new(&db).core_ids().await.expect("ids");
    let err = index::verify_prefix(&db, &mut Fake { dimension: 8 }, &artefact, &ids)
        .await
        .expect_err("changed text under the same id must be refused");
    let message = err.to_string();
    assert!(
        message.contains("does not match the artefact"),
        "refused for the wrong reason: {message}"
    );
}

#[tokio::test]
async fn an_artefact_from_a_different_model_is_refused_before_anything_is_embedded() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Db::open_in(dir.path()).await.expect("open");
    catalogue(&db, 8, 0).await;

    let path = sinephile_embedding::artefact_path(dir.path());
    produce(&db, &mut Fake { dimension: 8 }, &path).await;

    struct Other;
    impl DocumentEmbedder for Other {
        fn identity(&self) -> &str {
            "a-completely-different-model-int8"
        }
        fn dimension(&self) -> u16 {
            8
        }
        fn embed(&mut self, _: &str) -> Result<Vec<f32>, JobError> {
            panic!("must refuse on identity before embedding anything");
        }
    }

    let ids = CatalogueRepository::new(&db).core_ids().await.expect("ids");
    let err = index::verify_prefix(&db, &mut Other, &artefact_at(&path), &ids)
        .await
        .expect_err("a different model is a different space");
    assert!(err.to_string().contains("different spaces"), "{err}");
}

#[tokio::test]
async fn a_catalogue_missing_titles_is_refused() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Db::open_in(dir.path()).await.expect("open");
    catalogue(&db, 20, 0).await;

    let path = sinephile_embedding::artefact_path(dir.path());
    produce(&db, &mut Fake { dimension: 8 }, &path).await;
    let artefact = artefact_at(&path);

    let ids = CatalogueRepository::new(&db).core_ids().await.expect("ids");
    let short = &ids[..ids.len() - 3];

    let err = index::verify_prefix(&db, &mut Fake { dimension: 8 }, &artefact, short)
        .await
        .expect_err("removed titles shift every position after the gap");
    assert!(err.to_string().contains("positions have shifted"), "{err}");
}

#[tokio::test]
async fn build_writes_an_index_and_leaves_no_part_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Db::open_in(dir.path()).await.expect("open");
    catalogue(&db, 24, 0).await;

    let path = sinephile_embedding::artefact_path(dir.path());
    produce(&db, &mut Fake { dimension: 8 }, &path).await;

    let built = index::build(&db, &mut Fake { dimension: 8 }, dir.path(), |_, _| {})
        .await
        .expect("build");

    assert_eq!(built.indexed, 24);
    assert_eq!(built.beyond_artefact, 0);
    assert!(built.bytes > 0, "an index that saved nothing is not saved");

    let final_path = sinephile_vector_index::index_path(dir.path());
    assert!(final_path.is_file(), "the app looks for exactly this path");
    assert!(
        !final_path.with_extension("usearch.part").exists(),
        "a .part left behind is a half-built graph the app would open and search"
    );
}
