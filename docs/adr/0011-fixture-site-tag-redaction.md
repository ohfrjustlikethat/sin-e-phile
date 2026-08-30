# 0011 — Fixture site-tag redaction policy

- **Status:** Accepted
- **Date:** 2026-08-30
- **Phase:** 0
- **Amends:** `SPEC.md` §2.1, §12.2
- **Risks:** R10, and the accuracy target of the Phase 12 identification harness

## Context

`SPEC.md` §12.2 requires `fixtures/filenames/` to hold 500+ **real-world messy
filenames** with hand-labelled correct answers, as the corpus for the Phase 12
filename-identification eval harness. Its targets are >95% top-1 accuracy and <1%
false-confident rate.

Real-world scene and fansub filenames routinely carry a site tag — a bracketed or
suffixed token naming the site the release was distributed through. A representative
shape is `Movie.Title.2019.1080p.BluRay.x264-GROUP[sitename]`.

§2.1 forbids site names anywhere in the repository, including test fixtures.
§12.2 requires realistic fixtures. Neither section acknowledges the other, and the
collision is guaranteed: the corpus cannot be built as specified without either
violating the posture or discarding a structural feature the parser genuinely has to
handle.

Discovering this in Phase 12 would mean 500 hand-labelled cases that cannot be
committed. It is resolved here, in Phase 0, while the guard is still being written.

## Decision

**Release-group tokens are permitted verbatim.** A release group is not an indexer,
a tracker, or a source of content. It is a naming convention the parser must learn:
group identity is a genuine quality signal used by the Phase 9 source scorer, and
stripping the trailing `-GROUP` correctly is a core parser responsibility. Removing
these would damage the corpus for no posture benefit.

**Site tags are replaced with synthetic tokens at authoring time.** Any token inside
a filename that names a distribution site becomes `[SITE01]`, `[SITE02]`, … The
mapping from synthetic token to real site is **never recorded anywhere** — not in
the fixture, not in a side file, not in a comment. There is nothing to leak because
nothing is written down.

Synthetic tokens are allocated per *distinct shape*, not per site, so the corpus
retains variety in tag position, bracket style, and casing:

```
Movie.Title.2019.1080p.BluRay.x264-GROUP[SITE01]
[SITE02] Series Title - 07 [1080p][HEVC].mkv
Movie Title (2019) [1080p] [WEBRip] [5.1] [YTS-LIKE]   <- group-shaped, kept
www.SITE03.tld - Movie.Title.2019.720p.mkv
```

This is the crux: **the parser still learns everything that matters.** Its job is to
recognise that a bracketed token in leading or trailing position is a distribution
tag and strip it before title extraction. That is a *positional and structural*
lesson. It is entirely indifferent to which specific string sits inside the
brackets. `[SITE01]` teaches the identical lesson to the real name, and the eval
harness measures the identical capability.

Consistent with ADR-0009, any domain-shaped tag in a fixture uses an RFC 2606
reserved TLD (`www.SITE03.tld` above, or `.invalid` / `.test` / `.example`).

**`fixtures/filenames/README.md` documents this policy** — that redaction happened,
what it preserves, and why the mapping does not exist — so a reader evaluating the
harness understands the corpus is redacted rather than synthetic, and does not
mistake the numbers for having been measured on toy data.

## Consequences

**Easier.** Phase 12's corpus can be built to full realism and committed to a public
repository with no posture risk. The guard stays strict; the fixture needs no
exemption, no `# guard:ignore` comment, and no special-case path — mechanisms which,
once they exist, get reused for things that should not have been exempted.

**Harder.** Corpus authoring gains a manual redaction step, and it must happen at
authoring time. A filename pasted in raw and redacted in a later commit is already
in history, and cleaning it means a rewrite. The pre-commit hook is the real defence
here, because it fires before the mistake is permanent.

**Honest limitation, and it belongs in the case study.** The corpus cannot measure
whether the parser handles a *specific* real site tag that happens to collide with a
title word. If a site were named, say, `Heat`, then `Heat.1995.[Heat].mkv` is a
genuine ambiguity this corpus cannot represent. This is a narrow and improbable gap,
but the Phase 12 case study must state it rather than quietly claim the corpus is
fully representative.

## Alternatives Considered

**Strip site tags entirely rather than substituting.** Rejected, and this is the
important rejection: it silently changes what the corpus tests. The parser would
never see a distribution tag in training or evaluation, would never learn to strip
one, and would then fail on real user files in exactly the case the corpus was
supposed to cover — while reporting a high accuracy number. A corpus that is easier
than reality is worse than no corpus, because it produces false confidence, which is
the specific failure mode §12.2 names as the worst one.

**Hash the site tags** (`[a3f9c2]`). Rejected: no advantage over sequential
synthetic tokens, and it invites someone to try reversing the hashes.

**Keep the real tags and exempt `fixtures/` from the guard.** Rejected outright.
An exempt directory is where the posture goes to die: it becomes the path of least
resistance for every subsequent inconvenience, and R10 describes this exact drift.

**Generate the corpus synthetically from templates.** Rejected: synthetic filenames
encode the assumptions of whoever wrote the generator, so the parser is evaluated
against its own author's imagination. The value of §12.2's corpus is precisely that
real filenames are messier and stranger than anyone would think to invent.
