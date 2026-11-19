# Case study — the discovery engine

Phase 18, November 2026. Every number here is in `docs/eval-results.md` ("Phase 18") with the command that produced
it. The examples are one profile's Home on a copy of the development data: 27 well-known films rated (Tarkovsky,
Bergman and Ozu; Ghibli; American crime from the 1970s to the 1990s; Wong Kar-wai and Korean cinema), printed by
`eval recommend --rail 2`.

## The problem

Phase 17's rail answers "what will this person like?", and answers it well: on people it never learned from it finds
61% more of their held-out favourites than "the most popular" does. But a recommender that only ever answers that
question narrows. Each round it shows what it is surest of, so each round it learns only about that, and a person who
loves Tarkovsky is shown Tarkovsky until there is no more. The spec's goal for Phase 18 is the opposite of that:
recommendations that are *surprising and right*. Surprising alone is easy (pick anything); right alone is Phase 17.

Three things make it hard here. There is no server and no other users, so "people like you went on to love" has to
come from 160,000 strangers in a ratings dataset. Every suggestion has to say why, and the why has to be true: "Your
blind spot: Iranian New Wave" is a claim about the person, and a false one is worse than none. And it has to be
measured, not admired.

## What was built

**Five ways of looking past the obvious** beside Phase 17's rail, each a pure function over described titles
(`crates/recommend/src/strategy.rs`):

| Arm | What it chooses | A card from the example profile |
|---|---|---|
| Stretch | close to one of your tastes, one step outside it (a decade, a genre, a country, a movement) | *Your Name.*: "Like Princess Mononoke, but from the 2010s" |
| Bridge | close to two of your tastes at once | *Inglourious Basterds*: "Between Apocalypse Now and The Godfather" |
| Blind spot | acclaimed films from somewhere you have never been | rail: "You've never watched anything from the 1950s" (from the tests: this profile has met every decade) |
| Canon | acclaimed, widely seen, close to a taste, not yet seen | *Kiki's Delivery Service*: "Acclaimed, and close to My Neighbor Totoro" |
| Deep cut | highly rated, little seen | "Loved by people who loved Mirror, and little seen" |

**Labels are claims, checked.** A pick carries a `Label` (data, not words), and `strategy::holds` re-derives it from
the facts independently of the code that chose it. A property test runs every arm over 300 generated profiles and
checks every label; it was seen to fail when the arms were made to take titles just short of "close", and when the
blind spot ignored what the profile had met. On real MovieLens users, the evaluation checks every label too: 467,741
of them, none false.

**Countries and movements from Wikidata** (ADR-0055). The IMDb datasets carry neither, so "Your blind spot: Iranian New
Wave" could never have been said. The hourly check asks Wikidata (P495, P135) by IMDb id: 69,124 titles in 2,150
seconds, 93% with a country. A country is claimed as a blind spot only once every title the profile has met has had its
country asked; until then the axis is *unknown* and nothing is said on it.

**A bandit chooses how much of Home each way gets** (`crates/recommend/src/bandit.rs`). Thompson sampling: each arm's
chance of a good outcome is a Beta belief, `Beta(1 + successes, 1 + failures)`, built from what came of the titles that
arm showed (finished, abandoned, saved, rated, dismissed), faded at a 60-day half-life, kept per part of the day.
Each Home draws from the beliefs and shares 60 slots by each arm's chance of being the best. There is no "explore 10%
of the time" setting: an arm little tried has a wide belief and sometimes draws high, so exploration shrinks by itself
as evidence grows. Two guards make it safe: a **floor** (discovery keeps at least a fifth of the slots, whatever the
profile does) and a **cap** (no discovery arm takes more than half of discovery's).

**The person steers it.** A Comfort to Adventure slider sets where discovery's share starts; finishing what discovery
chose pushes it further, abandoning it eases it back. Every recommended card takes "Not for me", "Not right now" and
"Already seen" (X on a pad), each fed back: to the bandit as an outcome, to the taste model ("not for me" weighs like an
early abandon), and to every rail (never shown again; "not right now" for a fortnight).

**And the rest of 18.4:** Surprise me (a button, RT on a pad: one film from what Home recommends now, what is on this PC
first, opened ready to play), "I have 90 minutes" (length chips that narrow Home and Surprise me), and mood shelves
("Slow and beautiful", "Tense, under 100 minutes": sentences searched by meaning, two a day).

## How it was judged

Every bar was written into `docs/eval-results.md` before the run it judges, so a number could not choose its own bar.

- **E1, the bandit follows a change: met.** 1,000 simulated viewers, 120 days; each has a favourite discovery arm that
  changes on day 61. The new favourite's share of discovery went from 12.8% to 47.8% (+35.1 points; bar +15), the old
  one's from 49.0% to 26.9% (-22.1; bar -15), and it rose for 98.8% of viewers (bar 80%). Frozen beliefs move it 0.1.
- **E2, the floor holds: met.** 1,000 viewers who refuse every discovery card for 120 days, on Comfort: discovery keeps
  20.0% of the slots every day, there is no day without a discovery rail, and every arm is shown on at least 16.7% of
  each viewer's days. With the floor removed (`--no-floor`) discovery falls to 0% on all 120,000 days.
- **E3, labels are true: met.** Above: 300 generated profiles in the tests, and 467,741 labels on real users, none
  false.
- **E4, novelty with relevance held.** On 15,640 MovieLens test users, Home as a new profile sees it. Novelty is
  self-information, `-log2` of the share of people who liked a title: one bit more is half as widely liked. Relevance is
  Recall@N of everything Home shows (taste and discovery together, about 60 cards) against popularity's top N. The bar:
  discovery at least one bit more novel than the taste rail, and Home more than 40% above popularity. **Not met, by
  0.1 point:** discovery is 1.89 bits more novel than the taste rail (about 3.7 times less widely liked), and Home is
  **39.9%** above popularity (0.3626 against 0.2593), with the taste rail itself unchanged at Phase 17's 0.2639.

## What went wrong, and what it taught

**The bandit's first allocation barely learned.** Sharing slots by one draw per arm, over as many rails as fit, gave
every arm about a rail's minimum of six: at 25 to 30 discovery slots, five arms at six each fill nearly all of them, so
a learned preference could only decide which arm was left out. E1's first run moved the new favourite +12 points against
a bar of 15. Sharing by each arm's *chance of being the best* (probability matching over 100 joint draws), over as few
rails as fit, moved it +35.

**E4 failed three times before it passed, and the failures were the useful part.**

1. The arms first ordered their picks by closeness, rating or region. On 100 users the discovery cards found a held-out
   favourite once in a hundred cards, a fifth as often as the most popular titles, and Home was 16% above popularity.
   Ordering each arm's picks by Phase 17's relevance (the arm decides *why* a title may be shown; relevance decides
   *which first*) gave **+42% on those 100 users and +28.8% on all 15,640**. A sample of 100 flattered the change by
   thirteen points; every later change was tried on 2,000 users (which tracked the full set within a point) and judged
   once on all of them.
2. Weighting the matrix's part more in that relevance changed nothing (+27.5%).
3. **A bug, found by measuring.** At the middle setting the plan gave the familiar 30 slots and Home showed 20, so
   discovery had six cards in ten where the plan meant five. Showing the familiar's whole share (a second taste rail
   past twenty) gave +33.2%.

