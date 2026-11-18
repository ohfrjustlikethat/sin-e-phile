# Phase 17 — Recommendation Engine: learning notes

Four sections only (ADR-0016 A4): what we built, why, new concepts, the five questions.

## 1. What we built

- **The item-item matrix** (17.1, `crates/recommend`): for every title MovieLens users rated, its 50 best neighbours
  by who loved both. Built on this machine from 25 million ratings in about 10 seconds, stored as 989,600 rows
  (migration 0029), never shipped (ADR-0019).
- **One score with two numbers**, tuned on users set aside for it: `alpha` 0.65 trades 3% of recall for reaching 16%
  of the catalogue instead of 10%. On users it never learned from, it finds 74% more of their held-out favourites
  than "the most popular" does.
- **The matrix keeps up with MovieLens** (ADR-0054, the author's rule that the app is current on every launch): every
  launch asks GroupLens, with one HEAD request, whether its rolling release has changed; when it has, or when there is
  no matrix yet, it is fetched, joined onto the catalogue, built and stored, and the archive deleted.
- **The hybrid ranker** (17.2, `crates/recommend/src/rank.rs`): about a thousand candidates from three pools (each
  taste mode's nearest in meaning, the matrix neighbours of what the profile loved, the most-voted), one blended score,
  a rail picked for variety within caps (two per director, two per franchise, five per year), and a reason for every
  pick: the part of its score that lifts it furthest above a typical candidate. Home's "Where your taste points" uses
  it, and each card says why: "Loved by people who loved Stalker and Solaris", or "Close to Stalker and Solaris, which
  you loved". `eval recommend --rail 1` prints a profile's whole rail with its reasons.
- **The evaluation** (`eval recommend`): an 80/10/10 split of users by hash, a fifth of each judged user's likes held
  out, Recall@20 against popularity, and coverage.
- **Calibration** (17.6): each taste mode fills at most its share of a rail, so a profile of seven films and two anime
  gets fifteen films and five anime, not twenty films.
- **Current on every launch** (17.6, ADR-0054): titles without meaning get a synopsis and a vector in a companion
  graph beside the published one (2,000 a launch, most-voted first); AniList's and TMDB's recommendations stand beside
  MovieLens', asked weekly for what each profile loved; a title nobody has rated reaches a rail through its director
  ("By Andrei Tarkovsky, who made Stalker and Solaris"); "Rising now" shows what gained most ratings at the last
  update; Settings says when each source last updated and what it did.

## 2. Why

- **Collaborative filtering knows what descriptions cannot**: that people who love one Ozu film love the others, and
  that Tarkovsky's audience also loves *Come and See*, whatever the synopses say.
- **Item-item, not user-user**: there are no other users on this machine. MovieLens' 162,000 strangers stand in for
  them, and an item's neighbours are the same for everyone, so they are computed once and stored.
- **Popularity is the bias to fight**: raw "people who liked i also liked j" says "everyone liked *The Shawshank
  Redemption*". Dividing popularity back out (`alpha` towards 1) lets an obscure film surface; too far, and the
  matrix recommends noise (0.9 fell below popularity). The tuning users decide where between.
- **Measured on people it never saw**: a recommender judged on the users it learned from looks better than it is.
  The split is by hash, so it is the same on every run, and a test pins that no user is in two parts.
- **Recall is not the only good**: held-out likes follow a person's larger taste, so the most "accurate" rail ignores
  their smaller ones. Calibration gave up 1.9 points of recall for rails that mix as the profile does, and nearly
  tripled the share of the catalogue that reaches someone.
- **Nothing derived may be a snapshot** (the author's rule): the published meaning file and MovieLens both stop at a
  date, so the app extends the first itself and sets live sources beside the second.

## 3. New concepts

- **Compressed sparse rows**: rows stored end to end with a start offset per row — `crates/recommend/src/cf.rs:44`.
- **Inverse user frequency**: each person weighted `1 / ln(e + likes)` — `crates/recommend/src/cf.rs:116`.
- **Asymmetric cosine with shrinkage**: `c / (mass_i^(1-alpha) * mass_j^alpha) * c / (c + shrink)` —
  `crates/recommend/src/cf.rs:203`.
- **Item-by-item accumulation with a dense array reset through a "touched" list**: nothing items x items is ever held
  — `crates/recommend/src/cf.rs:236`.
- **Scoped threads** (`std::thread::scope`): threads that borrow local data and are joined before it goes —
  `crates/recommend/src/cf.rs:226`.
- **Interleaved work** (`step_by(threads)`) so popular items do not pile onto one thread — `crates/recommend/src/cf.rs:236`.
- **A reused buffer**: `truncate` keeps capacity, so rows kept the room of everything they touched (1 GB); one buffer per
  thread, copied out at exact size, holds 137 MB — `crates/recommend/src/cf.rs:234`.
- **Partial selection** (`select_nth_unstable_by`): the best 50 of thousands without sorting them all —
  `crates/recommend/src/cf.rs:297`.
- **A deterministic split by hash** (splitmix64) — `crates/recommend/src/split.rs:21`.
- **Recall@K and coverage**, and why a popularity baseline is the honest one — `tools/eval/src/recommend.rs`.
- **CPU work off the async runtime** (`spawn_blocking`) — `crates/catalogue/src/neighbours.rs:39`.
- **Scaling each part of a score over the candidates** before weighting, so a weight says how much a part counts —
  `crates/recommend/src/rank.rs:153`.
- **A reason from the part furthest above the median candidate**, not the biggest part: a part every candidate has
  (closeness) tells none of them apart — `crates/recommend/src/rank.rs:196`.
- **Maximal marginal relevance**: score minus similarity to what is already picked —
  `crates/recommend/src/rank.rs:376`.
- **Hard caps as a filter, not a penalty** — `crates/recommend/src/rank.rs:367`.
- **A proxy, said to be one**: a franchise read from the title — `crates/recommend/src/rank.rs:429`.
- **Calibration by quota, shared out by largest remainder** (Steck 2018): slots in proportion to shares, adding up to
  the rail exactly — `crates/recommend/src/rank.rs:268`; miscalibration as KL divergence —
  `tools/eval/src/recommend.rs:221`.
- **A trait as a seam**: the meaning index behind `Meaning`, so a test can stand in a few vectors —
  `crates/taste/src/hybrid.rs:79`.
- **A growable graph beside a read-only one**: the published graph memory-mapped, the companion in memory, searches
  merged by distance — `crates/vector-index/src/lib.rs:330`, `crates/vector-index/src/lib.rs:427`.
- **Save whole or not at all**: write a part file, then rename over the old — `crates/vector-index/src/lib.rs:488`.
- **`block_in_place`** for CPU work on a multi-threaded async runtime, and why a single-threaded test cannot use it —
  `crates/catalogue/src/meaning.rs:134`.
- **Scores matched by rank across sources**: a live list's k-th row is worth what MovieLens' k-th neighbour averages —
  `crates/catalogue/src/live.rs:67`.
- **`UPDATE ... FROM`**, and every right-hand side reading the row as it was, to keep a value's predecessor —
  `crates/catalogue/src/refresh.rs:160`.
- **Window functions** (`ROW_NUMBER() OVER (PARTITION BY ...)`): each maker's newest few in one query —
  `crates/persistence/src/repositories/media.rs:359`.
- **Compute at write time, read a list at render time**: rising is ranked once per refresh, not per Home —
  `crates/persistence/src/repositories/media.rs:424`.
- **`Send` versus `Sync`**: a future holding `&T` across `.await` needs `T: Sync`; holding `&mut T` needs only
  `T: Send` — `crates/taste/src/hybrid.rs:101`, `crates/taste/src/home.rs:219`.
- **`WITHOUT ROWID` tables** keyed by their primary key — `crates/persistence/migrations/0029_item_neighbours.up.sql:10`.
- **A validator** (`Last-Modified`/`ETag`) asked by HEAD: "has it changed?" for a few hundred bytes — 
  `crates/catalogue/src/freshness.rs:80`.
- **Remember only after success**: a validator stored before the build would make a failed build look finished —
  `crates/catalogue/src/neighbours.rs:81`.
- **A launch step that can never fail the launch**: every error becomes an outcome to log —
  `crates/catalogue/src/neighbours.rs:43`.
- **`Sync` callbacks held across `.await`** in a future that must be `Send` — `crates/catalogue/src/bootstrap.rs:415`.
- **Replacing a table whole in one transaction**, so a reader never sees half — 
  `crates/persistence/src/repositories/neighbours.rs:22`.

## 4. Five questions

1. Why does item-item collaborative filtering work on a machine with one user, when "people like you" needs other
   people?
2. What does `alpha` do to *The Shawshank Redemption* as a neighbour, at 0, 0.5 and 1, and why did 0.9 fall below
   popularity?
3. Why is the matrix built item by item with a dense array, rather than counting every pair in a hash map?
4. Why are the users who tune `alpha` different from the users who judge it, and what would go wrong if they were the
   same?
5. Calibration took Recall@20 from +72.7% to +61.4% over popularity and coverage from 16.6% to 43.5%. What does each
   number say about a rail, and why was that trade taken?
