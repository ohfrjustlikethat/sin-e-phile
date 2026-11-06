# Source scoring

**Status:** built in Phase 9 (subtasks 9.1–9.4) · **Code:** `crates/selection/` ·
**Tests:** `crates/selection/tests/fixtures.rs`, `crates/selection/tests/properties.rs`

Which copy plays when the viewer presses Play. Every candidate the sources return, whether
it is a file on this machine, a direct stream or a swarm, is scored by one pure function,
`score(candidate, context) -> Score`. Nothing in it reads a clock, a disk or the network,
so a ranking can be reproduced in a test and shown term by term in the expert panel.

## 1. The order

Ranking is **lexicographic**, because some things are rules and should never be traded
against points:

1. **Sunk last.** A copy that is not what it says it is (§3) comes after every honest
   one, however healthy or sharp it claims to be.
2. **Unmet choices next to last.** If the viewer chose a language or a quality, a copy
   *known* not to have it comes after every copy that has it or might. "Might" matters:
   a copy whose languages are not stated stays ahead of one that states the wrong ones.
3. **This machine first.** Among the rest, a local file comes first. SPEC.md Phase 9
   says "a local file always wins", which is a rule, not a weight.
4. **The weighted sum**, highest first: `total = Σ value × weight`, each value in −1..=1.
5. **The key**, so equal totals order the same way on every run.

`Score` keeps `sunk` and `unmet` as sentences. The UI shows them as they are: "carries en,
not hi" is the honest answer to "why isn't this Hindi?" (Phase 9 E2).

## 2. The terms and their weights

| Term | Weight | +1 | 0 | −1 |
|---|---|---|---|---|
| `language` | 4 | carries the chosen audio language | none chosen, or not stated | states other languages only (also **unmet**) |
| `quality` | 3 | the wanted resolution (else the machine's best) | not stated | above what the machine plays smoothly, when nothing was chosen |
| `plausibility` | 3 | well above the floor for its resolution | thin but possible, or not stated | below the floor (**sunk**) |
| `duration` | 2 | within −20% and +30% of the runtime | not stated, or much longer | under 80% of the runtime (**sunk**) |
| `health` | 2 | served directly, or a big swarm | availability unknown; 5 seeders | no seeders |
| `compatibility` | 2 | a codec this machine decodes in hardware | codec not stated | — (worst is −0.5, H.265/AV1 in software; HDR on an SDR display costs 0.5 more) |
| `throughput` | 2 | local, or the connection has 1.5× the bitrate | not measured; 1–1.5× | slower than the bitrate |
| `encoding` | 1 | names its source and encode | no details | a camera recording; a name that contradicts the listing or a non-video extension (**sunk**) |
| `subtitles` | 0.5 | carries the wanted subtitles | otherwise | — |

Quality steps: each resolution below the target costs half the term (1080p for a 4K
choice scores 0.5), and a copy above a lower choice scores 0.5. Health for a swarm is
`(s − 5) / (s + 5)` for `s` seeders: −1 at none, 0 at five, 0.82 at fifty.

### Why these weights

- **Language is heaviest (4)** because the wrong language is the one failure a viewer
  cannot fix after Play: a soft picture is watchable, a film in a language they do not
  speak is not. It is also a rule (*unmet*) when chosen explicitly; the weight decides
  between copies when the choice is unknown.
- **Quality and plausibility (3)** are what the viewer notices next. They are equal so
  that a lie about quality costs as much as the quality would have earned. A fake that
  claims 4K gains +3 from `quality` and loses −3 from `plausibility`, and it is sunk
  regardless.
- **Health, duration, compatibility and throughput (2)** decide whether it plays at all,
  or plays smoothly. Each is worth less than a resolution step because the order
  already handles the worst case: a dead copy fails to start and the next one is tried
  (9.4).
- **Encoding (1)** is a weak signal, and is kept weak on purpose. A name that states
  "BluRay x264" is *more likely* honest than a bare one, but a name is cheap to fake.
  The strong evidence is bytes per minute.
- **Subtitles (0.5)** only break ties: subtitles can come from elsewhere (Phase 10).

## 3. Fakes and mislabels (9.2)

- **Bytes per minute.** Megabytes per minute against a floor for the claimed resolution:
  SD 1, 720p 3, 1080p 5, 4K 12 for H.264 or MPEG-4, and 60% of that for H.265, AV1 and
  VP9 (and for an unknown codec, the lenient assumption). The floors catch lies, not
  thin encodes: 12 MB/min is about 1.6 Mbit/s, a third of the leanest real 4K streams.
  The length used is the file's own if stated, else the catalogue's. **A 700 MB "4K" file
  of a 140-minute film is 5 MB/min, below 7.2, so it is sunk.**
- **Duration.** A file under 80% of the catalogue runtime is a different thing: a
  trailer, an episode, a cut-down copy. **A 60-minute file for a 140-minute film is
  sunk.** Up to 30% longer is fine (extended cuts, extras); beyond that it scores 0, not
  −1, since a longer file is usually the film plus something.
- **Structure of the name.** In the title only, since a key is opaque and `.ts` is a
  real container:
  - an executable or archive extension (`.exe`, `.scr`, `.bat`, `.cmd`, `.msi`, `.lnk`,
    `.zip`, `.rar`, `.7z`, checked in the key too) means it is not a video, so it is
    **sunk**. `.com` is left out because too many titles end in it.
  - a resolution in the name that contradicts the listing ("720p" listed as 4K) is a
    mislabel, so it is **sunk**.
  - camera tags (`CAM`, `TS`, `TC` and their variants) mean it was filmed off a
    cinema screen. This scores −1 but is not sunk: it is real, just bad.
  - source tags (`BluRay`, `WEB-DL`, `HDTV`…) and encode tags (`x264`, `HEVC`…) count
    as a weak sign of care.

**"Release-group reputation" is structural only.** No group, site or indexer is named
anywhere in the code, the tests or this file (SPEC.md §2.1): such a list would be a
catalogue of where to find things, and it would be out of date within a month anyway.
What can be judged from structure is judged: whether a name is consistent with itself
and with the bytes.

## 4. The machine

`Machine` comes from `tiers.rs` through `sinephile_playback::machine`:

- `max_resolution` is 4K where `HighResPlayback` is allowed, and 1080p otherwise.
- `hevc` is set where hardware decode was found.
- `av1` and `hdr_display` are not probed yet, so both are assumed absent. Guessing low
  costs a little ranking; guessing high costs a stutter.

## 5. The viewer's choices (9.3)

- **What is offered** is `offers(&ranked)`: the audio languages and resolutions that some
  *honest* copy states. A fake's claims offer nothing, because choosing them could only
  play the fake. Languages are primary subtags (`hi`, not `hi-IN`), most widely carried
  first; qualities are best first.
- **On the detail page** these are two rows of options under Play, Audio (Auto, then each
  language) and Quality (Best, then each resolution). They are buttons, not a
  `<select>`: a native select opens a Windows popup the pad cannot drive (ADR-0041), and
  each option is a focus stop for the TV focus ring (ADR-0042).
- **Original or dub.** Each language is labelled "original" or "dub" against the film's
  `original_language`. For anime that comes from AniList's `countryOfOrigin` (JP → ja,
  KR → ko, CN/TW → zh; anywhere else is left unknown rather than guessed). With no
  original language known, the labels are left off rather than guessed.