Per arm the numbers showed why: canon found held-out favourites as often as the most popular titles (0.05 a card),
deep cuts almost never (0.002: a film with fewer than 15,000 IMDb votes is seldom a MovieLens favourite), while deep
cuts carried the novelty (11 bits) and canon had less than the taste rail's. Blind spots cost relevance and bought
little novelty. A new profile, whose bandit knows nothing, gave each arm the same share, the worst of both.

**The fix was a population prior**, chosen by a rule fixed before it ran: each arm's prior value is its relevance and
its novelty, each over the best arm's, so neither alone decides (a prior from relevance alone would have dropped deep
cuts and failed novelty); its strength was chosen on users set aside for tuning, and the test users judged once. It is
used only to share discovery's slots between its arms, so the person's budget, not the population's, still decides how
much of Home is discovery, and a profile's own outcomes outweigh it within weeks. With it, E4 measured **+40.4%**: met,
by 0.4 point.

**Then the real window changed the arms, and E4 slipped back.**

**The real window found what no metric did.** Printing one profile's Home showed four things the evaluation could not
see: *Kiki's Delivery Service* was "close to Tokyo Story" (a taste was named by its strongest film, though it also held
the Ghibli films; now it is named by its film nearest the pick: "close to My Neighbor Totoro"); *Ghost in the Shell*
was "Like Tokyo Story, but from the United Kingdom" (true, a co-producer, and absurd; a country now counts as a step out
only when none of the film's countries are the taste's); bridges like *Tokyo File 212* (stretches and bridges now need
1,000 votes); and mood shelves opening on a 1912 short (scaled closeness let obscure plots win, and the sentence's own
word "epic" found *Epic Movie*).

The stretch and bridge arms had changed, so the prior's rule was run again from the start on the tuning users. It chose
a weight of 16, and the simulation the rule itself required showed 16 starving blind spots (2.5% of a refusing viewer's
days, against E2's 10%): a prior strong enough to win on relevance had stopped exploring. The next weight by the rule's
own order, 2, keeps E1 and E2; that departure is recorded, not hidden. Judged once on the test users with the honest
arms: **+39.9%**. The fixes stay; the 0.1 point is the next session's work, chosen on the tuning users and judged once.

## The side effect on search

Mood shelves depend on search by meaning, which had been carried since Phase 5 at nDCG@10 0.0472 against a target of
0.75: plot-first documents let hundreds of thousands of obscure plots outrank the films a person means. A popularity
prior (`cosine + 0.4 x ln(1 + votes) / ln(1 + most votes among the 200 nearest)`), chosen on ten new queries written
for tuning before any run, took the original eight from **0.0472 to 0.3276**, put both known-item queries' film first
(*Manchester by the Sea* for "a man returns to his hometown to care for his nephew after a death"), kept exact titles at
43 of 43, and cost about 10 ms at p95. Still short of 0.75, and "films about grief that aren't depressing" still returns
*Antichrist*: a meaning model does not do negation.

## What would be done differently

- **Write the per-arm diagnostics first.** Held-out likes per card by arm explained E4 in one run; three attempts came
  before it was printed.
- **Look at the real window before the full evaluation, not after.** The four things it found were cheaper to see than
  any number, and two of them (the co-production, the mis-named taste) were true labels that read as false.
- **Judge a metric's arms in the window before tuning the metric.** The prior was tuned, judged and met, and then the
  window changed the arms it was tuned on.
- **Offline relevance punishes discovery by construction.** A held-out favourite is a film the person found anyway;
  the deep cut they would have loved and never heard of cannot be in it. E5 (the author on their own taste) is the
  test that can see that, which is why the spec calls it the real one.
