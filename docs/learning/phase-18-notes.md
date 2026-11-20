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
- **Surprise me** (a word in the masthead left of Search, RT on a pad): one film from what Home recommends now, on this
  PC first, opened on its page. ("I have 90 minutes" was built, then removed by the author: ADR-0056.)
- **Ratings for any title** (ADR-0056, the author's answers): half stars, 0.5 to 5; a show rated season by season, its
  average shown; from the title page, every card's menu, the end card and a **Rate films** screen of well-known films.
  Stars and watching both feed the taste model.
- **Remove from Continue watching**: hidden, its place kept, back once played again.
- **A cancelled download that restarted itself**, fixed: the queue's once-a-second pass and the viewer's actions now
  take turns.
- **Mood shelves**: six moods as sentences searched by meaning ("Slow and beautiful", "Tense, under 100 minutes"), two
  a day, each shelf leaning to the profile's own tastes.
- **A population prior** for the bandit (blocker B17): what a profile with no history is likely to find in each arm,
  from users set aside for tuning, so a new profile starts with the arms that tend to land.
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
  fell to 16% above popularity's (100 users); ordered by relevance, 29% over all 15,640 test users, with novelty 1.9
  bits above the taste rail's. (The first 100 users said 42%: a small sample flatters.)
- **Novelty is measured in bits**: `-log2` of the share of people who liked a title. One bit more is half as widely
  liked; the discovery rails' picks are on average about three times less widely liked than the taste rail's.

- **Ratings are what recommendations stand on** (the author's words). Half stars keep a 3.5 distinct from a 4: on the
  taste harness the model then beats popularity by more (1.17x against 1.14x), on a stricter set of favourites.
- **One view, every reader**: a show's rating is its seasons' average everywhere because there is one place that says
  so (`title_ratings`), not five copies of the rule.
- **A race needs a turn, not a retry**: the download queue worked from what it read a second ago, so a cancel in that
  second was undone. Making the two take turns removes the window; a test that holds the pass open shows it.

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
- **A prior as pseudo-trials**: a population's evidence counted as a few imaginary outcomes, which a profile's own
  outweigh as they come — `crates/recommend/src/bandit.rs:169`.
- **Tuning and test users kept apart for a choice**: the prior's weight chosen on one set, judged once on the other —
  `tools/eval/src/discovery.rs` (`--tune`), `docs/eval-results.md` (B17's rule).
- **A tuning fixture written before the run**, so a search setting is not chosen on the queries that judge it —
  `fixtures/search/meaning-tuning.tsv`.
- **Scaling parts over a pool before weighing**: cosines near a mood sit in a narrow band — `crates/taste/src/mood.rs:172`.
- **A rotation by day**: two moods a day, every one within the week — `crates/taste/src/mood.rs:78`.
- **Embedding once, keeping it**: a mood's sentence is the same all run — `crates/taste/src/mood.rs` (`MoodVectors`).
- **Keeping the previous page while the next loads** (`placeholderData: keepPreviousData`), so a filter does not blank
  the screen — `src/features/home/HomeScreen.tsx`.
- **A pad button as a keyboard key**: X sends the context-menu key to whatever has focus — `src/app/App.tsx`
  (`case "options"`).
- **Capture-phase listeners at the window**, to answer a synthetic Escape before the app's Back does —
  `src/features/home/MenuCard.tsx`.
- **An async mutex as a turn** (`tokio::sync::Mutex<()>`: holding it is the turn, nothing is inside) —
  `crates/downloads/src/manager.rs:86`, taken at `:168`.
- **Reproducing a race deterministically**: a fake that stalls inside the pass until told to go (`tokio::sync::Notify`)
  — `crates/downloads/tests/cancel.rs`.
- **A SQL view** as the single reader of a rule (a title's rating, or its seasons' average) —
  `crates/persistence/migrations/0035_half_stars.up.sql:32`.
- **Rebuilding a SQLite table** to change a CHECK (create, copy, drop, rename) — `0035_half_stars.up.sql:5`.
- **Piecewise-linear interpolation** between known weights — `crates/taste/src/modes.rs:52` (`rating_weight`).
- **"Back once played again" as a comparison of times**, not a flag to clear — `profiles.rs:424`
  (`hidden_from_continue`).
- **A native control under a drawing**: a transparent range input over five stars gives keys, pad and screen readers a
  slider for free — `src/design-system/primitives/StarRating.tsx:45`.
- **Optimistic updates and in-flight loads**: set the new value, and cancel the load without reverting, or it puts the
  old value back — `src/features/ratings/useRating.ts:53`.
- **Saving once the input settles** (debounce: a timer replaced by each change) — `useRating.ts` (`settle`).

## 4. Five questions

1. Thompson sampling never has an "explore" setting. Where does its exploration come from, and why does it shrink on
   its own?
2. Why did sharing slots by one draw per arm over as many rails as fit barely follow a changed favourite, and what
   about probability matching fixed it?
3. The exploration floor keeps discovery at 20% even for someone who refuses every discovery card. What would the
   bandit do without it, and why is that a failure rather than "giving people what they want"?
4. Why may the app say "You've never watched anything from Iran" only once every title the profile met has had its
   country asked, when a decade can be claimed at once?
5. On 100 users relevance ordering looked like it cleared E4's bar (+42%); on 15,640 it did not (+28.8%). Why does a
   small sample mislead here, and what does a bit of novelty mean?

(Added with ADR-0056, for the next gate: why does cancelling a download need the queue to wait its turn, when the
cancel already marks the row cancelled? And why is a show's rating a view over its seasons rather than a number stored
when a season is rated?)
