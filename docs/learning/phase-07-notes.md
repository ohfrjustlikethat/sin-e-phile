# Phase 7 — Learning notes

**Torrent engine and streaming server.** Four sections per ADR-0016 (A4): what we
built, why, new concepts as concept + `file:line`, and the five questions.

---

## 1. What we built

- **The engine** — `crates/torrent/src/engine.rs`. librqbit in-process, with every file
  it writes under `data/`, the seeding policy, a dead-swarm deadline, and adapter binding.
- **The streaming server** — `crates/torrent/src/server.rs`. A small HTTP server on
  loopback that makes an in-progress torrent look like a seekable file to the player.
- **File selection** — `crates/torrent/src/select.rs`. The feature, never the sample;
  the episode asked for, or nothing.
- **Windows adapter binding** — `crates/vendor/librqbit-dualstack-sockets/`. librqbit
  could not bind to a network adapter on Windows; now it can, so a VPN dropping stops
  torrent traffic instead of leaking it.
- **Settings and the play step** — `settings.rs`, `play.rs`, and Settings → Network.
- **A test swarm** — `crates/torrent/tests/common/mod.rs`. Two engines on loopback, one
  seeding generated files to the other. No network, no URLs, runs in CI.
- **Live benchmarks** — `examples/bench.rs` and `tools/bench/qbittorrent_compare.py`.

## 2. Why

- **Streaming, not downloading.** A download fetches pieces in whatever order keeps the
  swarm healthy; a stream needs the next few seconds first. librqbit already keeps the
  32 MiB ahead of each reader at top priority, so the work was to drive it, bound it and
  measure it (`docs/specs/streaming-scheduler.md`).
- **The player should not know it is playing a torrent.** An HTTP range server is the
  universal interface: libmpv, FFmpeg and a browser all speak it.
- **Seeding stops at completion** because seeding is uploading and uploading is
  distribution (SPEC.md §15 Phase 7). The default protects the user.
- **Adapter binding** is the single highest-value safety feature in the phase: without
  it, a VPN that drops silently hands every torrent connection to the real network.
- **A swarm on loopback** is the only way to make a swarm behave badly on purpose — a
  slow seeder for the bandwidth floor, a dead one for the fast failure.

## 3. New concepts

### BitTorrent

- **Pieces and the piece picker** — a torrent is split into pieces (256 KiB in the test
  swarm, `tests/common/mod.rs:52`); which piece to ask for next is the whole scheduling
  problem. See the order in `docs/specs/streaming-scheduler.md` §1.
- **Info hash** — the SHA-1 of a torrent's metadata; its identity everywhere. Adding by
  hash alone lets DHT find the metadata (`engine.rs:242`, `Source::InfoHash`).
- **DHT, trackers, local service discovery** — three ways to find peers; all off in the
  tests (`tests/common/mod.rs`, `offline_config`).
- **Fastresume** — on restart, re-check what is on disk instead of starting over
  (`engine.rs:199`; tested by `a_download_survives_a_restart`).

### HTTP

- **Range requests** — `bytes=a-b`, `a-`, `-n`; `206 Partial Content` with
  `Content-Range`; `416` past the end; a multi-range request may be answered with the
  whole file (`server.rs:195`).
- **Why one request per connection** — mpv opens a fresh connection per seek anyway, so
  `Connection: close` removes keep-alive state from the server entirely.
- **A token in the path** — another program on the same machine can find the port, but
  not read a stream without the token (`server.rs:103`); compared in constant time
  (`server.rs:335`).

### Rust and Tokio

- **`AsyncRead + AsyncSeek` as a trait object** — `server.rs:32`. A torrent stream and a
  file on disk are interchangeable behind `Box<dyn ReadSeek>`.
- **`take(n)` to bound a copy** — `server.rs:298`: copy exactly the range, no more.
- **`OnceCell::get_or_try_init`** — `src-tauri/src/state.rs:132`. Start the engine on
  first use, exactly once, even if two commands race to be first; off the cold-start path.
- **Polling when there is no event** — `engine.rs:456`. librqbit exposes no completion
  event, so the seeding policy checks once a second.
- **Measuring lateness against a schedule** — `tests/buffer.rs:29`. A chunk is "late" by
  how long after its due time it arrived; a negative control proves the measure works.

### Windows networking

- **`IP_UNICAST_IF` / `IPV6_UNICAST_IF`** — `crates/vendor/librqbit-dualstack-sockets/src/bind_device.rs:81`.
  Pin a socket's outgoing traffic to one interface. IPv4 takes the index in network byte
  order, IPv6 in host order. When the interface is gone, sends fail rather than reroute.
- **`[patch.crates-io]`** — the root `Cargo.toml`. Replace one crate in the whole
  dependency tree with a local copy, without forking its users.

## 4. The questions

Written, not asked — Phase 7 is not a tier boundary (§10.10). They accumulate until the
end of Phase 8.

1. librqbit keeps a 32 MiB window ahead of each open stream at top priority. Why does a
   seek re-prioritise in hundreds of milliseconds rather than seconds, and what would a
   much smaller window cost?
2. The streaming server answers each request with a fresh reader seeked to the range
   start. What would go wrong if two concurrent range requests shared one reader?
3. Why does a finished torrent get served from the file on disk instead of librqbit's
   stream, and how does that interact with "seeding stops at completion"?
4. Bound to an adapter, a socket's traffic "cannot leave by another". Using the probe
   results in `crates/vendor/librqbit-dualstack-sockets/SINEPHILE.md`, explain what
   happens to a torrent's connections at the moment a VPN drops.
5. The bandwidth floor is 1.5× the bitrate. Why is exactly 1.0× not enough, even though
   on average the data arrives as fast as it is played?
