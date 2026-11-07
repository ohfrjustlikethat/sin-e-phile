//! Phase 13 storage: clearing a finished download takes its folder, its library file and
//! its row, and nothing else in the downloads folder.

use sinephile_downloads::storage::{clear, stored};
use sinephile_persistence::repositories::profiles::PlaybackPosition;
use sinephile_persistence::repositories::{
    DownloadRepository, LibraryRepository, MediaRepository, NewDownload, NewLocalFile,
    ProfileRepository,
};
use sinephile_persistence::{Db, NewMediaItem};
use tempfile::TempDir;

#[tokio::test]
async fn clearing_a_watched_download_takes_its_folder_its_file_and_its_row() {
    let work = TempDir::new().unwrap();
    let dir = work.path().join("Downloads");
    let own = dir.join("Late Spring");
    std::fs::create_dir_all(&own).unwrap();
    let film_file = own.join("Late.Spring.1949.mkv");
    std::fs::write(&film_file, vec![0u8; 4096]).unwrap();
    std::fs::write(own.join("Late.Spring.1949.en.srt"), b"1").unwrap();
    let neighbour = dir.join("Early Summer");
    std::fs::create_dir_all(&neighbour).unwrap();
    std::fs::write(neighbour.join("Early.Summer.1951.mkv"), vec![0u8; 8192]).unwrap();

    let db = Db::in_memory().await.unwrap();
    let film = MediaRepository::new(&db)
        .insert(&NewMediaItem::film("Late Spring", 1949))
        .await
        .unwrap();
    let other = MediaRepository::new(&db)
        .insert(&NewMediaItem::film("Early Summer", 1951))
        .await
        .unwrap();
    let repo = DownloadRepository::new(&db);
    let library = LibraryRepository::new(&db);
    let folder = library.add_folder(&dir.to_string_lossy()).await.unwrap();
    let mut ids = Vec::new();
    for (item, title, file) in [
        (film, "Late Spring", film_file.clone()),
        (
            other,
            "Early Summer",
            neighbour.join("Early.Summer.1951.mkv"),
        ),
    ] {
        let id = repo
            .add(&NewDownload {
                media_item_id: Some(item),
                title,
                source: "torrent",
                backend: "librqbit",
                ..Default::default()
            })
            .await
            .unwrap();
        let path = file.to_string_lossy().into_owned();
        repo.finish(id, &path).await.unwrap();
        library
            .upsert_file(&NewLocalFile {
                folder_id: folder,
                path: &path,
                size_bytes: 1,
                modified_at: None,
            })
            .await
            .unwrap();
        ids.push(id);
    }
    let profiles = ProfileRepository::new(&db);
    let profile = profiles.ensure_default().await.unwrap();
    profiles
        .save_position(
            profile,
            &PlaybackPosition {
                media_item_id: film,
                position_seconds: 6_000,
                duration_seconds: Some(6_060),
                audio_language: None,
                subtitle_language: None,
            },
        )
        .await
        .unwrap();

    let list = stored(&db, profile, 0).await.unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(
        list[0].download.id, ids[0],
        "watched comes first, though smaller"
    );
    assert!(list[0].watched && !list[1].watched);
    assert_eq!(
        (list[0].bytes, list[1].bytes),
        (4096, 8192),
        "sizes on disk"
    );

    clear(&db, ids[0], &dir).await.unwrap();
    assert!(!own.exists(), "its folder, subtitles included, is gone");
    assert!(
        neighbour.join("Early.Summer.1951.mkv").exists(),
        "the other is not"
    );
    assert!(dir.exists(), "nor the downloads folder");
    assert!(repo.get(ids[0]).await.unwrap().is_none());
    assert!(library
        .file_by_path(&film_file.to_string_lossy())
        .await
        .unwrap()
        .is_none());
    assert_eq!(stored(&db, profile, 0).await.unwrap().len(), 1);
}
