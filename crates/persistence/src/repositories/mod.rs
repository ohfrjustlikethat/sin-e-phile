//! The repository layer.
//!
//! `SPEC.md` Phase 3: "Repository-pattern access layer — no raw SQL outside
//! `persistence/`." Every statement in the application is in one of these
//! modules, and `tools/guard` enforces that none appears under `src-tauri/`.

pub mod backfill;
pub mod catalogue;
pub mod credentials;
pub mod downloads;
pub mod episodes;
pub mod library;
pub mod media;
pub mod profiles;
pub mod search;
pub mod sources;
pub mod subtitles;

pub use backfill::{BackfillRepository, BackfilledRow};
pub use catalogue::{
    CatalogueRepository, PendingArticle, Readiness, StepProgress, WikipediaRepository, CORE_TIER,
    MIN_SYNOPSIS,
};
pub use credentials::{CredentialRepository, TmdbAccess};
pub use downloads::{Download, DownloadRepository, DownloadState, NewDownload};
pub use episodes::{EpisodeRepository, EpisodeRow};
pub use library::{
    Candidate, CandidateFacts, FileMatch, LibraryFolder, LibraryRepository, LocalFile, MatchMethod,
    NewLocalFile, Probe, Upserted,
};
pub use media::{Appearance, Credit, MediaRepository};
pub use profiles::ProfileRepository;
pub use search::{Hit, HitDetail, MatchReason, SearchRepository};
pub use sources::{
    AddonRow, BackendRow, DirectLinkRepository, DirectLinkRow, Sample, SourceRepository,
};
pub use subtitles::{CachedSubtitle, QuotaDay, SubtitleRepository};
