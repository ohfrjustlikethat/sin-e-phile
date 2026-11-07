//! The local library (SPEC.md Phase 12): what each file in the user's folders is.
//!
//! - [`identify`]: one file's name, folders and duration against the catalogue, with a
//!   confidence that says whether to ask.
//! - [`folders`]: what "The Wire (2002)/Season 1/" says about the files inside.
//! - [`scan`]: a folder walked, reconciled with what is stored, and what is new identified.
//! - [`watch`]: changes in a watched folder, applied as they happen, a rename kept a rename.
//! - [`review`]: the files it was not sure about, and the user's answers, remembered.
//! - [`probe`]: FFprobe, for the files a name alone could not settle.

pub mod folders;
pub mod identify;
pub mod probe;
pub mod review;
pub mod scan;
pub mod watch;

pub use identify::{identify, Evidence, Identification, Scored, Signals, ACCEPT_AT};
