//! Deduplication: the same bytes reached two ways are one candidate.
//!
//! A pure fold, separate from the resolver, so it can be tested without a clock and
//! applied incrementally as each backend's batch arrives.

use std::collections::HashMap;

use source_protocol::SourceCandidate;

/// Candidates merged so far, in arrival order.
#[derive(Debug, Default)]
pub struct Merged {
    candidates: Vec<SourceCandidate>,
    by_locator: HashMap<String, usize>,
}

impl Merged {
    /// Fold in one backend's batch. A candidate whose `locator` was already seen is
    /// not added again. The earliest to arrive stays — which one is "best" is Phase 9's
    /// scoring, not this — and gains the newcomer's backend in `also_from`, plus any
    /// languages and a size it knew that the first did not.
    pub fn add(&mut self, batch: Vec<SourceCandidate>) {
        for c in batch {
            let Some(locator) = c.locator.clone().filter(|l| !l.is_empty()) else {
                self.candidates.push(c);
                continue;
            };
            match self.by_locator.get(&locator) {
                Some(&i) => absorb(&mut self.candidates[i], c),
                None => {
                    self.by_locator.insert(locator, self.candidates.len());
                    self.candidates.push(c);
                }
            }
        }
    }

    pub fn candidates(&self) -> &[SourceCandidate] {
        &self.candidates
    }

    pub fn into_candidates(self) -> Vec<SourceCandidate> {
        self.candidates
    }
}

fn absorb(kept: &mut SourceCandidate, dup: SourceCandidate) {
    let from = dup.provenance.backend;
    if from != kept.provenance.backend && !kept.provenance.also_from.contains(&from) {
        kept.provenance.also_from.push(from);
    }
    for lang in dup.languages {
        if !kept.languages.contains(&lang) {
            kept.languages.push(lang);
        }
    }
    for lang in dup.subtitles {
        if !kept.subtitles.contains(&lang) {
            kept.subtitles.push(lang);
        }
    }
    kept.size_bytes = kept.size_bytes.or(dup.size_bytes);
    kept.duration_seconds = kept.duration_seconds.or(dup.duration_seconds);
}

#[cfg(test)]
mod tests {
    use super::*;
    use source_protocol::{Provenance, Transport};

    fn cand(backend: &str, key: &str, locator: Option<&str>, langs: &[&str]) -> SourceCandidate {
        SourceCandidate {
            key: key.into(),
            title: "T".into(),
            year: None,
            transport: Transport::Http,
            quality: Default::default(),
            languages: langs.iter().map(|s| s.to_string()).collect(),
            subtitles: vec![],
            codec: Default::default(),
            size_bytes: None,
            duration_seconds: None,
            health: Default::default(),
            locator: locator.map(Into::into),
            provenance: Provenance {
                backend: backend.into(),
                ..Default::default()
            },
        }
    }

    #[test]
    fn the_same_locator_from_two_backends_is_one_candidate() {
        let mut m = Merged::default();
        m.add(vec![cand(
            "a",
            "1",
            Some("https://x.example/f.mp4"),
            &["en"],
        )]);
        m.add(vec![cand(
            "b",
            "2",
            Some("https://x.example/f.mp4"),
            &["en", "fr"],
        )]);
        let c = m.candidates();
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].provenance.backend, "a");
        assert_eq!(c[0].provenance.also_from, vec!["b".to_string()]);
        assert_eq!(c[0].languages, vec!["en".to_string(), "fr".to_string()]);
    }

    #[test]
    fn candidates_without_a_locator_are_never_merged() {
        let mut m = Merged::default();
        m.add(vec![cand("a", "1", None, &[]), cand("a", "2", None, &[])]);
        m.add(vec![cand("b", "1", Some(""), &[])]);
        assert_eq!(m.candidates().len(), 3);
    }

    #[test]
    fn a_backend_repeating_itself_does_not_list_itself_twice() {
        let mut m = Merged::default();
        m.add(vec![
            cand("a", "1", Some("btih:00"), &[]),
            cand("a", "2", Some("btih:00"), &[]),
        ]);
        assert_eq!(m.candidates().len(), 1);
        assert!(m.candidates()[0].provenance.also_from.is_empty());
    }
}
