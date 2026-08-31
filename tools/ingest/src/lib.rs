//! Offline dataset ingestion (`SPEC.md` §7, Phase 4).
//!
//! Everything here is resumable, because `SPEC.md` Phase 4 requires it as an exit
//! criterion and because an ingestion that must run to completion in one go is one
//! that never completes on a laptop that sleeps.
//!
//! The resumability lives in [`job`], and is built on one rule: a checkpoint commits
//! in the same transaction as the work it describes.

pub mod download;
pub mod imdb;
pub mod job;
pub mod load;
pub mod measure;
pub mod tsv;

pub use download::{Downloaded, Downloader, Progress};
pub use job::{Batch, Job, JobError, StepOutcome, StepProgress, StepStatus};
pub use tsv::{Row, TsvReader};
