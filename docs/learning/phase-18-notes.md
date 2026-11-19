# Phase 18 — Discovery Engine: learning notes

Four sections only (ADR-0016 A4): what we built, why, new concepts, the five questions.

## 1. What we built

- **Five discovery arms** beside Phase 17's taste rail (`crates/recommend/src/strategy.rs`): a **stretch** (close to
  one of your tastes, one step outside it: "Like Stalker, but from the 1990s"), a **bridge** (close to two of your
  tastes at once), a **blind spot** (acclaimed films from a decade, genre, country or movement you have met nothing
  from), the **canon** (acclaimed, widely seen, close to a taste) and **deep cuts** (highly rated, little seen).
- **Labels that are claims**: each pick carries a `Label`, and `holds` re-derives it from the facts. Tested on 300
  generated profiles, and checked on every pick for real MovieLens users by `eval discovery` (none false).
- **Countries and movements** (ADR-0055): the IMDb datasets have neither, so the hourly check asks Wikidata. 93% of the
  70,000 most-voted titles came back with a country; 404 with a movement.
- **The bandit** (`crates/recommend/src/bandit.rs`): Thompson sampling over the six arms, one belief per arm per part
  of the day, learned from what came of the titles each arm showed (finished, abandoned, saved, rated, dismissed).
  It shares 60 slots a Home: the taste rail at most 20, discovery between 12 (the floor) and 48.
- **The exploration budget**: a Comfort to Adventure slider on the taste page sets where discovery's share starts;
  finishing what discovery chose pushes it up, abandoning it eases it down.
- **Not for me, not right now, already seen** on every recommended card: X on a pad, the context-menu key, a right
  click. Each feeds the bandit, the first is also a taste signal, and every rail leaves the title out.
- **`eval discovery --simulate`** (E1, E2) and **`eval discovery`** (E4, E3 on real users).

## 2. Why

- **A recommender that only exploits narrows**: every round it shows what it is surest of, so it only ever learns more
  about that. A filter bubble is this optimisation failing, not a law of nature; giving some slots to arms it is unsure
  of, and keeping a floor, is the fix.
- **Thompson sampling, not "explore 10%"**: an arm's slots follow how likely it is to be the best, given the evidence.
  A new or little-tried arm has a wide belief and sometimes draws high, so it gets tried; a tried and failed one
  rarely does. Exploration shrinks by itself as evidence grows, with no rate to tune.
- **Probability matching over a page**: the first allocation shared slots by one draw per arm over as many rails as
  fit, and a learned preference barely showed (+12 points in two months against a bar of +15). Sharing by each arm's
  chance of being the best, over as few rails as fit, showed it (+35).
- **Labels are part of the product**: "Your blind spot: Iran" is a claim about the person; a false one is worse than
  none. So country claims wait until every title the profile met has had its country asked.
- **Relevance within an arm**: the arm says why a title may be shown; Phase 17's relevance says which of those first.
  Ordered by closeness alone, discovery picks found a held-out favourite once in a hundred cards and Home's relevance
  fell to 16% above popularity's; ordered by relevance, 42% with novelty still 1.7 bits above the taste rail's.
- **Novelty is measured in bits**: `-log2` of the share of people who liked a title. One bit more is half as widely
  liked; the discovery rails' picks are on average about three times less widely liked than the taste rail's.

## 3. New concepts

- **The Beta distribution as a belief about a chance**: `Beta(1 + successes, 1 + failures)` —
  `crates/recommend/src/bandit.rs:198`.
- **Fractional rewards**: a "played" counts 0.6 of a success and 0.4 of a failure — `crates/recommend/src/bandit.rs:182`.
- **Forgetting by half-life**: an outcome's weight `0.5^(age / 60 days)`, so a changed taste is followed —
  `crates/recommend/src/bandit.rs:182`.
- **Borrowing strength between contexts**: an evening's belief takes 0.3 of the arm's other evidence —
  `crates/recommend/src/bandit.rs:198`.
- **Sampling Beta from two Gammas** (Marsaglia and Tsang), and Box-Muller for the normal they need —
  `crates/recommend/src/bandit.rs:252`, `crates/recommend/src/bandit.rs:272`.
- **Probability matching**: how often an arm's draw is the best of 100 joint draws is its share —
  `crates/recommend/src/bandit.rs:363`.
- **Water-filling under a cap**: each chosen arm a rail's minimum, the rest one slot at a time to whoever is furthest
  below its share — `crates/recommend/src/bandit.rs:347`.
- **Guards as data**, so a harness can remove one and show its verdict fail — `crates/recommend/src/bandit.rs:287`.
- **A claim checked independently of the code that makes it** (`holds`) — `crates/recommend/src/strategy.rs:267`.
- **Property testing without a library**: a seeded generator, 300 worlds, every pick checked —
  `crates/recommend/src/strategy.rs` (`every_label_of_every_arm_holds_on_generated_profiles`).
- **Closeness by rank, not by threshold**: the 1,000th nearest title's cosine defines "close" for each taste —
  `crates/taste/src/discover.rs:102`.
- **"Exactly one axis away"**: the regions a title has and a taste lacks, kept only when they are all on one axis —
  `crates/recommend/src/strategy.rs:372`.
- **Unknown is not empty**: an axis where something met was never asked claims nothing —
  `crates/taste/src/discover.rs:139`.
- **One rail, one region**: the blind spot prefers a region with enough titles to fill a rail —
  `crates/recommend/src/strategy.rs:489`.
- **Round robin across lists**, so every taste (or pair, or mode) is heard — `crates/recommend/src/strategy.rs:307`.
- **The strongest outcome per title, counted once**: played then finished then rated is one success —
  `crates/recommend/src/bandit.rs:85`.
- **A seed per hour**, so Home's plan holds still while the hour lasts — `crates/taste/src/home.rs:360`.
- **SPARQL with two OPTIONALs**: rows per pairing, gathered per title — `crates/metadata-api/src/wikipedia.rs:115`,
  `crates/metadata-api/src/wikipedia.rs:438`.
- **An episode as its series** in SQL (`COALESCE` over a `LEFT JOIN`) — `crates/persistence/src/repositories/regions.rs:52`.
- **Self-information as novelty**, and leaving out what has no share rather than calling it infinitely novel —
  `tools/eval/src/discovery.rs`.
- **A verdict written before its run**, and a first run that failed it recorded as it was — `docs/eval-results.md`
  (Phase 18).
- **A pad button as a keyboard key**: X sends the context-menu key to whatever has focus — `src/app/App.tsx`
  (`case "options"`).
- **Capture-phase listeners at the window**, to answer a synthetic Escape before the app's Back does —
  `src/features/home/RecommendedCard.tsx`.

## 4. Five questions

1. Thompson sampling never has an "explore" setting. Where does its exploration come from, and why does it shrink on
   its own?
2. Why did sharing slots by one draw per arm over as many rails as fit barely follow a changed favourite, and what
   about probability matching fixed it?
3. The exploration floor keeps discovery at 20% even for someone who refuses every discovery card. What would the
   bandit do without it, and why is that a failure rather than "giving people what they want"?
4. Why may the app say "You've never watched anything from Iran" only once every title the profile met has had its
   country asked, when a decade can be claimed at once?
5. Ordering each arm's picks by relevance cost 1.9 bits of novelty and gained 26 points of relevance. What does a bit
   of novelty mean, and why was the trade worth it?
