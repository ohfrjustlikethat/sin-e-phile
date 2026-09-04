//! The artwork cache, against a real directory.

use sinephile_artwork::ArtworkCache;

/// A JPEG of a given size, distinct per `seed` so entries differ.
fn jpeg(width: u32, height: u32, seed: u8) -> Vec<u8> {
    let mut buf = image::RgbImage::new(width, height);
    for (x, y, pixel) in buf.enumerate_pixels_mut() {
        *pixel = image::Rgb([
            ((x * 7 + seed as u32) % 256) as u8,
            ((y * 11 + seed as u32) % 256) as u8,
            ((x + y + seed as u32) % 256) as u8,
        ]);
    }
    let mut out = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(buf)
        .write_to(&mut out, image::ImageFormat::Jpeg)
        .expect("write jpeg");
    out.into_inner()
}

#[test]
fn a_miss_then_a_hit() {
    // Lazy fetch lives at this boundary: a caller asks the cache first and only
    // reaches the network on None.
    let dir = tempfile::tempdir().expect("tempdir");
    let cache = ArtworkCache::new(dir.path(), ArtworkCache::DEFAULT_BUDGET);
    let url = "https://image.example.test/t/p/w500/abc.jpg";

    assert_eq!(cache.get(url), None, "nothing cached yet");

    let stored = cache.put(url, &jpeg(200, 300, 1)).expect("put");
    assert!(stored.path.is_file());
    assert_eq!(stored.blurhash.len(), 28);
    assert_eq!((stored.width, stored.height), (200, 300));

    assert_eq!(cache.get(url).as_deref(), Some(stored.path.as_path()));
}

#[test]
fn a_second_put_of_the_same_url_replaces_rather_than_accumulating() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cache = ArtworkCache::new(dir.path(), ArtworkCache::DEFAULT_BUDGET);
    let url = "https://image.example.test/a.jpg";

    cache.put(url, &jpeg(100, 100, 1)).expect("first");
    let first = cache.size().expect("size");
    cache.put(url, &jpeg(100, 100, 2)).expect("second");
    let second = cache.size().expect("size");

    assert!(
        second < first * 2,
        "a replaced image must not leave the old one behind: {first} then {second}"
    );
}

#[test]
fn the_budget_evicts_least_recently_used_not_least_recently_written() {
    // A poster fetched months ago and looked at yesterday is on the user's home
    // screen. One fetched yesterday and never looked at again was scrolled past.
    let dir = tempfile::tempdir().expect("tempdir");
    let old = "https://image.example.test/old.jpg";
    let never_seen = "https://image.example.test/never.jpg";

    // The budget is MEASURED rather than guessed: put two images with no limit, see
    // what they actually cost, then set a budget that holds barely more than two. A
    // guessed constant either never triggers eviction — which is how this test passed
    // vacuously the first time — or evicts everything.
    let unbounded = ArtworkCache::new(dir.path(), u64::MAX);
    unbounded.put(old, &jpeg(120, 120, 1)).expect("old");
    std::thread::sleep(std::time::Duration::from_millis(1_100));
    unbounded
        .put(never_seen, &jpeg(120, 120, 2))
        .expect("never");

    let two = unbounded.size().expect("size");
    assert!(two > 0);
    let cache = ArtworkCache::new(dir.path(), two + two / 4);

    // Touch the older one, making it the most recently USED.
    std::thread::sleep(std::time::Duration::from_millis(1_100));
    assert!(cache.get(old).is_some());

    // A third image tips it over the budget.
    std::thread::sleep(std::time::Duration::from_millis(1_100));
    cache
        .put("https://image.example.test/new.jpg", &jpeg(120, 120, 3))
        .expect("new");

    assert!(
        cache.size().expect("size") <= two + two / 4,
        "eviction must bring the cache back inside its budget"
    );
    assert!(
        cache.get(old).is_some(),
        "the recently USED image must survive even though it was written first"
    );
    assert!(
        cache.get(never_seen).is_none(),
        "the never-looked-at image is the one to evict"
    );
}

#[test]
fn a_write_is_never_refused_even_when_it_breaches_the_budget() {
    // The budget is a soft ceiling. Refusing a write would mean a poster silently
    // fails to appear, which is a worse failure than being briefly over budget.
    let dir = tempfile::tempdir().expect("tempdir");
    let cache = ArtworkCache::new(dir.path(), 1);
    let url = "https://image.example.test/big.jpg";

    let stored = cache
        .put(url, &jpeg(200, 200, 1))
        .expect("put must succeed");
    assert!(
        stored.bytes > 1,
        "the image is larger than the whole budget"
    );
}

#[test]
fn a_partial_write_is_not_counted_or_served() {
    // `.part` files are what an interrupted write leaves behind. Counting them
    // against the budget would evict real entries to make room for rubbish; serving
    // one would be a permanently broken image with no way to notice.
    let dir = tempfile::tempdir().expect("tempdir");
    let cache = ArtworkCache::new(dir.path(), ArtworkCache::DEFAULT_BUDGET);
    cache
        .put("https://image.example.test/a.jpg", &jpeg(100, 100, 1))
        .expect("put");
    let real_size = cache.size().expect("size");

    let shard = std::fs::read_dir(dir.path())
        .expect("read root")
        .next()
        .expect("a shard")
        .expect("entry")
        .path();
    std::fs::write(shard.join("deadbeef.webp.part"), vec![0u8; 50_000]).expect("part file");

    assert_eq!(
        cache.size().expect("size"),
        real_size,
        "an interrupted write must not count against the budget"
    );
}

#[test]
fn removing_and_clearing() {
    // ADR-0027's key-removal path discards artwork fetched under the old key.
    let dir = tempfile::tempdir().expect("tempdir");
    let cache = ArtworkCache::new(dir.path(), ArtworkCache::DEFAULT_BUDGET);
    for i in 0..3u8 {
        cache
            .put(
                &format!("https://image.example.test/{i}.jpg"),
                &jpeg(80, 80, i),
            )
            .expect("put");
    }
    assert!(cache.size().expect("size") > 0);

    assert!(cache
        .remove("https://image.example.test/1.jpg")
        .expect("remove"));
    assert!(
        !cache
            .remove("https://image.example.test/1.jpg")
            .expect("remove again"),
        "removing what is not there is not an error"
    );

    assert_eq!(cache.clear().expect("clear"), 2);
    assert_eq!(cache.size().expect("size"), 0);
}

#[test]
fn an_empty_cache_reports_zero_rather_than_failing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cache = ArtworkCache::new(dir.path().join("not-created-yet"), 1_000);
    assert_eq!(cache.size().expect("size"), 0);
    assert_eq!(cache.clear().expect("clear"), 0);
    assert_eq!(cache.get("https://image.example.test/a.jpg"), None);
}

#[test]
fn a_response_that_is_not_an_image_is_rejected_before_anything_is_written() {
    // TMDB serving an HTML error page, or a truncated download.
    let dir = tempfile::tempdir().expect("tempdir");
    let cache = ArtworkCache::new(dir.path(), ArtworkCache::DEFAULT_BUDGET);
    let url = "https://image.example.test/broken.jpg";

    assert!(cache
        .put(url, b"<!doctype html><title>404</title>")
        .is_err());
    assert_eq!(cache.get(url), None, "nothing was written");
    assert_eq!(cache.size().expect("size"), 0);
}
