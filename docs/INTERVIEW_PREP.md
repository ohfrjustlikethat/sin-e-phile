# Interview preparation

Likely questions about this project, with real answers grounded in the actual code.

> **Status: Phase 0.** `SPEC.md` §13.4 builds this from Phase 8 onward, once there is
> a working system to talk about. Two answers are already available, and they are
> here because they are already true — not as placeholders.
>
> **The exit criterion for the entire project** is that the author can answer
> everything here without notes (Phase 27). Answers must be honest, including about
> weaknesses: a candidate who can name their project's flaws is more convincing than
> one who cannot.

## Questions to answer, and the phase that makes them answerable

| Question | Answerable from |
|---|---|
| Walk me through the architecture. | 8 |
| What was the hardest problem? | 7 |
| How do you get a torrent to start playing in eight seconds? | 7 |
| How does your search understand "slow films about loneliness"? | 5 |
| How do you know your subtitle alignment works? | 10 |
| How does the recommender avoid a filter bubble? | 17 |
| Why Rust for the backend? | 1 |
| What would you do differently? | 27 |
| What is the biggest weakness in this codebase? | 27 |

---

## Answerable now

### "Why is there no content in this thing?"

Because that is the difference between a project you can show someone and one that
gets taken down. Popcorn Time bundled sources and its forks were removed from
GitHub; Stremio ships a protocol and no sources, and Stremio still exists.

So the app ships the machinery — a documented backend protocol, a declarative
manifest format, a catalogue browser pointed at a URL the user supplies — and
working reference backends against unambiguously legal sources only: the local
filesystem, the Internet Archive, and user-supplied HTTP.

The part I would actually want to talk about is that it is enforced by a program
rather than by discipline. A rule that has to hold across 28 phases and dozens of
sessions will eventually be broken by accident, so `tools/guard/` checks every
commit and every push against the working tree *and every blob that has ever existed
in the history*.

The design problem there was interesting: the spec said to keep a denylist of site
names, but the spec also said site names must never appear in the repository. Those
contradict. The resolution was a salted-hash denylist plus plaintext *structural*
patterns — magnet URIs, bare infohashes, tracker paths — which name nothing and
generalise better anyway. And I was careful to describe the hashing as hygiene
rather than security, because the salt is committed and therefore brute-forceable.
It defends against accidental plaintext, not against a determined adversary, and
claiming otherwise would be wrong.

### "How do you know your safety checks actually work?"

Because I planted failures and watched them fire, and one of them caught a real bug.

The guard and the secret scanner each have an exit criterion requiring a
deliberately planted violation to fail the build. When I ran it, the secret scanner
caught the planted GitHub token but silently missed the planted API key. The reason
was that its placeholder-exclusion list contained the bare word `a`, so any secret
whose value started with `a` was skipped entirely — and the planted key began
`a1b2c3`.

That would have passed every casual test and protected nothing. It is the exact
argument for the exit criterion being "prove it fires" rather than "write it".

The same applies to false positives, which I think matter just as much: a guard that
cries wolf gets bypassed, and then it protects nothing either. The self-test runs 16
vectors that must *not* fire alongside the 12 that must. Its first real run produced
118 false positives, and getting to zero took three rounds of genuine design fixes,
not tuning.
