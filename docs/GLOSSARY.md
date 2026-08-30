# Glossary

Every domain and stack term this project uses, in one clear paragraph each, with a
pointer to where it shows up in the codebase.

This exists because `SPEC.md` §13.3 makes it a deliverable, and because being able
to define these terms out loud is the difference between having built something and
being able to talk about having built it. **If you cannot explain an entry here in
your own words, that is the signal to slow down**, not to keep going.

Terms are added as each phase introduces them. Entries marked *(Phase N)* name the
phase where the concept first appears in real code.

---

## BitTorrent and networking

**Piece** *(Phase 7)* — A torrent's file content is split into fixed-size chunks,
typically 256 KB to 16 MB, called pieces. Each piece has a hash recorded in the
torrent metadata, so a downloaded piece can be verified independently. Pieces are
the unit of transfer and the unit of scheduling: the entire Phase 7 streaming
scheduler is a set of rules about *which piece to ask for next*.

**Swarm** *(Phase 7)* — Everyone currently sharing a particular torrent: the peers
downloading it and the seeders who have it complete. "Swarm health" is shorthand for
how many peers there are, how much they are actually uploading, and therefore how
fast and how reliably you can expect to download. Phase 9 scores candidates partly
on measured swarm health, not merely on the advertised seeder count, because the two
differ a lot.

**Seeder / peer / leecher** *(Phase 7)* — A **seeder** has the complete file and is
only uploading. A **peer** is anyone in the swarm. A **leecher** is a peer still
downloading. More seeders generally means faster and more reliable transfer, which
is why seeder count is an input to source selection.

**Rarest-first** *(Phase 7)* — The default BitTorrent piece-selection strategy:
request the piece that fewest peers have. This maximises the number of distinct
pieces in circulation and keeps the swarm healthy. It is also **exactly wrong for
playback**, which needs pieces in order, right now. Reconciling those two facts is
the hardest engineering in this project.

**Sequential download** *(Phase 7)* — Requesting pieces in file order rather than
rarest-first, so playback can begin before the download completes. It trades swarm
health for immediacy. Phase 7's scheduler is a deadline-driven compromise: sequential
in a priority window just ahead of the playhead, rarest-first everywhere else.

**DHT (Distributed Hash Table)** *(Phase 7)* — A decentralised way to find peers for
a torrent without a tracker. Each client stores a slice of a giant distributed
lookup table mapping infohashes to peer addresses. It is why a magnet link works
with no server involved.

**Magnet link** *(Phase 7)* — A URI that identifies a torrent by its **infohash**
rather than by pointing at a `.torrent` file. The client uses DHT and peer exchange
to find peers and fetch the metadata. Phase 20 registers sin-e-phile as a handler for
the `magnet:` protocol.

**Infohash** *(Phase 7)* — The hash of a torrent's metadata; the torrent's identity.
A 40-character hex string (BitTorrent v1, SHA-1) or 64-character (v2, SHA-256).
`tools/guard/` treats a bare 40-hex string as a structural violation for exactly this
reason.

**HTTP range request** *(Phase 7)* — An HTTP request asking for a byte range of a
resource rather than the whole thing (`Range: bytes=1000-2000`). It is what makes
seeking in a video work over HTTP. Phase 7's local server implements ranges over an
*in-progress* torrent, so the player can treat a live swarm as an ordinary file.

**Backpressure** *(Phase 7)* — When a fast producer overwhelms a slow consumer, the
consumer needs a way to say "slow down". In async Rust this is usually a bounded
channel that blocks the producer when full. Without it, a fast download with slow
disk writes grows memory until something breaks.

---

## Search and retrieval

**Embedding** *(Phase 5)* — A list of numbers — here 384 of them — representing a
piece of text as a point in space, arranged so that texts with similar *meaning* sit
near each other. "A slow meditation on grief" and "a quiet film about loss" land
close together despite sharing almost no words. This is what makes searching by
meaning possible, and it is the single most important concept in the search and
recommender phases.

**Sentence transformer** *(Phase 5)* — A neural network trained to turn a sentence
into an embedding. This project uses a small quantised one (`bge-small-en-v1.5` or
`all-MiniLM-L6-v2`) running locally through ONNX Runtime — no API, no cost, works
offline.

**Quantisation / INT8** *(Phase 5)* — Storing a model's numbers at lower precision —
8-bit integers instead of 32-bit floats — making it roughly four times smaller and
faster, at a small accuracy cost. It is what allows a transformer to run acceptably
on Tier 0 hardware.

**ANN (approximate nearest neighbour)** *(Phase 5)* — Finding the points nearest a
query point *approximately*, trading a little accuracy for enormous speed. Exact
search over hundreds of thousands of vectors cannot fit an 80 ms budget on two
cores; ANN can.

