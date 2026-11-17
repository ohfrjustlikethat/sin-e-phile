//! Recommendations (SPEC.md Phase 17).
//!
//! - [`cf`]: Layer 2, item-item collaborative filtering. "People who loved this also loved
//!   that", computed on this machine from MovieLens' ratings (ADR-0019: the ratings are
//!   downloaded by the user and never redistributed, and nothing derived from them is
//!   shipped). No server and no other users: the ratings of 160,000 strangers stand in for
//!   them.
//! - [`split`]: which MovieLens users build the matrix, which tune it, and which judge it,
//!   so a number in `docs/eval-results.md` is never measured on the people it learned from.

pub mod cf;
pub mod split;
