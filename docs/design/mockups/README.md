# Phase 2 mockups — three takes on one brief

Open **`index.html`** in a browser.

Three interpretations of `SPEC.md` §9.0 (ADR-0023), across three screens each:
Home, film detail, and the player with chrome visible. Same content, same palette,
same constraints — they differ in typography and layout, which is what is being
chosen between.

| | Direction | Display face |
|---|---|---|
| **Take A — Editorial** | Most MUBI-led. Bets on emptiness. | Instrument Serif |
| **Take B — Catalogue** | Criterion-led. Spine numbers, index column, rule lines. | Bricolage Grotesque 800 |
| **Take C — Signal** | Spotify-led. Heavy display, quiet metadata, denser. | Archivo 800 |

## What is real here

**The artwork.** Every still is a frame extracted with FFmpeg from a public-domain
film on the Internet Archive — the legal reference source §2.1 names. Nine films:
*Night of the Living Dead*, *Battleship Potemkin*, *Scarlet Street*, *Detour*,
*His Girl Friday*, *The General*, *Un Chien Andalou*, *Beat the Devil*.

**The metadata.** Real directors, years, runtimes, casts, and written synopses. The
brief forbids lorem ipsum and grey boxes because they hide the problems worth
seeing: how chrome behaves against a bright still, whether type holds at real title
lengths, whether a rail reads when the artwork is inconsistent.

**The fonts.** Self-hosted woff2 in `fonts/`, no network requests (§9.2).

## What every take obeys

- §9.0 banned list: no purple, no glassmorphism, no gradients as decoration, no
  emoji, no bento grids, no shadows as depth, no Tailwind defaults, no neutral sans
  as the display face, at most two radius values, deliberately irregular card sizes.
- §9.1 revised palette: warm near-black greys, non-linear value steps, oxblood
  reserved for intent and never a large fill.
- **ADR-0020**: the player chrome is a solid opaque panel with a hard edge. No
  gradient scrim, no blur-behind.
- **ADR-0013**: the film with no artwork gets a typographic card, designed to be
  the nicest thing in its rail rather than a fallback.

> The one gradient that does appear is a legibility scrim over *artwork* in the
> heroes — not decoration, and not the banned player-chrome scrim, which ADR-0020
> governs separately and which none of these use.

## Regenerating

```bash
# stills   (scratchpad script, ~5 min, needs network)
python get_stills.py
# fonts
python get_fonts.py
# screenshots
pwsh shoot2.ps1
```

## Status

**Awaiting the author's choice.** No token or component code is written until then
— ADR-0023 makes that explicit.