**HNSW (Hierarchical Navigable Small World)** *(Phase 5)* — The specific ANN
structure used here. A layered graph you navigate by repeatedly hopping to whichever
neighbour is closer to the query, starting coarse and refining. Its parameters trade
recall against memory and speed, and §8 tiers them by hardware.

**BM25** *(Phase 5)* — The standard keyword-ranking function. It scores a document
higher when it contains the query's rare words often, while damping the reward for
repetition and for sheer document length. Excellent at exactness, blind to meaning —
the mirror image of embeddings, which is why the two are combined.

**FTS5** *(Phase 5)* — SQLite's built-in full-text search extension, providing the
inverted index and BM25 ranking. Built in, so no extra dependency (ADR-0005).

**Reciprocal rank fusion (RRF)** *(Phase 5)* — A way to merge two ranked lists using
only each item's *position*, not its score: each list contributes `1/(k + rank)`.
Because raw BM25 scores and cosine similarities are not comparable, fusing by rank
sidesteps the problem of normalising them.

**nDCG@10** *(Phase 5)* — Normalised Discounted Cumulative Gain at 10. A measure of
ranking quality: how much relevant material appears in the top ten, weighted so that
a relevant result at position 1 counts for more than one at position 10, normalised
so a perfect ranking scores 1.0. The Phase 5 target is > 0.75.

---

## Recommendation

**Collaborative filtering** *(Phase 16)* — Recommending based on behaviour patterns
rather than content: "people who liked this also liked that". It captures things no
content analysis can, like a shared sensibility across films with nothing obvious in
common.

**Item-item similarity matrix** *(Phase 16)* — A precomputed table of how similar
every item is to every other, derived from co-occurrence in ratings data. Computing
it offline is what lets this project have collaborative filtering **with no server
and no other users** — the similarity is baked in, not queried live.

**Popularity bias** *(Phase 16)* — The tendency of raw co-occurrence to favour
already-popular items, simply because more people have seen them. Left uncorrected,
the recommender only ever surfaces blockbusters. Normalising for it is what allows a
beloved obscure film to outrank a mediocre famous one.

**Candidate generation versus ranking** *(Phase 16)* — A two-stage pattern: retrieve
~1,000 plausible items cheaply, then rank those precisely with an expensive model.
Scoring the whole catalogue precisely is unaffordable; this makes the cost tractable.

**MMR (maximal marginal relevance)** *(Phase 16)* — A selection method that rewards
relevance while penalising similarity to what has already been selected. It is what
stops a rail being eight films by the same director.

**Recall@20 / coverage / novelty** *(Phase 16)* — Recommender metrics. **Recall@20**:
of the things the user actually liked, how many appeared in the top 20. **Coverage**:
what fraction of the catalogue the recommender ever surfaces. **Novelty**: mean
inverse popularity, i.e. how obscure its suggestions are. Optimising recall alone
produces a system that recommends the same 2,000 famous films forever, which is why
all three are tracked.

**Multi-armed bandit** *(Phase 17)* — A framework for choosing repeatedly among
options with unknown payoffs, balancing **exploitation** (use what works) against
**exploration** (try something to learn). A **contextual** bandit conditions that
choice on the current situation.

**Exploration floor** *(Phase 17)* — A hard minimum on how much exploration happens,
regardless of what the reward signal says. It is the mechanism that makes a filter
bubble structurally impossible rather than merely discouraged: even a user who
always picks the safe option still receives genuine discovery.

---

## Media and playback

**Container versus codec** *(Phase 8)* — A **container** (MKV, MP4) is the box: it
holds video, audio and subtitle streams plus metadata. A **codec** (H.264, HEVC,
AV1) is how a stream is compressed. "MKV" says nothing about whether your hardware
can decode what is inside it — a distinction that matters constantly in Phase 9's
source scoring.

**Hardware decode** *(Phase 8)* — Using dedicated silicon on the GPU (D3D11VA,
NVDEC, QSV) to decode video instead of the CPU. The difference between smooth 4K and
a slideshow on a weak machine. Its absence is one of the Tier 0 detection signals.

**Muxing / remuxing / transcoding** *(Phase 26)* — **Muxing** combines streams into a
container. **Remuxing** moves streams into a different container without
re-encoding — fast and lossless. **Transcoding** re-encodes, which is slow and lossy.
Phase 26 remuxes where it can and transcodes only when the receiver cannot handle the
source codec.

**ASS/SSA** *(Phase 10)* — Advanced SubStation Alpha, a subtitle format supporting
positioning, styling, fonts and effects, used heavily in anime fansubs. Unlike plain
SRT it carries layout, so a renderer that ignores it destroys the intended
presentation. First-class ASS rendering is a main reason for choosing libmpv
(ADR-0004).

