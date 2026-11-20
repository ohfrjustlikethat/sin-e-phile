# The discovery engine (Phase 18)

Status: 18.1 to 18.4 built (the arms and their labels, countries and movements, the bandit, the budget, dismissals,
Surprise me, the length limit, mood shelves).

## 1. The arms

Six ways of choosing what to show. The first is Phase 17's taste rail; the other five are
`crates/recommend/src/strategy.rs`, pure functions over described titles.

| Arm | Chooses | A card says |
|---|---|---|
| Familiar | Phase 17's hybrid ranker (`rank::rank`) | its `rank::Reason` ("Loved by people who loved Stalker") |
| Stretch | close to a taste mode, outside it on exactly one axis | "Like Stalker, but from the 1990s" |
| Bridge | close to two modes at once | "Between Stalker and Spirited Away" |
| Blind spot | acclaimed (7.5+, 5,000 votes) from one region the profile met nothing from | rail: "You've never watched anything from the 1950s"; card: "Rated 8.1 by 25,400 people" |
| Canon | 7.8+ with 50,000 votes, close to a mode | "Acclaimed, and close to Stalker" |
| Deep cut | 7.5+ with 1,000 to 15,000 votes, loved alongside a loved title or close to a mode | "Loved by people who loved Mirror, and little seen" |

**Close** is by rank: within a mode's 1,000 nearest titles in the meaning index (of about 120,000), the 1,000th's cosine
read from the index each time (`discover::CLOSE_RANK`). A cosine threshold would mean something different for every
model.

**Regions** are a decade, an IMDb genre (not the broadcast ones: Adult, Game-Show, News, Reality-TV, Short, Talk-Show),
a country and a film movement. A stretch is outside the mode on one axis only: on an axis the mode says something
about, with a value none of its titles has. A blind-spot rail holds one region, since its name is the region's.

## 2. Labels are claims

`Label` is data. `strategy::holds(label, title, profile)` re-derives the claim from the facts, independently of how the
arm chose; the words (`discover::note`, `discover::rail_title`) say no more than the label. Tested on 300 generated
profiles (`every_label_of_every_arm_holds_on_generated_profiles`), seen to fail when the arms took titles short of
"close" and when the blind spot ignored what was met.

**An honest "never"**: country and movement are *unknown* for a profile until every title it met (an episode as its
series) was asked of Wikidata (`Profile::unknown`); no arm claims anything on an unknown axis. A blind spot also needs
ten titles met.

## 3. Countries and movements (ADR-0055)

`crates/catalogue/src/regions.rs`, hourly beside the live neighbours: Wikidata's P495 (country of origin) and P135
(movement), English labels, by IMDb id, 100 a query allowed WDQS's 60 s, 6,000 titles a run (measured on the
development copy: 69,124 titles in 2,150 s at 200 a query one day; 39 s a query of 200 the next, past the transport's
30 s, which is why the batch halved and the wait doubled). Met titles
first, then films and series with 1,000 votes, most voted first. `title_regions` holds the answers and `regions_asked`
every title asked. `ingest regions` runs it to the end on a data folder.

## 4. Gathering (`crates/taste/src/discover.rs`)

The pool: each mode's 1,000 nearest (asked of the whole index), the acclaimed (up to 6,000), and Phase 17's
neighbours of what the profile loved. Facts from `MediaRepository::facts` (rating, countries and movements added in
Phase 18). Home shows each arm the plan gives a rail, with at least five picks, after "Where your taste points", none
repeating a title shown elsewhere on Home. Within an arm, picks are ordered by Phase 17's relevance (closeness to the
nearest taste, plus half the matrix's part): the label says why a title may be in the arm, relevance which come first.

## 5. The bandit (`crates/recommend/src/bandit.rs`)

- **Beliefs**: per arm and part of the day (day 05-17, evening 17-23, late), `Beta(1 + successes, 1 + failures)`, each
  outcome faded at a 60-day half-life, a part of the day borrowing 0.3 of the arm's evidence from the others.
- **Rewards**: completed 1, watchlisted 0.7, played 0.6, not right now 0.25 (that part of the day only), abandoned in
  the first fifth or not for me 0, already seen nothing. A title counts once, by its most decisive outcome.
- **Discovery's share** of 60 slots: from 20% (Comfort) to 80% (Adventure) by the budget, moved by `0.6 x` how far the
  discovery arms' mean chance leads the familiar's, never outside 20% to 80%. The taste rail shows at most 20.
- **Allocation**: each discovery arm's chance of being the best, from 100 joint draws (probability matching); as few
  rails as fit discovery's slots under the cap (half of discovery), plus any arm whose chance is worth a rail; each a
  rail's minimum (6), the rest by chance. The plan is seeded by profile, day and hour, so Home holds still within an
  hour. Arms choose in the plan's order.
- **Measured** (docs/eval-results.md, 18.2): E1 a changed favourite moves +35 and -21 points of discovery in 60 days; E2
  a viewer who refuses every discovery card still gets 20% of the slots, every arm on at least 23% of days.

## 6. Memory and dismissals (migration 0034)

`discovery_shown` keeps the arm that first showed each title to a profile, and the part of the day. What came of it is
read from viewings (an episode counting for its series), the watchlist, ratings (4+ completed, 2 or less not for me) and
`dismissals`, after the title was shown. "Not for me" and "already seen" leave every rail for good and are taste
signals (the first weighs as an early abandon); "not right now" for 14 days. X on a pad, the context-menu key or a
right click opens them on a recommended card; the card stands as an Undo, and a toast says what follows.

## 7. Surprise me and "I have 90 minutes" (`crates/taste/src/surprise.rs`, I42)

One film from what Home would recommend now (the taste rails and the discovery rails; for a profile with no taste yet,
new releases and the widely loved), films only, what is on this PC first, at random among the best five, opened on its
page. The Home button, and RT on a pad from anywhere while browsing. Home's length chips (any, under 90 minutes, under
2 hours) narrow the recommendation rails and Surprise me; a title with no known length is left out under a limit, and
what the profile started, saved or keeps on this PC is never cut.

## 8. Mood shelves (`crates/taste/src/mood.rs`)

Six moods, each a sentence asked of the meaning index as a query is ("a slow, contemplative and visually beautiful film
of long quiet takes"), "Tense, under 100 minutes" with its length kept. A shelf is the mood's 300 nearest titles, each
part scaled 0 to 1 over them, scored `mood + 0.5 x nearest taste + 0.2 x popularity` (`ln(1 + votes)` over the most
among them), nothing another rail shows. Two moods a day, in turn; each sentence is embedded once per run of the app.

