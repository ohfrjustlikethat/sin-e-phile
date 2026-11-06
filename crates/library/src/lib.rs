//! The local library (SPEC.md Phase 12): what each file in the user's folders is.
//!
//! - [`identify`]: one file's name, folders and duration against the catalogue, with a
//!   confidence that says whether to ask.
//! - [`folders`]: what "The Wire (2002)/Season 1/" says about the files inside.

pub mod folders;
pub mod identify;

pub use identify::{identify, Evidence, Identification, Scored, Signals, ACCEPT_AT};
