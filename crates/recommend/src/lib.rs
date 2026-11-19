//! Recommendations (SPEC.md Phase 17).
//!
//! - [`cf`]: Layer 2, item-item collaborative filtering. "People who loved this also loved
//!   that", computed on this machine from MovieLens' ratings (ADR-0019: the ratings are
//!   downloaded by the user and never redistributed, and nothing derived from them is
//!   shipped). No server and no other users: the ratings of 160,000 strangers stand in for
//!   them.
//! - [`rank`]: Layer 3, the hybrid ranker: one score from closeness to a taste mode, the
//!   matrix, popularity and the profile's axes; a rail picked by maximal marginal relevance
//!   within caps per director, franchise and year; a truthful reason for every pick.
//! - [`split`]: which MovieLens users build the matrix, which tune it, and which judge it,
//!   so a number in `docs/eval-results.md` is never measured on the people it learned from.
//! - [`strategy`]: Phase 18's discovery arms (stretch, bridge, blind spot, canon, deep cut),
//!   each pick with a label that says what is true of it.

pub mod cf;
pub mod rank;
pub mod split;
pub mod strategy;
