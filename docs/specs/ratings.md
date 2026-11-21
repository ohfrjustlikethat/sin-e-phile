# Ratings for any title

ADR-0056 (the author's answers of 2026-11-20). What is built, and the rules it keeps.

## 1. Half stars

A rating is 0.5 to 5 stars in half steps, like Letterboxd, and no reviews. Ratings are stored as **halves, 1 to 10**:
`ratings.halves` (migration 0035; every whole-star rating already given was doubled, so nothing moved).

## 2. A show is rated season by season

`season_ratings (profile, series, season, halves)`. A show has no rating of its own any more: what every reader sees is
the **average of its rated seasons**. A show rated as a whole before seasons existed (onboarding's "I love this") keeps
that rating until a season is rated, which then replaces it.

## 3. One reader for every consumer: `title_ratings`

A view (migration 0035): a title's own rating, or a show's seasons' average, per profile. The taste model's signals
(`ProfileRepository::ratings`), liked titles (`liked`), the discovery engine's outcomes (`DiscoveryRepository::happened`),
the countries lookup's "met" titles (`RegionRepository::awaiting_met`), the model's fingerprint and `rated_count` all
read it, so they can never disagree about what a profile rated.

## 4. What a rating says to the taste model

`modes::rating_weight(stars)`: whole stars weigh what they always have (5: 1.6, 4: 1.0, 3: 0.2, 2: −0.6, 1: −1.0); half
stars lie on the line between; half a star is −1.2. Strongly held (fading four times slower) from 4.5. **Stars and
watching both count** (the author's answer): a rating adds to what finishing or abandoning already said.

`liked` (the taste seed, "loved with", rounds): four stars and up, weight 0.7 at four, 0.85 at four and a half, 1.0 at
five, as MovieLens' `cf::LIKED` = 4.0. A completed watch with no rating is 0.3.

The discovery engine reads a rating as `completed` from four stars, `not_for_me` at two or fewer, `played` between.

## 5. Imports

Every import already reads a rating out of ten (Letterboxd's stars doubled; IMDb, Trakt and MyAnimeList are out of ten).
Out of ten is halves, one to one (`imports::halves`): a Letterboxd 3.5 stays 3.5, an IMDb 7 is three and a half stars.

## 6. Where a rating is given

- **A film's page**: a row of stars on its own line under the title and year, "Rate" beside it until rated.
- **A show's page**: "your average" beside the byline; the chosen season's header carries its stars and episode count.
- **Any card's menu** (X on a pad, right click, the context-menu key): a film's stars; a show's "Rate seasons…" opens
  its page.
- **The end card** after a film.
- **Rate films**: one well-known film after another (`taste::rate::batch`: films with at least 100,000 IMDb votes, most
  voted first, never one rated, passed over or dismissed; a kids profile only what suits children). "Haven't seen it"
  is stored in `rate_skipped` and is no signal of taste. Opened from Your taste and from Home's card while a profile has
  fewer than ten ratings.

The control (`StarRating`) is a native range input, 0 to 10, over five drawn stars: the arrow keys, the pad's Left and
Right and the focus ring are a slider's; the mouse rates by position on the stars, and clicking the rating given clears
it. A change is saved once the stars stop moving (350 ms).

## 7. Removing from Continue watching

`continue_hidden (profile, title, hidden_at)`: the film or show leaves the row and its place is kept; it is back once a
position of it (or of one of the show's episodes) is saved after it was hidden, that is, once it is played again.
Reached from the card's menu, an × on the card for the mouse, and the title's page.
