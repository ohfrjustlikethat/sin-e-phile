//! The repository layer.
//!
//! `SPEC.md` Phase 3: "Repository-pattern access layer — no raw SQL outside
//! `persistence/`." Every statement in the application is in one of these
//! modules, and `tools/guard` enforces that none appears under `src-tauri/`.

pub mod backfill;
pub mod catalogue;
pub mod credentials;
pub mod episodes;
pub mod media;
pub mod profiles;
pub mod search;
pub mod sources;

pub use backfill::{BackfillRepository, BackfilledRow};
pub use catalogue::{
    CatalogueRepository, PendingArticle, Readiness, StepProgress, WikipediaRepository, CORE_TIER,
    MIN_SYNOPSIS,
};
pub use credentials::{CredentialRepository, TmdbAccess};
pub use episodes::EpisodeRepository;
pub use media::MediaRepository;
pub use profiles::ProfileRepository;
pub use search::{Hit, HitDetail, MatchReason, SearchRepository};
pub use sources::{
    AddonRow, BackendRow, DirectLinkRepository, DirectLinkRow, Sample, SourceRepository,
};
