# 0057 — Disk use, the stream cache, and searching a person

- **Status:** Accepted 2026-11-21. The author's answers, question by question; this records them.
- **Date:** 2026-11-21
- **Phase:** between 18 and 19 (asked for before Phase 19 starts)
- **Amends:** Phase 7 (streaming), Phase 5 (search). `SPEC.md` §15 Phases 5 and 7; spec_version 4.10.0.

## Context

The author: the app "is occupying way too much storage on this computer ... delete anything and everything that isn't
absolutely necessary to build the app"; and "search is broken, i searched zendaya and it did not display a single movie
of hers". Measured before anything was deleted:

- The installed app's `data/torrents`: **524 GB**, 43 torrents that had been streamed, not downloaded. Streaming one film
  or episode added the whole torrent, so a season pack or a 95 GB collection was fetched entire in the background; and
  nothing ever removed a streamed torrent.
- The repository's `target/`: **295 GB** (146 GB of test binaries carrying every dependency's full debug information,
  137 GB of incremental caches); `src-tauri/target` 6.5 GB stale; old experiment builds 4.4 GB; 40 GB of scratch copies
  of data in Temp.
- Search: Zendaya's films and credits are in the catalogue and the full-text index finds her name, but text ranking puts
  titles and episodes *named* "Zendaya" first, so her films never reach the candidates; meaning search does not know
  names.

## Decision (the author's answers)

- **Every streamed torrent deleted** (524 GB), and removed from the torrent session so it is not fetched again.
  Downloads are the author's and are kept.
- **Streaming fetches only the file being watched**, never the rest of a pack.
- **The stream cache holds at most 20 GB**, least recently watched deleted first, never what is playing and never a
  download; the cap is a setting (Settings, Downloads).
- **Development data the measurements need is kept** (the dev catalogue, MovieLens, the IMDb datasets, the current
  meaning index); stale backups and the previous model's files deleted.
- **Build output is kept small for good**: dependencies carry no debug information, the workspace's own code keeps line
  tables, test builds keep no incremental cache, and `tools/doctor` warns when `target/` passes 60 GB.
- **Searching a person's name shows their films and shows, best known first**, each saying why ("With Zendaya",
  "Directed by Christopher Nolan"); never single episodes. Then anything else the query matches.

## Consequences

- A streamed film resumed after it has been evicted streams again; one within the cap resumes from disk.
- A search that is exactly a credited person's name (accents and case aside) puts their work above text matches, after
  exact titles: "Drive" still finds the film *Drive* first.
- The next clean build of the workspace takes longer once; later ones are no slower.
