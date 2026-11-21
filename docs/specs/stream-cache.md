# The stream cache, and searching a person

ADR-0057 (the author's answers of 2026-11-21).

## Streaming fetches only the watched file

`play::prepare` adds a torrent, chooses the file to play, and then narrows the torrent to that file
(`Engine::select_files`, librqbit's `update_only_files`). A torrent the engine already holds as a stream is narrowed to
the file now played; one held as a download keeps the files it was asked for. Before, a season pack or a collection was
fetched whole in the background for one episode.

## The cache

Streamed torrents live in the engine's own folder (`<data>/torrents`); a download is moved to the downloads folder when
it is asked for, so nothing in the engine's folder is a download. The cache holds at most a cap (20 GB unless chosen on
the Downloads page). `cache::evictions` decides what goes: the least recently watched first, never what is playing (the
stream opened last, and the one just opened), a stream never recorded first. When each was last opened is kept in
`<data>/torrent-session/stream-cache.json`. `Engine::trim_streams` runs at the engine's start (also removing anything in
the folder that no held torrent owns), after each stream opens, and when the cap changes.

## Searching a person

`SearchRepository::person_named`: the query (four words at most) is matched as a phrase against the index's people
column; of the credits of the first 50 titles found, a person whose name equals the query once case and accents are
folded (`normalise_query`) is the one meant, the best known of several, if their work has 10,000 votes in all. Their
films and shows, most voted first (`works_of`: directed, written or acted, never only produced), come after exact titles
and before every other tier, each saying "With", "Directed by" or "Written by".