**VAD (voice activity detection)** *(Phase 10)* — Detecting which parts of an audio
track contain speech. Phase 10 extracts a VAD signal from the audio and compares it
against the pattern of subtitle timings to work out how far out of sync they are.

**Cross-correlation** *(Phase 10)* — A way to find the offset at which two signals
best line up: slide one across the other and measure agreement at each shift. The
peak is the alignment. Phase 10 solves for **offset and framerate scale** together,
because a 23.976-versus-25 fps mismatch is a multiplication, not an addition, and
no constant offset can fix it.

---

## Rust, and this stack

**Ownership and borrowing** *(Phase 1)* — Rust's core idea: every value has exactly
one owner, and you either borrow it immutably (many readers) or mutably (one writer,
no readers). Enforced at compile time, which is how Rust achieves memory safety with
no garbage collector.

**`Result` and `?`** *(Phase 1)* — Rust has no exceptions. A function that can fail
returns `Result<T, E>` — either `Ok(value)` or `Err(error)` — and the caller must
handle both. The `?` operator returns early on error, making the common path
readable while keeping failure explicit.

**`Arc<Mutex<T>>`** *(Phase 7)* — `Arc` is a thread-safe reference-counted pointer,
letting several threads share ownership of the same value; `Mutex` ensures only one
touches it at a time. Together they are the standard way to share mutable state
across threads. The classic mistake is holding the lock across an `await`, which can
deadlock.

**`async` / `await` and `tokio`** *(Phase 7)* — Async lets one thread juggle
thousands of waiting operations (network reads, disk I/O) instead of blocking on
each. `tokio` is the runtime that schedules them. Essential for a torrent engine
talking to many peers at once.

**Trait / trait object / `async_trait`** *(Phase 6)* — A **trait** is a shared
interface. A **trait object** (`dyn SourceBackend`) allows different types to be used
interchangeably behind it, chosen at runtime — which is what makes the resolver
backend-agnostic. `async_trait` is a macro working around Rust's historical inability
to put `async fn` in traits.

**`sqlx` compile-time checking** *(Phase 3)* — `sqlx` checks SQL queries against the
real database schema **at compile time**, so a misspelled column is a build error
rather than a runtime failure. Genuinely valuable when learning SQL (ADR-0005).

**WAL mode** *(Phase 3)* — SQLite's Write-Ahead Logging. Instead of writing changes
in place, they go to a separate log first, so readers are not blocked by a writer.
Better concurrency and better crash resilience.

**IPC (inter-process communication)** *(Phase 1)* — How the React frontend and the
Rust backend talk. Tauri exposes Rust functions as commands the frontend calls, and
events the backend pushes. Phase 1 *generates* the TypeScript types from the Rust
definitions, so the two sides cannot silently drift apart.

**Virtualisation (UI)** *(Phase 2)* — Rendering only the rows or cards currently
visible, rather than all 500. Without it, a long rail creates thousands of DOM nodes
and scrolling stutters. Required to hit the 60 fps budget.

**Design token** *(Phase 2)* — A named design value (`--oxblood`, `--surface`) held
in one place and referenced everywhere, so the visual language stays consistent and
can be changed centrally. Here they are CSS custom properties consumed by a Tailwind
theme extension.

---

## Process

**ADR (architecture decision record)** *(Phase 0)* — A short document recording a
decision, its context, its consequences, and **what was rejected and why**. Written
at the moment of deciding, never reconstructed afterwards. See
[`docs/adr/0001`](adr/0001-record-architecture-decisions.md).

**Conventional commits** *(Phase 0)* — A commit message convention,
`type(scope): subject`, that makes history scannable and changelogs generable. The
`commit-msg` hook enforces it.

**Evidence standard** *(Phase 0)* — `SPEC.md` §10.8: an exit criterion is met only
with an **artefact** — a passing test name, a measured number *with the command that
produced it*, a file path, a commit SHA, or an explicit `manual:` note. "Looks good"
is not evidence, and the state file's schema actively rejects it.

**The understanding gate** *(Phase 0)* — `SPEC.md` §10.10: a phase is not done until
the author has answered the learning note's five self-check questions **out loud**.
Not "yeah I get it" — explained back. If two or more cannot be answered, the code was
too complex or went too fast, and the response is to simplify, not to proceed.

**Tier A/B/C/D** *(Phase 0)* — The legitimate stopping points in `SPEC.md`
Appendix E. **Tier B (Phases 9–18, 21, 27) is the definition of done.** A finished
Tier B project beats an abandoned Tier D one, always.
