# The discovery engine (Phase 18)

Status: 18.1 built (the arms, their labels, countries and movements). 18.2 (the bandit), 18.3 (dismissals and the
budget) and 18.4 (Surprise me, mood shelves) extend this file as they land.

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
(movement), English labels, by IMDb id, 200 a query at one query in three seconds, 12,000 titles a run. Met titles
first, then films and series with 1,000 votes, most voted first. `title_regions` holds the answers and `regions_asked`
every title asked. `ingest regions` runs it to the end on a data folder.

## 4. Gathering (`crates/taste/src/discover.rs`)

The pool: each mode's 1,000 nearest (asked of the whole index), the acclaimed (up to 6,000), and Phase 17's
neighbours of what the profile loved. Facts from `MediaRepository::facts` (rating, countries and movements added in
Phase 18). Home shows each arm with at least five picks as a rail after "Where your taste points", none repeating a
title shown elsewhere on Home. Which arms and in what order is the bandit's (18.2); until then, `Arm::ALL`'s order.
