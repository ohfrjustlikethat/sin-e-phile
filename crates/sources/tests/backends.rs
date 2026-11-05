//! Properties every registered backend must have, checked over `backends::all` so a
//! new backend is covered the moment it is registered — with no edit here (E1).

use std::collections::HashSet;

use sinephile_sources::backends::{all, Context};

#[test]
fn every_backend_has_a_unique_well_formed_id() {
    let backends = all(&Context::default());
    let mut seen = HashSet::new();
    for b in &backends {
        let id = b.id();
        assert!(
            !id.is_empty()
                && id.len() <= 80
                && id
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "._-:".contains(c)),
            "{id:?} is not a valid backend id"
        );
        assert!(seen.insert(id.to_string()), "{id} is registered twice");
    }
}

#[test]
fn every_backend_declares_what_it_can_find() {
    for b in all(&Context::default()) {
        let caps = b.capabilities();
        assert!(!caps.media.is_empty(), "{} finds nothing", b.id());
        assert!(
            !caps.transports.is_empty(),
            "{} declares no transport",
            b.id()
        );
    }
}