- **Choosing re-plans** with `Context.audio_language` or `Context.wanted`. The choice is
  remembered across films (`sinephile.choice` in local storage), the way a TV app keeps
  the language you watch in.
- **When nothing has it**, the copy that plays is the best other one, and the page says
  so: "None of your sources has Hindi audio." The chosen option stays visible and
  selected, so it can be changed. Play is never disabled for want of a choice.

## 6. Fallback (9.4)

`crates/playback/src/fallback.rs`. The plan keeps every candidate with its score and
which one it opened; `PlayPlan::fallbacks()` is every **honest** copy ranked below it. A
sunk copy is never tried, even last: a fake is worse than saying nothing would start.

| Signal | Action |
|---|---|
| mpv reports an error (404, refused, not a video) | next copy at once |
| nothing loaded within **8 s** | next copy |
| loaded, no frame within **30 s** more | next copy |

The 8 s is the budget: a dead first choice plus the next copy's start fits E4's 10 s
(measured 8.1 s). A copy that *has* loaded is downloading, and slow is not broken: the
Internet Archive takes up to 8 s to a first frame. Each next copy is resolved only when
needed, so a swarm nobody plays is never joined. A newer Play or a close stops an old
loop before it opens anything (a generation counter in `src-tauri`). Each skip is logged
with its reason; the viewer sees nothing unless every copy fails, and then the player
shows every reason.

## 7. Known limits

- A film genuinely titled with a camera tag as a word (*Cam*, 2018) takes the −1
  `encoding` term. It is not sunk, and a local copy still comes first.
- `original_language` is filled only for anime matched to AniList, and only on an
  ingest or refresh after this change. Films from IMDb have none, so they show no
  original/dub labels.
- A swarm abandoned by fallback stays in the torrent engine until it is cleaned up.
- Throughput is a term with no measurement behind it yet. `Context.throughput` is
  `None` until the engine reports a speed.
