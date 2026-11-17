# Settings: a side list, add-on cards, and every change said

*Phase 16, the author's brief item 8 (`docs/design/author-brief-2026-11-17.md`, rounds 4, 6 and 7). The author:
"too long and hard to find things… adding local library feels like you did nothing… after adding an addon or a key
or connecting local library, the app should tell that something happened".*

- **The side list** (`SettingsScreen`): Profile, Library, Add-ons, Playback, Subtitles, Downloads, Network,
  Controller, About; one section beside it. The open section is kept in the store (`settingsSection`), so
  Settings reopens where it was, scroll included (the scroll key is `settings|<section>`). Opening with nothing
  focused puts focus on the open section's entry, so the pad starts in the list. `openSettingsAt` maps the
  places other screens link to ("Add a source", "Library", "Subtitles", "Network") to their section.
  - Library holds the folders, the catalogue's refresh and the TMDB key; About holds the machine, its tier and
    capabilities, where the data lives, and the privacy statement.
- **Add-ons as cards** (`AddonCard`): a monogram (an add-on's own logo needs a remote image, which the app's
  content policy does not load; it comes with item 10's artwork, fetched by the backend), the name and version,
  "Stremio add-on · Films and series · asks a.example", the health line, on/off, earlier/later (the order its
  streams are listed in), Configure (`addon_configure`: by id, never by address, it opens the add-on's own
  `<base>/configure` page in the browser, and only for a manifest that says it is configurable), and Remove.
  Built-ins are cards too, without version, Configure or Remove.
- **Every change said** (`src/lib/toast.ts`): inline where it happened and in a toast. An add-on added: "Added ✓"
  on its card and "Add-on added: <name>…". A library folder added: "Added ✓" on its row, the live scan line, then
  "Library folder added: 203 files found, 199 recognised, 4 to check." when its first scan ends (or that it waits
  for the catalogue). Keys saved or removed (TMDB, OpenSubtitles), subtitle and network settings saved, an add-on
  turned on or off, removed, or its settings opened. Errors say what went wrong, in red.

Evidence: `src/features/settings/SettingsScreen.test.tsx`, `SourcesSection.test.tsx` (cards and "added"),
`LibrarySection.test.tsx` (a folder's result toast), each seen to fail; `crates/sources/tests/addons.rs` (a
Stremio add-on's card fields); `source-protocol` `stremio` (the configure address); the pad audit's Settings
steps (Menu lands in the side list; Add-ons, Library and Profile chosen by pad; the field, "Check them" and the
taste profile reached inside them; Settings reopens where it was left).
