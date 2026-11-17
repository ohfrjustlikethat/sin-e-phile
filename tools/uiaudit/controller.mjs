/**
 * tools/uiaudit/controller.mjs — Phase 8 E5 with a simulated Xbox controller.
 *
 * "With only an Xbox controller, a user can browse to a film, start it, pause, seek and
 * leave the player." jsdom has no layout, so spatial navigation (which moves focus to
 * the nearest thing on screen in a direction) cannot be tested there. This runs the real
 * frontend in headless Chrome, where everything has a real position, with:
 *
 * - a fake backend behind `window.__TAURI_INTERNALS__` — the same seam the generated
 *   bindings call through, answering with the shapes the Rust commands return; and
 * - a fake pad behind `navigator.getGamepads()` — the API the app reads every frame.
 *
 * Then it presses buttons, and only buttons: Y to search, the D-pad and A to type on the
 * on-screen keyboard, A on the result, A on Play, A to pause, RT to seek, B to leave.
 * Each step asserts what the app did, and a screenshot of it is written to --out.
 *
 * Usage:  node tools/uiaudit/controller.mjs [--out <dir>]
 */
import { spawn, spawnSync } from "node:child_process";
import { createServer } from "node:net";
import { tmpdir } from "node:os";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { createServer as createHttpServer } from "node:http";
import { extname, resolve } from "node:path";
import { setTimeout as sleep } from "node:timers/promises";

const ARGS = process.argv.slice(2);
const OUT = ARGS.includes("--out") ? ARGS[ARGS.indexOf("--out") + 1] : "tools/uiaudit/out/controller";

const freePort = () =>
  new Promise((res, rej) => {
    const srv = createServer();
    srv.on("error", rej);
    srv.listen(0, "127.0.0.1", () => {
      const { port } = srv.address();
      srv.close(() => res(port));
    });
  });
const PORT = await freePort();
const CDP_PORT = await freePort();

const CHROME = [
  process.env["CHROME_PATH"],
  "C:/Program Files/Google/Chrome/Application/chrome.exe",
  "C:/Program Files (x86)/Google/Chrome/Application/chrome.exe",
  "/usr/bin/google-chrome",
].find((p) => p && existsSync(p));
if (!CHROME) {
  console.error("controller: no Chrome found. Set CHROME_PATH to a Chrome executable.");
  process.exit(1);
}

// ── the fake backend and the fake pad, installed before the app's first script ──
const FAKE = `(() => {
  const SUNRISE = { id: 7, title: "Sunrise", year: 1927, kind: "film", why: "exact_title",
    made_by: "F. W. Murnau", runtime_minutes: 94, excerpt: null };
  const searchResponse = (q) => ({ hits: /sun/i.test(q) ? [SUNRISE] : [], searchable_titles: 900000,
    catalogue_partial: false, semantic: true, ready: true });
  const answers = {
    search: (a) => searchResponse(a.query),
    search_backfill: () => ({ found: 0, added: [] }),
    optional_assets: () => ({ assets: [{ name: "Meaning index", purpose: "Lets you search by describing a film.", bytes: 329537568, state: "absent", unusable_reason: null }], outstanding_bytes: 329537568, complete: false }),
    // Appendix F, I21: marked by hand, and remembered for the rest of the flow.
    // Phase 16 E3: a film with chapters, for the bumpers.
    player_chapters: () => [{ time: 0, title: "Opening" }, { time: 600, title: "The city" }, { time: 1500, title: null }],
    // Appendix F, I39: the watchlist, saved from the film page and read by its own page.
    title_watchlisted: () => window.__saved === true,
    set_watchlisted: (a) => { window.__saved = a.on; return null; },
    watchlist: () => (window.__saved === true
      ? [{ id: 7, title: "Sunrise", year: 1927, made_by: "F. W. Murnau", runtime_minutes: 94, note: null }] : []),
    title_watched: () => window.__watched === true,
    set_watched: (a) => { window.__watched = a.watched; return 1; },
    item_detail: () => ({ id: 7, title: "Sunrise", year: 1927, kind: "film", runtime_minutes: 94,
      synopsis: "A married farmer falls under the spell of a woman from the city.",
      credits: [{ name: "F. W. Murnau", role: "director", character: null }] }),
    // The paused film's details, long enough to scroll (Phase 16's right stick).
    pause_details: () => ({ cast: Array.from({ length: 40 }, (_, i) => ({ person_id: i + 1, name: "Player " + (i + 1),
      part: "Part " + (i + 1) })), crew: [], ratings: [], more_like_this: [] }),
    prepare_play: () => ({ title: "Sunrise", open: "https://films.example/sunrise.mp4", start: null,
      from: "Internet Archive", offered: 2, languages: ["en", "de"], original_language: null,
      qualities: ["full_hd", "hd"], unmet: null, why: "1080p · served directly",
      // Phase 16: the streams panel lists these; Play, the round Download and each row use them.
      candidates: [0, 1].map((index) => ({ title: index ? "Sunrise (1927) 720p" : "Sunrise (1927) 1080p",
        from: "Internet Archive", transport: "http", quality: index ? "720p" : "1080p", codec: "h264", total: 5 - index,
        sunk: null, unmet: null, local: false, chosen: index === 0, terms: [], index, hdr: false,
        size_bytes: 4.2 * 1024 ** 3, seeders: null, direct: true, audio: ["en"], subtitles: [], weak: null,
        downloadable: true })) }),
    play_candidate: (a) => "https://films.example/sunrise-" + a.index + ".mp4",
    player_pause_still: () => "data:image/gif;base64,R0lGODlhAQABAAAAACw=",
    dev_autoplay: () => null,
    // Phase 16 E7: a fresh install, then what the first run sets up by pad, remembered for the
    // rest of the flow (and read by Appendix F I38's "Finish setting up" card).
    library_folders: () => window.__folders,
    // The Films and TV Shows pages (Phase 16), passed through on the way to Watchlist.
    library_titles: () => [],
    // ADR-0048: posters, off until chosen; the fake has none to show.
    artwork_settings: () => ({ posters: !!window.__posters, tmdb: !!window.__tmdbSet }),
    set_show_posters: (a) => { window.__posters = a.on; return null; },
    poster: () => null,
    // Phase 16: the catalogue under Films and TV Shows, so neither tab is empty.
    catalogue_rails: (a) => [{ key: "most", title: a.shows ? "Most watched shows" : "Most watched films",
      items: [{ id: 7, title: "Sunrise", year: 1927, made_by: "F. W. Murnau", runtime_minutes: 94, note: null }] }],
    add_library_folder: (a) => {
      window.__folders.push({ id: window.__folders.length + 1, path: a.path, files: 0, scanned_at: null, scanning: true });
      return window.__folders.length;
    },
    addon_preview: (a) => ({ pasted: a.url, backend_id: "example-films", name: "Example Films", version: "1.0.0",
      description: "Films from an example source.", finds: "Finds films and episodes.", delivers: ["Streams"],
      host: "a.example", homepage: null, replaces: false }),
    addon_install: (a) => { window.__addon = a.pasted; return "example-films"; },
    sources_overview: () => ({ sources: [{ id: "archive", name: "Internet Archive", blurb: "Public-domain films.",
      addon_host: null, enabled: true, health: "Answering", last_error: null },
      ...(window.__addon ? [{ id: "example-films", name: "Example Films", blurb: "Films from an example source.",
        addon_host: "a.example", enabled: true, health: "Answering", last_error: null }] : [])], refused_addons: [] }),
    torrent_settings: () => ({ download_limit_kib: null, upload_limit_kib: null, peer_limit: 200, keep_seeding: false,
      bind_interface: window.__adapter ?? null, adapters: [{ name: "Ethernet", connected: true },
        { name: "VPN tunnel", connected: true }], engine_running: false }),
    set_torrent_settings: (a) => { window.__adapter = a.bindInterface; return null; },
    set_opensubtitles_key: (a) => { window.__osKey = a.key; return null; },
    set_tmdb_key: (a) => { window.__tmdbKey = a.key; return null; },
    // Phase 12 E5: twenty files waiting, answered by pad alone further down.
    library_review_count: () => window.__review.length,
    library_review: () => window.__review,
    resolve_review: (a) => {
      window.__review = window.__review.filter((i) => i.file_id !== a.fileId);
      return 0;
    },
    // Phase 13: the advisor under Play, Download, and the Downloads page.
    download_advice: () => ({ choice: "download", stall: 0.6, size_bytes: 4.2 * 1024 ** 3,
      sentence: "It would pause to buffer if streamed (1.8 MB/s against the 2.0 MB/s it needs), so download it first, about a minute.",
      download: window.__downloads[0]?.download ?? null }),
    download_title: (a) => {
      const d = { id: 1, media_item_id: a.id, title: "Sunrise", state: "active", backend: "librqbit", priority: 0,
        done_bytes: 1.7 * 1024 ** 3, total_bytes: 4.2 * 1024 ** 3, limit_kib: null, not_before: null,
        reason: a.reason, error: null, file_path: null };
      window.__downloads = [{ download: d, speed: 3.1 * 1024 ** 2, peers: 9 }];
      return d;
    },
    downloads: () => window.__downloads,
    download_pause: () => {
      window.__downloads.forEach((r) => { r.download.state = "paused"; r.speed = 0; });
      return null;
    },
    downloads_storage: () => ({ used_bytes: 0, free_bytes: 200 * 1024 ** 3, budget_bytes: null, items: [] }),
    downloads_settings: () => ({ max_active: 3, budget_gib: null, qbittorrent_address: null,
      qbittorrent_user: null, qbittorrent_has_password: false }),
    // Phase 14: first run (the wizard), then Home's rails.
    onboarding_done: () => window.__onboarded,
    finish_onboarding: () => { window.__onboarded = true; return null; },
    update_profile: () => null,
    rate_title: () => null,
    taste_round: () => Array.from({ length: 12 }, (_, i) => ({ id: 100 + i, title: "Film " + (i + 1),
      year: 1930 + i * 8, made_by: "A Director" })),
    taste_input: () => ({ statements: { pace: null, ambiguity: null, era: null, subtitles: null, runtime: null, form: null }, love: null }),
    taste_preview: () => [],
    // Phase 15: the taste profile, and a correction to it.
    taste_profile: () => ({
      modes: [
        { anchor: 31, share: 0.6, examples: [], correction: window.__corrections[31] ?? null,
          sentence: "You gravitate toward horror, mostly from the 1970s and 80s (Night Fog, Cold Hands, The Pale House)." },
        { anchor: 32, share: 0.4, examples: [], correction: window.__corrections[32] ?? null,
          sentence: "You also love documentary, mostly from the 2010s." },
      ],
      axes: ["You tend to walk out of war."],
    }),
    correct_taste: (a) => { window.__corrections[a.anchor] = a.correction; return null; },
    home: () => [{ key: "loved", title: "Widely loved", items: [{ id: 7, title: "Sunrise", year: 1927,
      made_by: "F. W. Murnau", runtime_minutes: 94 }] },
      // Phase 16 E3: a rail long enough to run off the screen, for the right stick.
      { key: "taste", title: "For you", items: Array.from({ length: 24 }, (_, i) => ({ id: 300 + i,
        title: "Rail film " + (i + 1), year: 1950 + i, made_by: "A Director", runtime_minutes: 100 })) }],
    // Phase 14: two profiles, so Who's watching? opens on launch; Jon's has a PIN.
    profiles: () => window.__profiles,
    switch_profile: (a) => {
      if (a.id === 1 && a.pin !== "2468") throw "that PIN is not right";
      window.__profiles.forEach((p) => { p.active = p.id === a.id; });
      return window.__profiles.find((p) => p.id === a.id);
    },
    // Phase 16: subtitles found and chosen in the player.
    subtitle_offers: () => [
      { file_id: "3", language: "en", hash_match: true, downloads: 5, release: "Sunrise.1927.BluRay" },
      { file_id: "2", language: "en", hash_match: false, downloads: 10, release: "Sunrise.1927.WEB" },
    ],
    subtitle_choose: () => "aligned: offset +1200 ms",
    // Phase 16 E2: the subtitles' delay, which the backend keeps and returns.
    subtitle_nudge: (a) => (window.__subDelay = (window.__subDelay ?? 0) + a.deltaMs),
    set_subtitle_settings: () => null,
    tmdb_status: () => ({ has_key: !!window.__tmdbKey, fetched_on: null }),
    // Phase 16: the catalogue keeps itself current.
    catalogue_status: () => ({ refreshed_at: "2026-11-08T06:21:10Z", titles_added: 885, episodes_added: 7951,
      running: false, ready: true, building: null, percent: 100 }),
    refresh_catalogue: () => true,
    subtitle_settings: () => ({ settings: { languages: ["en"], always: false, font: "Segoe UI", size: 46,
      position: 95, background: false }, has_key: !!window.__osKey, used_today: 0, budget: 5, server_remaining: null }),
  };
  let nextId = 1;
  window.__calls = [];
  window.__downloads = [];
  window.__onboarded = false;
  window.__folders = [];
  window.__corrections = {};
  window.__profiles = [
    { id: 1, name: "Jon", is_default: true, avatar_colour: "slate", is_kids: false, has_pin: true, active: true },
    { id: 2, name: "Mina", is_default: false, avatar_colour: "ochre", is_kids: false, has_pin: false, active: false },
  ];
  const monster = (id, kind, runtime) => ({ media_item_id: id, kind, title: "Monster", year: 2004,
    runtime_minutes: runtime, made_by: null, synopsis: null });
  window.__review = Array.from({ length: 20 }, (_, i) => ({ file_id: i + 1,
    name: "Monster - " + String(i + 1).padStart(2, "0") + " [DVD].mkv", folders: "Anime", minutes: 24, height: 480,
    read_title: "Monster", read_year: null, read_episode: "Episode " + (i + 1), same_name: 0,
    options: [monster(10, "anime_series", 24), monster(11, "series", 45)] }));
  window.__on = {};
  window.__cbs = {};
  window.__emit = (event, payload) => (window.__on[event] ?? []).forEach((h) => window.__cbs[h]?.({ event, id: h, payload }));
  window.__TAURI_INTERNALS__ = {
    invoke: async (cmd, args) => {
      window.__calls.push([cmd, args ?? {}]);
      // An event's handler is kept, so the flow can send what the backend would (__emit).
      if (cmd === "plugin:event|listen") { (window.__on[args.event] ??= []).push(args.handler); return nextId++; }
      const f = answers[cmd];
      return f ? f(args ?? {}) : null;
    },
    transformCallback: (fn) => { const id = nextId++; window.__cbs[id] = fn; return id; },
    unregisterCallback: () => {},
    convertFileSrc: (s) => s,
    metadata: { currentWindow: { label: "main" }, currentWebview: { windowLabel: "main", label: "main" } },
  };
  const buttons = Array.from({ length: 17 }, () => ({ pressed: false, touched: false, value: 0 }));
  const pad = { id: "Xbox Wireless Controller (simulated)", index: 0, connected: true, mapping: "standard",
    timestamp: 0, axes: [0, 0, 0, 0], buttons };
  window.__padSet = (b, down) => { buttons[b] = { pressed: down, touched: down, value: down ? 1 : 0 }; pad.timestamp++; };
  window.__padAxis = (i, v) => { pad.axes[i] = v; pad.timestamp++; };
  navigator.getGamepads = () => [pad, null, null, null];
})();`;

// ── CDP ─────────────────────────────────────────────────────────────────────
let ws;
let msgId = 0;
const pending = new Map();
const send = (method, params = {}) =>
  new Promise((res, rej) => {
    const id = ++msgId;
    pending.set(id, { res, rej });
    ws.send(JSON.stringify({ id, method, params }));
  });
const evalJs = async (expression) => {
  const r = await send("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true });
  if (r.exceptionDetails) throw new Error(r.exceptionDetails.exception?.description ?? "eval threw");
  return r.result.value;
};
const shoot = async (name) => {
  const { data } = await send("Page.captureScreenshot", { format: "png" });
  writeFileSync(`${OUT}/${name}.png`, Buffer.from(data, "base64"));
};

const children = [];
process.on("exit", () => {
  for (const c of children) {
    try {
      if (process.platform === "win32")
        spawnSync("taskkill", ["/pid", String(c.pid), "/T", "/F"], { stdio: "ignore" });
      else c.kill();
    } catch {
      /* already gone */
    }
  }
});

const waitFor = async (probe, what, tries = 90) => {
  for (let i = 0; i < tries; i++) {
    try {
      const v = await probe();
      if (v) return v;
    } catch {
      /* not yet */
    }
    await sleep(200);
  }
  throw new Error(`controller: timed out waiting for ${what}`);
};

mkdirSync(OUT, { recursive: true });

// The whole flow takes about a minute. Past five, something is stuck: fail loudly.
setTimeout(() => {
  console.error("controller: FAILED — still running after 5 minutes");
  process.exit(1);
}, 5 * 60_000).unref();

// The BUILT app (`npm run build` first), served with the fakes inlined as the first
// script — so they exist before any module runs, with no reload. Reloading or
// navigating a page that Chrome has already opened left the module scripts unrun.
const DIST = resolve("dist");
if (!existsSync(resolve(DIST, "index.html"))) {
  console.error("controller: no dist/index.html. Run `npm run build` first.");
  process.exit(1);
}
const TYPES = { ".js": "text/javascript", ".css": "text/css", ".html": "text/html", ".woff2": "font/woff2",
  ".svg": "image/svg+xml", ".png": "image/png", ".json": "application/json", ".ico": "image/x-icon" };
const server = createHttpServer((req, res) => {
  const path = decodeURIComponent(new URL(req.url, "http://x").pathname);
  const file = resolve(DIST, `.${path === "/" ? "/index.html" : path}`);
  if (!file.startsWith(DIST) || !existsSync(file)) {
    res.writeHead(404).end();
    return;
  }
  let body = readFileSync(file);
  if (file.endsWith("index.html")) body = body.toString().replace("<head>", `<head><script>${FAKE}</script>`);
  res.writeHead(200, { "content-type": TYPES[extname(file)] ?? "application/octet-stream" }).end(body);
});
await new Promise((r) => server.listen(PORT, "127.0.0.1", r));
process.on("exit", () => server.close());

children.push(
  spawn(
    CHROME,
    [
      "--headless=new",
      "--disable-gpu",
      "--hide-scrollbars",
      "--no-first-run",
      `--remote-debugging-port=${CDP_PORT}`,
      `--user-data-dir=${resolve(tmpdir(), `sinephile-controller-${process.pid}`)}`,
      "--window-size=1920,1080", // Phase 9 E7: the TV focus language at 1920×1080
      `http://127.0.0.1:${PORT}/`,
    ],
    { stdio: "ignore" },
  ),
);
const target = await waitFor(
  async () => (await (await fetch(`http://127.0.0.1:${CDP_PORT}/json/list`)).json()).find((t) => t.type === "page"),
  "Chrome",
);
ws = new WebSocket(target.webSocketDebuggerUrl);
await new Promise((r) => ws.addEventListener("open", r, { once: true }));
// If Chrome dies, every question waiting on it fails rather than waiting for ever: a
// check that hangs is worse than one that fails (it hung for ten minutes once).
ws.addEventListener("close", () => {
  for (const p of pending.values()) p.rej(new Error("controller: Chrome went away"));
  pending.clear();
});
ws.addEventListener("message", (e) => {
  const m = JSON.parse(e.data);
  const p = m.id && pending.get(m.id);
  if (!p) return;
  pending.delete(m.id);
  if (m.error) p.rej(new Error(m.error.message));
  else p.res(m.result);
});
await send("Page.enable");
await send("Runtime.enable");
// The machine's own setting must not decide the result: Windows Server (the CI runner)
// reports reduced motion, which correctly switched the glide and the grow off. Motion is
// asserted with it off, then again with it on, explicitly.
await send("Emulation.setEmulatedMedia", { features: [{ name: "prefers-reduced-motion", value: "no-preference" }] });
const errors = [];
ws.addEventListener("message", (e) => {
  const m = JSON.parse(e.data);
  if (m.method === "Runtime.exceptionThrown") errors.push(m.params.exceptionDetails.exception?.description);
  if (m.method === "Runtime.consoleAPICalled" && m.params.type === "error")
    errors.push(m.params.args.map((a) => a.value ?? a.description).join(" "));
});
await waitFor(() => evalJs(`!!document.querySelector("header, nav") && !!window.__padSet`), "the app").catch(async (e) => {
  await shoot("failure");
  console.error(errors.join("\n") || "(no page errors)");
  console.error(await evalJs("document.body.innerText.slice(0, 400)"));
  throw e;
});

// ── the pad ─────────────────────────────────────────────────────────────────
const B = { A: 0, B: 1, X: 2, Y: 3, LB: 4, RB: 5, LT: 6, RT: 7, MENU: 9, UP: 12, DOWN: 13, LEFT: 14, RIGHT: 15 };
/**
 * Phase 16 (the author: "glitches and weird ui/gui", layout and spacing, cards and text):
 * what is wrong with the layout on screen now. The page must not scroll sideways; no text
 * may be cut off inside its box; nothing may run off the right edge, except inside
 * something that scrolls on purpose (a rail).
 */
const layoutProblems = () =>
  // The same rules as src/lib/layoutCheck.ts (kept in step by hand: this runs in a plain
  // browser page, the other in the app through the debug probe).
  evalJs(`(() => {
  const out = [];
  if (document.documentElement.scrollWidth > innerWidth + 1) {
    const culprit = [...document.body.querySelectorAll("*")].find(
      (el) => el.getBoundingClientRect().right > innerWidth + 1 && (el.parentElement?.getBoundingClientRect().right ?? 0) <= innerWidth + 1,
    );
    out.push(
      \`the window scrolls sideways by \${document.documentElement.scrollWidth - innerWidth}px: \${
        culprit ? \`\${culprit.tagName.toLowerCase()}.\${String(culprit.className).split(" ").slice(0, 5).join(".")}\` : "unknown"
      }\`,
    );
  }
  const pages = [...document.querySelectorAll('main, [role="dialog"][aria-modal="true"]')];
  for (const pg of pages) {
    if (pg.scrollWidth <= pg.clientWidth + 1) continue;
    // Name the outermost element that crosses the edge: the one to fix.
    const edge = pg.getBoundingClientRect().right + 1;
    const culprit = [...pg.querySelectorAll("*")].find(
      (el) => el.getBoundingClientRect().right > edge && (el.parentElement?.getBoundingClientRect().right ?? 0) <= edge,
    );
    const name = culprit
      ? \`\${culprit.tagName.toLowerCase()}.\${String(culprit.className).split(" ").slice(0, 6).join(".")} (\${Math.round(culprit.getBoundingClientRect().width)}px) "\${(culprit.textContent ?? "").trim().slice(0, 30)}"\`
      : "unknown";
    out.push(\`the page scrolls sideways: \${name}\`);
  }
  const inScroller = (el) => {
    for (let a = el.parentElement; a && !pages.includes(a); a = a.parentElement) {
      const o = getComputedStyle(a).overflowX;
      if (o === "auto" || o === "scroll" || o === "hidden") return true;
    }
    return false;
  };
  for (const el of document.querySelectorAll("h1, h2, h3, p, button, a, label, li span")) {
    const r = el.getBoundingClientRect();
    if (!r.width || !r.height) continue;
    const cs = getComputedStyle(el);
    const text = (el.textContent ?? "").trim().slice(0, 40);
    if (cs.overflow === "hidden" && cs.textOverflow !== "ellipsis" && el.scrollWidth > el.clientWidth + 2) {
      out.push(\`cut off: \${text}\`);
    }
    if (r.right > innerWidth + 1 && cs.position !== "fixed" && !inScroller(el)) out.push(\`off the edge: \${text}\`);
  }
  return [...new Set(out)].slice(0, 12);
})()`);
const SIZES = [[1280, 720], [1600, 900], [1920, 1080]];
/** Check the layout at each size, then put the window back. */
const layoutAtSizes = async (where) => {
  const found = [];
  for (const [width, height] of SIZES) {
    await send("Emulation.setDeviceMetricsOverride", { width, height, deviceScaleFactor: 1, mobile: false });
    await sleep(250);
    for (const p of await layoutProblems()) found.push(`${width}x${height} ${p}`);
  }
  await send("Emulation.clearDeviceMetricsOverride");
  await sleep(200);
  check(found.length === 0, `${where}: no layout problems at 1280, 1600 and 1920 wide${found.length ? " — " + found.join("; ") : ""}`);
};

/** Down for a few frames, then up: one press, as a thumb makes it. */
const press = async (b) => {
  await evalJs(`__padSet(${b}, true)`);
  await sleep(90);
  await evalJs(`__padSet(${b}, false)`);
  await sleep(160);
};
const active = () =>
  evalJs(`(() => { const a = document.activeElement; if (!a || a === document.body) return null;
    const r = a.getBoundingClientRect(); return { text: (a.getAttribute("aria-label") || a.textContent || "").trim(),
    x: r.x + r.width / 2, y: r.y + r.height / 2 }; })()`);
const calls = (cmd) => evalJs(`__calls.filter(c => c[0] === ${JSON.stringify(cmd)}).map(c => c[1])`);

let failures = 0;
const check = (ok, what) => {
  console.log(`${ok ? "  ok  " : "  FAIL"} ${what}`);
  if (!ok) failures++;
};

// ── TV focus language probes (ADR-0042) ─────────────────────────────────────
/** The ring's box, colour and opacity, and the focused element's box. */
const ringState = () =>
  evalJs(`(() => { const r = document.querySelector("[data-focus-ring-el]"); const a = document.activeElement;
    if (!r || !a) return null; const cs = getComputedStyle(r); const rb = r.getBoundingClientRect(); const ab = a.getBoundingClientRect();
    return { colour: cs.borderTopColor, width: cs.borderTopWidth, opacity: Number(cs.opacity),
      ring: { x: rb.left, y: rb.top, w: rb.width, h: rb.height }, el: { x: ab.left, y: ab.top, w: ab.width, h: ab.height } }; })()`);
/** Start sampling the ring's x every frame for `ms`; await the result with `samples()`. */
const startSampling = (ms) =>
  evalJs(`(() => { const r = document.querySelector("[data-focus-ring-el]"); const out = []; const t0 = performance.now();
    window.__ringSamples = new Promise((res) => { const f = () => { out.push(new DOMMatrix(getComputedStyle(r).transform).m41);
      if (performance.now() - t0 < ${ms}) requestAnimationFrame(f); else res(out); }; requestAnimationFrame(f); }); return true; })()`);
const samples = () => evalJs("window.__ringSamples");
/** The computed transform and shadow of a throwaway focused tile: focus.css applies to any tile. */
const tileProbe = () =>
  evalJs(`(async () => { const t = document.createElement("button"); t.dataset.focusTile = ""; t.textContent = "probe";
    t.style.cssText = "position:fixed;left:40%;top:40%;width:120px;height:160px"; document.body.appendChild(t); t.focus();
    await new Promise((r) => setTimeout(r, 350)); const cs = getComputedStyle(t); const out = { transform: cs.transform, shadow: cs.boxShadow };
    t.remove(); return out; })()`);
/** The X scale of a computed `matrix(...)` or `matrix3d(...)`; this runs in Node, which has no DOMMatrix. */
const scaleOf = (transform) => {
  if (transform === "none") return 1;
  const n = transform.match(/-?[\d.]+(?:e-?\d+)?/g).map(Number);
  // In "matrix3d(" the "3" is matched too, so the sixteen values start at n[1].
  return transform.startsWith("matrix3d") ? Math.hypot(n[1], n[2], n[3]) : Math.hypot(n[0], n[1]);
};

/** Steer focus with the D-pad onto the key labelled `label`, using only the pad. */
const steerTo = async (label) => {
  const goal = await evalJs(`(() => { const k = [...document.querySelectorAll('[aria-label="On-screen keyboard"] button')]
    .find(b => b.textContent.trim() === ${JSON.stringify(label)}); if (!k) return null;
    const r = k.getBoundingClientRect(); return { x: r.x + r.width / 2, y: r.y + r.height / 2 }; })()`);
  if (!goal) throw new Error(`no key ${label}`);
  for (let i = 0; i < 24; i++) {
    const a = await active();
    if (a?.text === label) return;
    const dx = goal.x - a.x;
    const dy = goal.y - a.y;
    if (Math.abs(dy) > 8) await press(dy > 0 ? B.DOWN : B.UP);
    else await press(dx > 0 ? B.RIGHT : B.LEFT);
  }
  throw new Error(`the D-pad could not reach ${label}`);
};

/**
 * One press toward a goal, as a thumb would: up or down while it lies above or below, else
 * across. And across when the last vertical press found nothing that way, or came back to
 * somewhere already passed: Down from the right of a row takes the nearest thing below
 * there (the footer), Up comes back, and a goal at the far left is never reached that way.
 */
const towards = async (dx, dy, state) => {
  const vertical = Math.abs(dy) > 8 && !(state.stuck && Math.abs(dx) > 8);
  await evalJs(`void (window.__before = document.activeElement)`);
  const b = vertical ? (dy > 0 ? B.DOWN : B.UP) : (dx > 0 ? B.RIGHT : B.LEFT);
  await press(b);
  state.stuck = vertical && (await evalJs(`(() => { const a = document.activeElement;
    const again = a === window.__before || window.__passed.has(a); window.__passed.add(a); return again; })()`));
  // STEER=1 traces every press, for when a step cannot reach its goal.
  if (process.env.STEER) console.log(`    steer ${Object.keys(B).find((k) => B[k] === b)} (dx ${Math.round(dx)}, dy ${Math.round(dy)})`
    + ` -> ${JSON.stringify(await active())}${state.stuck ? " (stuck)" : ""}`);
};

/** Steer the D-pad to the button labelled `label` (its aria-label or text). */
const steerToButton = async (label) => {
  const state = { stuck: false };
  await evalJs(`void (window.__passed = new Set([document.activeElement]))`);
  for (let i = 0; i < 24; i++) {
    const a = await active();
    if (a?.text === label) return;
    const goal = await evalJs(`(() => { const b = [...document.querySelectorAll('button')].find((x) =>
      (x.getAttribute("aria-label") || x.textContent || "").trim() === ${JSON.stringify(label)}); if (!b) return null;
      const r = b.getBoundingClientRect(); return { x: r.x + r.width / 2, y: r.y + r.height / 2 }; })()`);
    if (!goal || !a) throw new Error(`no button ${label}${a ? "" : " (nothing has focus)"}`);
    await towards(goal.x - a.x, goal.y - a.y, state);
  }
  const where = await evalJs(`(() => { const a = document.activeElement; const r = a.getBoundingClientRect();
    const l = a.id && document.querySelector('label[for="' + a.id + '"]');
    const b = [...document.querySelectorAll('button')].find((x) => (x.getAttribute("aria-label") || x.textContent || "").trim()
      === ${JSON.stringify(label)}); const g = b?.getBoundingClientRect();
    return a.tagName + ' "' + ((l && l.textContent.trim()) || a.textContent.trim()) + '" at ' + Math.round(r.x) + "," + Math.round(r.y)
      + " " + Math.round(r.width) + "x" + Math.round(r.height) + "; the goal at " + (g ? Math.round(g.x) + "," + Math.round(g.y) : "none")
      + " in a window " + innerWidth + "x" + innerHeight; })()`);
  throw new Error(`the D-pad could not reach ${label}; focus stayed on ${where}`);
};

/** A page expression for the form control whose <label> reads `label`. */
const fieldByLabel = (label) => `(() => { const l = [...document.querySelectorAll("label")]
  .find((x) => x.textContent.trim() === ${JSON.stringify(label)}); return l ? document.getElementById(l.htmlFor) : null; })`;

/** Steer the D-pad to whatever the page expression `find` returns. */
const steerToEl = async (find, what) => {
  const state = { stuck: false };
  await evalJs(`void (window.__passed = new Set([document.activeElement]))`);
  for (let i = 0; i < 40; i++) {
    const at = await evalJs(`(() => { const el = ${find}(); if (!el) return null; const a = document.activeElement;
      const r = el.getBoundingClientRect(); const f = a && a !== document.body ? a.getBoundingClientRect() : null;
      return { here: a === el, gx: r.x + r.width / 2, gy: r.y + r.height / 2,
        ax: f ? f.x + f.width / 2 : 0, ay: f ? f.y + f.height / 2 : 0 }; })()`);
    if (!at) throw new Error(`no ${what}`);
    if (at.here) return;
    await towards(at.gx - at.ax, at.gy - at.ay, state);
  }
  const where = await evalJs(`(() => { const a = document.activeElement; const r = a.getBoundingClientRect();
    const g = ${find}()?.getBoundingClientRect(); return a.tagName + ' "' + (a.getAttribute("aria-label") || a.textContent).trim()
      .slice(0, 40) + '" at ' + Math.round(r.x) + "," + Math.round(r.y) + "; the goal at " + (g ? Math.round(g.x) + ","
      + Math.round(g.y) + " " + Math.round(g.width) + "x" + Math.round(g.height) : "none") + " in " + innerWidth + "x" + innerHeight; })()`);
  throw new Error(`the D-pad could not reach ${what}; focus stayed on ${where}`);
};

/** The on-screen keyboard's pages (src/app/keyboardText.ts), and the one showing now. */
const KEY_PAGES = ["abcdefghijklmnopqrstuvwxyz1234567890", "ABCDEFGHIJKLMNOPQRSTUVWXYZ1234567890",
  ".,-_@:/'!?&+#()*=%$\"~;[]{}<>|\\^`"];
const keyPage = async () => {
  // The page key is labelled with the page it leads to.
  const next = await evalJs(`document.querySelector('[aria-label^="Keys: "]')?.getAttribute("aria-label").slice(6)`);
  return (["abc", "ABC", "#+="].indexOf(next) + 2) % 3;
};

/** Type `text` on the open on-screen keyboard as a thumb would: RB for the page, the D-pad, A. */
const typeOnPad = async (text) => {
  for (const ch of text) {
    const want = KEY_PAGES.findIndex((p) => p.includes(ch));
    for (let i = 0; i < 3 && (await keyPage()) !== want; i++) await press(B.RB);
    await steerTo(ch);
    await press(B.A);
  }
};

/** Phase 16 E7: a field filled by pad: the D-pad to it, A for the keyboard, the keys, Menu. */
const fillByPad = async (label, text) => {
  await steerToEl(fieldByLabel(label), `the "${label}" field`);
  await press(B.A);
  await waitFor(() => evalJs(`!!document.querySelector('[aria-label="On-screen keyboard"]')`), "the keyboard", 20);
  await typeOnPad(text);
  await press(B.MENU);
  await waitFor(() => evalJs(`!document.querySelector('[aria-label="On-screen keyboard"]')`), "the keyboard to close", 20);
  return evalJs(`${fieldByLabel(label)}()?.value`);
};

// ── the flow ────────────────────────────────────────────────────────────────
try {
  // Phase 14: first run is the wizard, by pad: a name field first, Continue, a title card
  // picked with A, Continue, then Start watching.
  await waitFor(() => evalJs(`document.body.textContent.includes("Who's this for?")`), "the first-run wizard", 40);
  check(await evalJs(`document.activeElement?.tagName === "INPUT"`), "first run opens on the wizard, focus on the name field");
  await shoot("00a-onboarding");
  await steerToButton("Continue");
  await press(B.A);
  await waitFor(() => evalJs(`document.body.textContent.includes("What do you like?")`), "step 2", 25);
  await waitFor(() => evalJs(`document.body.textContent.includes("Film 1")`), "the title cards", 25);
  await steerToButton("Film 1A Director · 1930");
  await press(B.A);
  check((await calls("rate_title")).some((a) => a.id === 100 && a.stars === 4), "A on a title card picks it (liked, four stars)");
  await shoot("00b-title-cards");
  await steerToButton("Continue");
  await press(B.A);
  // Phase 16 E7: each step filled in by pad alone, and each says it took: a folder typed in,
  // an add-on chosen by its address (checked, then added), both keys checked as they are
  // saved, and downloads bound to the VPN's adapter from its list.
  const PATH = "d:\\films";
  const ADDRESS = "https://a.example/m.json";
  const ADAPTER = fieldByLabel("Use only this network adapter");
  const FILLED_BY_PAD = {
    "Your films on this PC": async () => {
      const typed = await fillByPad("Or type its path", PATH);
      await steerToButton("Add");
      await press(B.A);
      await waitFor(async () => (await calls("add_library_folder")).length === 1, "the folder to be added", 25);
      await waitFor(() => evalJs(`[...document.querySelectorAll("p")].some((p) => p.title === ${JSON.stringify(PATH)})`),
        "the folder in the list", 25);
      check(typed === PATH && (await calls("add_library_folder"))[0].path === PATH,
        `first run, by pad: a library folder typed and added (${typed})`);
    },
    "Where films come from": async () => {
      const typed = await fillByPad("Source address", ADDRESS);
      await steerToButton("Look at it");
      await press(B.A);
      await waitFor(() => evalJs(`!!document.querySelector('[aria-label="About Example Films"]')`), "what the add-on declares", 25);
      await steerToButton("Add source");
      await press(B.A);
      await waitFor(() => evalJs(`document.body.textContent.includes("Example Films was added and is on.")`), "the add-on added", 25);
      check(typed === ADDRESS && (await calls("addon_install"))[0]?.pasted === ADDRESS,
        `first run, by pad: an add-on chosen by its address, looked at, then added (${typed})`);
    },
    Subtitles: async () => {
      await fillByPad("API key", "k3y");
      await steerToButton("Save key");
      await press(B.A);
      await waitFor(() => evalJs(`document.body.textContent.includes("Your key is saved.")`), "the OpenSubtitles key saved", 25);
      check((await calls("set_opensubtitles_key"))[0]?.key === "k3y", "first run, by pad: an OpenSubtitles key typed, checked and saved");
    },
    "Posters and what's on": async () => {
      // ADR-0048: posters are off until chosen; the switch is reached and turned on by pad.
      await steerToEl(`(() => document.querySelector('section[aria-label="Posters"] [role="switch"]'))`, "the posters switch");
      await press(B.A);
      await waitFor(async () => (await calls("set_show_posters")).length === 1, "posters switched on", 25);
      check((await calls("set_show_posters"))[0].on === true, "first run, by pad: posters switched on, and the switch says what it sends");
      await fillByPad("TMDB API key", "t9");
      await steerToButton("Save key");
      await press(B.A);
      await waitFor(() => evalJs(`document.querySelector('section[aria-label="TMDB"]')?.textContent.includes("Your key is saved.")`),
        "the TMDB key saved", 25);
      check((await calls("set_tmdb_key"))[0]?.key === "t9", "first run, by pad: a TMDB key typed, checked and saved");
    },
    Downloads: async () => {
      await steerToEl(ADAPTER, "the network adapter");
      await press(B.A);
      const list = `document.querySelector('[role="dialog"][aria-label="Use only this network adapter"]')`;
      await waitFor(() => evalJs(`!!${list}`), "the adapter list", 20);
      check((await active())?.text.startsWith("Any connection"), "A on a list (the network adapter) opens it by pad, focus on the current choice");
      await steerToButton("VPN tunnel");
      await press(B.A);
      check((await evalJs(`${ADAPTER}().value`)) === "VPN tunnel" && !(await evalJs(`!!${list}`)) &&
        (await evalJs(`document.activeElement === ${ADAPTER}()`)), "A chooses the VPN's adapter; the list closes, focus back on it");
      await steerToEl(`(() => [...document.querySelectorAll('section[aria-label="Network"] button')]
        .find((b) => b.textContent.trim() === "Save"))`, "Network's Save");
      await press(B.A);
      await waitFor(async () => (await calls("set_torrent_settings")).length === 1, "the network settings saved", 25);
      check((await calls("set_torrent_settings"))[0].bindInterface === "VPN tunnel" &&
        (await evalJs(`document.querySelector('section[aria-label="Network"]').textContent.includes("Saved.")`)),
        "first run, by pad: downloads bound to the VPN's adapter, and saved");
    },
  };
  // Phase 16: four steps that leave the viewer ready to watch, each moved through by pad.
  for (const [title, check_] of [
    ["Your films on this PC", "Add a folder"],
    ["Where films come from", "comes with no sources of its own"],
    ["Subtitles", "Get a free key"],
    ["Posters and what's on", "sees which titles you look at"],
    ["Downloads", "choose its network adapter"],
  ]) {
    await waitFor(() => evalJs(`[...document.querySelectorAll("h1")].some((h) => h.textContent === ${JSON.stringify(title)})`), title, 25);
    check(await evalJs(`document.body.textContent.includes(${JSON.stringify(check_)})`), `first run: "${title}" says what it is for`);
    await layoutAtSizes(`first run, "${title}"`);
    await (FILLED_BY_PAD[title] ?? (async () => {}))();
    if (title !== "Downloads") {
      await steerToButton("Continue");
      await press(B.A);
    }
  }
  await steerToButton("Start watching");
  await press(B.A);
  await waitFor(async () => (await calls("finish_onboarding")).length === 1, "the wizard to finish", 25);
  check(true, "the whole first run by pad, and it ends in the app");

  // Phase 14: two profiles, so launch asks who is watching (ADR-0043), focus on the first.
  await waitFor(() => evalJs(`document.body.textContent.includes("Who's watching?")`), "the profile picker", 40);
  await waitFor(async () => (await active())?.text === "Jon, PIN", "focus on the first profile", 25);
  check(true, "with two profiles, launch asks who is watching, and focus is on the first");
  await shoot("00-profiles");
  await press(B.A);
  await waitFor(() => evalJs(`document.body.textContent.includes("Jon's PIN")`), "the PIN pad", 25);
  check((await active())?.text === "1", "a profile with a PIN opens the pad, focus on 1");
  for (const d of ["1", "1", "1", "1"]) {
    await steerToButton(d);
    await press(B.A);
  }
  await waitFor(() => evalJs(`document.body.textContent.includes("that PIN is not right")`), "the wrong PIN", 25);
  check(true, "a wrong PIN says so and changes nothing");
  for (const d of ["2", "4", "6", "8"]) {
    await steerToButton(d);
    await press(B.A);
  }
  await waitFor(() => evalJs(`!document.body.textContent.includes("Who's watching?")`), "the picker to close", 25);
  const switched = await calls("switch_profile");
  check(switched.length === 2 && switched[1].pin === "2468", "the right PIN, typed with the D-pad and A, opens the profile");
  check(await evalJs(`!!document.querySelector('[aria-label="Jon: switch profile"]')`), "the masthead shows who is watching");
  await shoot("01-home");
  await layoutAtSizes("Home");
  // Appendix F, I38, after E7: the first run set everything up by pad, so Home has nothing
  // left to ask for. (A fresh install's card, "0 of 4 done", is src/features/home tests'.)
  check(!(await evalJs(`!!document.querySelector('section[aria-label="Finish setting up"]')`)) &&
    (await calls("library_folders")).length > 0 && (await calls("torrent_settings")).length > 0,
    "after a first run that set everything up by pad, Home has nothing left to set up");
  // Phase 16 E3: the right stick scrolls a long rail sideways, and focus follows into view.
  const inRail = () => evalJs(`!!document.activeElement?.closest('section[aria-label="For you"]')`);
  for (let i = 0; i < 14 && !(await inRail()); i++) await press(B.DOWN);
  check(await inRail(), "the D-pad reaches the long rail");
  const railLeft = () => evalJs(`document.querySelector('section[aria-label="For you"] [data-rail-viewport]').scrollLeft`);
  const leftBefore = await railLeft();
  await evalJs(`__padAxis(2, 0.9)`);
  await sleep(500);
  await evalJs(`__padAxis(2, 0)`);
  await sleep(200);
  const leftAfter = await railLeft();
  check(leftAfter > leftBefore + 300, `the right stick scrolls the rail sideways (${leftBefore} -> ${Math.round(leftAfter)})`);
  check(await evalJs(`(() => { const r = document.activeElement.getBoundingClientRect();
      return !!document.activeElement.closest('section[aria-label="For you"]') && r.right > 0 && r.left < innerWidth; })()`),
    `focus follows along the rail, onto a card in view (${(await active())?.text})`);
  // The control: the check must see a page that runs off the edge, or it measures nothing.
  await evalJs(`(() => { const d = document.createElement("p"); d.id = "too-wide"; d.textContent = "too wide";
    d.style.width = "3000px"; document.querySelector("main").appendChild(d); })()`);
  const seen = await layoutProblems();
  await evalJs(`document.getElementById("too-wide").remove()`);
  check(seen.length > 0, `the layout check sees a 3000 px paragraph (${seen.join("; ")})`);

  await press(B.Y);
  const kb = await waitFor(() => evalJs(`!!document.querySelector('[aria-label="On-screen keyboard"]')`), "the keyboard", 20);
  check(kb, "Y opens search, and the field brings up the on-screen keyboard");
  await shoot("02-keyboard");

  // ADR-0042 in TV mode: the ring, its glide, the couch scale.
  await sleep(300);
  const ring = await ringState();
  check(
    ring?.colour === "rgb(255, 255, 255)" && ring.width === "3px" && ring.opacity === 1,
    `the focus ring is white, 3px and showing (${ring?.colour}, ${ring?.width})`,
  );
  const hug = ring && Math.abs(ring.ring.x - (ring.el.x - 5)) < 2 && Math.abs(ring.ring.w - (ring.el.w + 10)) < 2;
  check(
    !!hug,
    `the ring sits 2px outside the focused key, under the 125% scale (ring x ${ring?.ring.x.toFixed(1)}, key x ${ring?.el.x.toFixed(1)})`,
  );
  const zoom = await evalJs(`getComputedStyle(document.getElementById("root")).zoom`);
  check(zoom === "1.25", `TV mode renders the app at 125% (zoom ${zoom})`);
  // The scaled app must still fill the window, with the safe margin inside it.
  const fit = await evalJs(`(() => { const r = document.getElementById("root").getBoundingClientRect();
    const m = document.querySelector("header, nav").getBoundingClientRect();
    return { w: r.width / innerWidth, h: r.height / innerHeight, left: m.left / innerWidth, top: m.top / innerHeight }; })()`);
  check(
    Math.abs(fit.w - 1) < 0.01 && Math.abs(fit.h - 1) < 0.01 && Math.abs(fit.left - 0.05) < 0.01 && Math.abs(fit.top - 0.05) < 0.01,
    `the scaled app fills the window, with a 5% safe margin (fills ${(fit.w * 100).toFixed(0)}% x ${(fit.h * 100).toFixed(0)}%, masthead at ${(fit.left * 100).toFixed(1)}% / ${(fit.top * 100).toFixed(1)}%)`,
  );
  const fromX = ring?.ring.x ?? 0;
  await startSampling(450);
  await press(B.RIGHT);
  const xs = await samples();
  const toX = xs.at(-1);
  const between = xs.filter((x) => x > fromX + 1 && x < toX - 1).length;
  const overshoot = Math.max(...xs) - toX;
  check(between >= 3 && overshoot < 0.5, `the ring glides to the next key: ${between} frames in between, overshoot ${overshoot.toFixed(2)}px`);
  await press(B.LEFT); // back to "a", where the typing loop expects to start
  const tile = await tileProbe();
  check(
    Math.abs(scaleOf(tile.transform) - 1.06) < 0.01 && tile.shadow !== "none",
    `a focused tile grows to 1.06 and lifts (scale ${scaleOf(tile.transform).toFixed(3)})`,
  );
  await evalJs(`[...document.querySelectorAll('[aria-label="On-screen keyboard"] button')].find((b) => b.textContent.trim() === "a").focus()`);

  for (const ch of "sunrise") {
    await steerTo(ch);
    await press(B.A);
  }
  const typed = await evalJs(`document.querySelector('input[aria-label="Search the catalogue"]').value`);
  check(typed === "sunrise", `typed with the D-pad and A: "${typed}"`);
  await steerTo("Done");
  await press(B.A);
  await waitFor(async () => (await active())?.text.includes("Sunrise"), "focus on the result", 25);
  check(true, "Done closes the keyboard and focus lands on the result");
  await shoot("03-result");

  await press(B.A);
  await waitFor(async () => (await active())?.text === "Play", "Play to take focus", 25);
  check((await calls("prepare_play")).length === 1, "A opens the film; its sources are asked before Play");
  await shoot("04-detail");

  // Phase 9 E7: the ring is visible on EVERY focus stop of the page, not just one.
  const stops = await evalJs(`(async () => {
    const sel = 'a[href], button:not([disabled]), input:not([disabled]), [tabindex]:not([tabindex="-1"])';
    const all = [...document.querySelectorAll(sel)].filter((e) => { const r = e.getBoundingClientRect(); return r.width > 0 && r.height > 0; });
    const misses = [];
    for (const el of all) {
      el.scrollIntoView({ block: "center" }); el.focus();
      await new Promise((r) => setTimeout(r, 350));
      const ring = document.querySelector("[data-focus-ring-el]"); const rb = ring.getBoundingClientRect(); const eb = el.getBoundingClientRect();
      const encloses = rb.left <= eb.left + 1 && rb.top <= eb.top + 1 && rb.right >= eb.right - 1 && rb.bottom >= eb.bottom - 1;
      if (Number(getComputedStyle(ring).opacity) !== 1 || !encloses) misses.push((el.getAttribute("aria-label") || el.textContent || el.tagName).trim().slice(0, 30));
    }
    document.getElementById("detail-play").focus();
    return { count: all.length, misses };
  })()`);
  check(stops.count >= 10 && stops.misses.length === 0, `the ring surrounds every focus stop on the film page (${stops.count} stops; missed: ${stops.misses.join(", ") || "none"})`);
  await sleep(300);

  // Phase 16 (the author): under the wide Play, big round buttons with a word under each.
  await press(B.DOWN);
  check((await active())?.text === "Download", `D-pad down from Play reaches the round Download (${(await active())?.text})`);
  // Appendix F, I39: the film is saved to the watchlist; the circle fills and the word says so.
  await press(B.RIGHT);
  check((await active())?.text === "Watchlist", `Right reaches Watchlist (${(await active())?.text})`);
  await press(B.A);
  await waitFor(async () => (await active())?.text === "On watchlist", "the film to be saved", 25);
  check((await calls("set_watchlisted")).at(-1).on === true &&
    (await evalJs(`document.activeElement.getAttribute("aria-pressed")`)) === "true",
    "A saves the film to the watchlist, and the button says it is on");
  // Appendix F, I21: the film is marked watched by hand, and back.
  await press(B.RIGHT);
  check((await active())?.text === "Mark watched", `Right reaches "Mark watched" (${(await active())?.text})`);
  await press(B.A);
  await waitFor(async () => (await active())?.text === "Watched", "the mark to show", 25);
  const marked = (await calls("set_watched")).at(-1);
  check(marked.watched === true && marked.scope.kind === "title" && marked.scope.id === 7, "A marks the film watched, and the button says so");
  await press(B.A);
  await waitFor(async () => (await active())?.text === "Mark watched", "the mark to clear", 25);
  check((await calls("set_watched")).at(-1).watched === false, "A again marks it as not watched");
  // Streams: every stream, from the right; Phase 9's language choice is at its top.
  await press(B.RIGHT);
  check(((await active())?.text ?? "").startsWith("Streams"), `Right reaches Streams (${(await active())?.text})`);
  await press(B.A);
  await waitFor(() => evalJs(`!!document.querySelector('[role="dialog"][aria-label="Streams"]')`), "the streams panel", 25);
  check(await evalJs(`document.querySelector('[role="dialog"][aria-label="Streams"]').contains(document.activeElement)`),
    "A opens the streams panel, with the D-pad inside it");
  await shoot("04a-streams");
  for (let i = 0; i < 3 && (await active())?.text !== "Auto"; i++) await press(B.DOWN);
  await waitFor(async () => (await active())?.text === "Auto", "the Audio row", 25);
  await press(B.RIGHT);
  await press(B.RIGHT);
  check((await active())?.text === "German", `the D-pad walks the Audio row (${(await active())?.text})`);
  await press(B.A);
  const replanned = await waitFor(async () => (await calls("prepare_play"))[1], "a new plan", 25);
  check(replanned.language === "de" && replanned.quality === null, "A on a language plans again with it");
  check(await evalJs(`!!document.querySelector('[role="dialog"][aria-label="Streams"]')`),
    "the panel stays open while it plans again");
  await shoot("04b-language");
  await press(B.B);
  await waitFor(() => evalJs(`!document.querySelector('[role="dialog"][aria-label="Streams"]')`), "the panel to close", 25);
  check(((await active())?.text ?? "").startsWith("Streams") &&
    (await evalJs(`(document.querySelector("main article h1")?.textContent ?? "").includes("Sunrise")`)),
    "B closes the panel, back on Streams, and the film's page stays");
  // Up to Play.
  for (let i = 0; i < 3 && (await active())?.text !== "Play"; i++) await press(B.UP);
  await waitFor(async () => (await active())?.text === "Play", "Play again", 25);

  await press(B.A);
  const opened = await waitFor(async () => (await calls("player_open"))[0], "the player", 25);
  check(opened.target === "https://films.example/sunrise.mp4", "A on Play starts the player");
  await shoot("05-playing");

  // Phase 16 (ADR-0045's TV-standard mapping): the author found the pad changed nothing
  // during playback. Up raises the volume and says so; Y opens the subtitles with focus in
  // the menu; Down then moves within it rather than lowering the volume; A picks.
  const actions = async (kind) => (await calls("player_action")).filter((a) => a.action?.kind === kind);
  await press(B.UP);
  const louder = await waitFor(async () => (await actions("volume"))[0], "a volume change", 25);
  check(louder.action.percent > 100, `Up raises the volume (${louder.action.percent})`);
  check(
    await evalJs(`[...document.querySelectorAll('[role="status"]')].some((e) => e.textContent.includes("Volume 105%"))`),
    "and the change is said on screen (Volume 105%)",
  );
  check(
    await evalJs(`document.querySelector('[aria-label="Controller buttons"]')?.textContent.includes("Subtitles")`),
    "the player's bar shows the pad's buttons",
  );
  const holes = async () => ((await calls("player_layout")).at(-1)?.cutouts ?? []).length;
  const holesBefore = await holes();
  await press(B.Y);
  await waitFor(() => evalJs(`!!document.querySelector('[role="menu"][aria-label="Subtitles"]')`), "the subtitles menu", 25);
  check((await active())?.text === "Off", "Y opens the subtitles, focus on its first choice");
  await waitFor(async () => (await holes()) === holesBefore + 1, "a hole for the menu", 25);
  check(true, `the menu is cut out of the video: ${holesBefore} -> ${holesBefore + 1} holes`);
  const volumes = (await actions("volume")).length;
  await press(B.DOWN);
  check((await actions("volume")).length === volumes, "Down inside the menu does not change the volume");
  await press(B.UP);
  await press(B.A);
  const picked = await waitFor(async () => (await actions("subtitles"))[0], "a subtitles choice", 25);
  check(picked.action.id === null, "A picks the focused choice (Off)");
  check(await evalJs(`!document.querySelector('[role="menu"]')`), "and the menu closes");

  // Phase 16 E3: X and the bumpers. The backend's report arrives as its event does: a film
  // 30 s in, with two sound tracks and chapters.
  await evalJs(`__emit("player-state", { position: 30, duration: 5640, paused: false, buffered_until: null, volume: 105,
    muted: false, speed: 1, hwdec: null, ended: false, started: true, chapter: 0, dropped_frames: 0,
    tracks: [{ id: 1, kind: "audio", lang: "en", title: "English", selected: true },
             { id: 2, kind: "audio", lang: "de", title: "German", selected: false },
             { id: 3, kind: "sub", lang: "en", title: "English", selected: false }] })`);
  await press(B.X);
  await waitFor(() => evalJs(`!!document.querySelector('[role="menu"][aria-label="Audio"]')`), "the sound menu", 25);
  check((await active())?.text.startsWith("English"), `X opens the sound menu, focus on its first track (${(await active())?.text})`);
  await press(B.DOWN);
  await press(B.A);
  const sound = await waitFor(async () => (await actions("audio"))[0], "a sound track chosen", 25);
  check(sound.action.id === 2, "Down and A choose the second sound track (German)");
  check(await evalJs(`!document.querySelector('[role="menu"]')`), "and the sound menu closes");
  await press(B.RB);
  const forward = await waitFor(async () => (await actions("seek_to")).at(-1), "a chapter jump", 25);
  check(forward.action.seconds === 600, "RB in a film goes to the next chapter (10:00)");
  check(
    await evalJs(`[...document.querySelectorAll('[role="status"]')].some((e) => e.textContent.includes("Chapter: The city"))`),
    "and says which chapter",
  );
  await press(B.LB);
  await waitFor(async () => (await actions("seek_to")).length === 2, "a second chapter jump", 25);
  check((await actions("seek_to")).at(-1).action.seconds === 0, "LB restarts the chapter it is in (0:00)");
  // Left and right seek ten seconds; held, they repeat, which is the scrub.
  const seeksBefore = (await actions("seek_by")).length;
  await press(B.RIGHT);
  const tap = (await actions("seek_by")).slice(seeksBefore);
  check(tap.length === 1 && tap[0].action.seconds === 10, `Right seeks forward 10 s, once for one press (${tap.length})`);
  await evalJs(`__padSet(${B.LEFT}, true)`);
  await sleep(900);
  await evalJs(`__padSet(${B.LEFT}, false)`);
  await sleep(160);
  const held = (await actions("seek_by")).slice(seeksBefore + 1);
  check(held.length >= 3 && held.every((a) => a.action.seconds === -10),
    `Left held for 0.9 s scrubs back: ${held.length} steps of 10 s`);

  // Phase 16 E1: every control in the full panel is reachable by pad. Walk it row by row —
  // right along a row until focus stops moving, then down — marking what focus lands on,
  // and compare with every enabled control the panel holds.
  await press(B.MENU);
  await waitFor(() => evalJs(`!!document.querySelector('[aria-label="Player menu"]')`), "the player's panel", 25);
  const PANEL_CONTROLS = `[...document.querySelectorAll('[aria-label="Player menu"] :is(button, input, select, [tabindex="0"]):not(:disabled)')]`;
  const mark = () => evalJs(`(() => { const a = document.activeElement; if (!a) return null; a.dataset.padSeen ??= String((window.__padSeen = (window.__padSeen ?? 0) + 1)); return a.dataset.padSeen; })()`);
  await mark();
  // Right only while the row has a control further right: on a full-width row, Right goes
  // to the nearest thing up and to the right, which is correct navigation but not a walk.
  const moreOnRow = (sign) => evalJs(`(() => { const a = document.activeElement.getBoundingClientRect();
    return ${PANEL_CONTROLS}.some((e) => { const r = e.getBoundingClientRect();
      return r.top < a.bottom - 1 && r.bottom > a.top + 1 && ${sign} * (r.left + r.width / 2 - (a.left + a.width / 2)) > 1; }); })()`);
  for (let row = 0; row < 40; row++) {
    for (let i = 0; i < 8 && (await moreOnRow(-1)); i++) {
      await press(B.LEFT);
      await mark();
    }
    for (let i = 0; i < 8 && (await moreOnRow(1)); i++) {
      await press(B.RIGHT);
      await mark();
    }
    const before = await mark();
    await press(B.DOWN);
    if ((await mark()) === before) break;
  }
  const unreached = await evalJs(`${PANEL_CONTROLS}.filter((e) => !e.dataset.padSeen).map((e) => e.getAttribute("aria-label") ?? e.textContent.trim())`);
  // Phase 16 E3: the right stick scrolls the panel too. The walk left it at the bottom.
  const panelTop = () => evalJs(`document.querySelector('[aria-label="Player menu"]').scrollTop`);
  const topBefore = await panelTop();
  await evalJs(`__padAxis(3, -0.9)`);
  await sleep(400);
  await evalJs(`__padAxis(3, 0)`);
  await sleep(200);
  const topAfter = await panelTop();
  check(topBefore > 100 && topAfter < topBefore - 100,
    `the right stick scrolls the player panel (${Math.round(topBefore)} -> ${Math.round(topAfter)})`);
  const total = await evalJs(`${PANEL_CONTROLS}.length`);
  check(total > 10 && unreached.length === 0, `the D-pad reaches every control in the player panel (${total - unreached.length}/${total}; missed: ${unreached.join(", ") || "none"})`);
  await press(B.B);
  await waitFor(() => evalJs(`!document.querySelector('[aria-label="Player menu"]')`), "the panel to close", 25);

  // Phase 16: subtitles found and chosen from the full panel, by pad alone.
  await press(B.MENU);
  await waitFor(() => evalJs(`!!document.querySelector('[aria-label="Player menu"]')`), "the player's panel", 25);
  let onFind = false;
  for (let i = 0; i < 20 && !onFind; i++) {
    await press(B.DOWN);
    onFind = ((await active())?.text ?? "").startsWith("Find subtitles");
  }
  check(onFind, "the D-pad reaches 'Find subtitles' in the panel");
  await press(B.A);
  await waitFor(() => evalJs(`document.body.textContent.includes("Sunrise.1927.WEB")`), "the subtitles found", 25);
  let onOffer = false;
  for (let i = 0; i < 4 && !onOffer; i++) {
    await press(B.DOWN);
    onOffer = ((await active())?.text ?? "").includes("Sunrise.1927.BluRay");
  }
  check(onOffer, "the D-pad reaches the first subtitle found");
  await press(B.A);
  const chose = await waitFor(async () => (await calls("subtitle_choose"))[0], "a subtitle chosen", 25);
  check(chose.offer?.file_id === "3", "A downloads and shows the chosen subtitle");
  check(await evalJs(`document.body.textContent.includes("Showing the chosen subtitles")`), "and says so");
  // Phase 16 E2: sync in 0.1 s steps, and the style, each by pad in the panel.
  await steerToButton("Subtitles later");
  await press(B.A);
  await waitFor(async () => (await calls("subtitle_nudge")).length === 1, "the subtitles moved", 25);
  check((await calls("subtitle_nudge"))[0].deltaMs === 100 &&
    (await waitFor(() => evalJs(`document.body.textContent.includes("Subtitles +0.1 s")`), "the nudge said", 25), true),
    "A on 'Subtitles later' moves them 0.1 s, and says so (Subtitles +0.1 s)");
  await steerToButton("Larger subtitles");
  await press(B.A);
  await waitFor(async () => (await calls("set_subtitle_settings")).length > 0, "the style saved", 25);
  const styled = (await calls("set_subtitle_settings")).at(-1);
  check(styled.settings.size === 50, `A on 'Larger subtitles' sets the style for this profile (size 46 -> ${styled.settings.size})`);
  await press(B.B);
  await waitFor(() => evalJs(`!document.querySelector('[aria-label="Player menu"]')`), "the panel to close", 25);

  await press(B.A);
  await waitFor(() => evalJs(`document.body.textContent.includes("Paused")`), "the pause overlay", 25);
  check((await calls("player_pause_still")).length === 1, "A pauses, into a still frame");
  await shoot("06-paused");
  // Phase 16: the right stick reads the paused film's details, focus staying on Resume.
  const about = () => evalJs(`document.querySelector('[aria-label="About this film"]')?.scrollTop ?? -1`);
  await waitFor(async () => (await about()) >= 0, "the film's details", 25);
  await evalJs(`__padAxis(3, 0.9)`);
  await sleep(400);
  await evalJs(`__padAxis(3, 0)`);
  check((await about()) > 100, `the right stick scrolls the paused film's details (${await about()})`);

  await press(B.RT);
  const seeks = await calls("player_action");
  check(
    seeks.some((a) => a.action?.kind === "seek_by" && a.action.seconds === 30),
    "RT seeks forward 30 seconds",
  );

  await press(B.B);
  await waitFor(async () => (await calls("player_close")).length > 0, "the player to close", 25);
  check(await evalJs(`document.body.textContent.includes("Sunrise")`), "B leaves the player, back to the film");
  await shoot("07-left");

  // Phase 13: download from the film page, then pause it on the Downloads page, by pad.
  await waitFor(async () => (await active())?.text === "Play", "Play after the player", 25);
  await press(B.DOWN);
  await waitFor(async () => (await active())?.text === "Download", "Download", 25);
  await press(B.A);
  const asked = await waitFor(async () => (await calls("download_title"))[0], "the download", 25);
  check(asked.id === 7 && asked.tonight === false && asked.candidate === null,
    "A on the round Download queues the film's best stream");
  await waitFor(async () => ((await active())?.text ?? "").startsWith("Downloading"), "the button to say so", 25);
  check(true, `the round button stays focused and says how far it has got (${(await active())?.text})`);
  check(await evalJs(`!!document.querySelector('[aria-label="Downloads, 1 in progress"]')`),
    "the masthead counts the download");
  await press(B.A);
  await waitFor(() => evalJs(`document.body.textContent.includes("from 9 people")`), "the Downloads page", 25);
  check(true, "A on it opens the Downloads page, live: rate and people");
  await shoot("07a-downloads");
  let onPause = false;
  for (let i = 0; i < 12 && !onPause; i++) {
    await press(B.DOWN);
    onPause = (await active())?.text === "Pause";
    if (process.env.STEER) console.log(`    down -> ${JSON.stringify(await active())}`);
  }
  check(onPause, "the D-pad reaches Pause");
  await press(B.A);
  check((await calls("download_pause")).length === 1, "A pauses the download");
  await waitFor(() => evalJs(`document.body.textContent.includes("Paused at 40%")`), "the paused line", 25);
  await press(B.B);
  await waitFor(() => evalJs(`!document.body.textContent.includes("Downloading at once")`), "the page to close", 25);
  check(true, "B leaves the Downloads page");

  // Appendix F, I39: the Watchlist tab is a page now, and holds what was saved.
  const onWatchlist = () => evalJs(`!!document.querySelector('section[aria-label="Watchlist"]')`);
  for (let i = 0; i < 5 && !(await onWatchlist()); i++) await press(B.RB);
  check(await onWatchlist(), "RB reaches the Watchlist tab, and it is a page, not a placeholder");
  check(await evalJs(`document.querySelector('section[aria-label="Watchlist"]').textContent.includes("Sunrise")`),
    "the film saved from its page is on it");
  // The first press lands in the masthead (nothing had focus after the tab changed).
  for (let i = 0; i < 3 && !((await active())?.text ?? "").includes("Sunrise"); i++) await press(B.DOWN);
  check(((await active())?.text ?? "").includes("Sunrise"), `the D-pad reaches the saved film (${(await active())?.text})`);
  await shoot("07e-watchlist");
  await layoutAtSizes("Watchlist");

  // Phase 16 (the author): each tab keeps its page, and the way back says where it goes.
  const titleShown = () => evalJs(`(document.querySelector('main article h1')?.textContent ?? "").includes("Sunrise")`);
  await press(B.A);
  await waitFor(titleShown, "the saved film's page", 25);
  check(await evalJs(`!!document.querySelector('button[aria-label="Back to Watchlist"]')`),
    "its page has a back arrow that says Watchlist");
  await press(B.RB);
  await waitFor(async () => !(await titleShown()), "the next tab", 25);
  await press(B.LB);
  await waitFor(titleShown, "the film's page again", 25);
  check(true, "RB then LB comes back to the film's page: the Watchlist tab kept it");
  await press(B.B);
  await waitFor(onWatchlist, "the Watchlist again", 25);
  check(!(await titleShown()), "B goes back from the film's page to the Watchlist");

  // ADR-0043 (Phase 11 E6): every text field, not just search. Menu opens Settings, the
  // D-pad walks down past the library's path field to the add-on catalogue's address, and
  // the pad types with upper case. (The OpenSubtitles key this used to type into is saved
  // by the first run now, so its field has gone: Phase 16 E7.)
  const FIELD = "Or browse a catalogue";
  const settingsOpen = () => evalJs(`!!document.querySelector('nav[aria-label="Settings sections"]')`);
  // Phase 16 (the author): Settings is a side list of sections, one shown at a time.
  const openSection = async (name, shows) => {
    await steerToButton(name);
    await press(B.A);
    await waitFor(() => evalJs(`document.body.textContent.includes(${JSON.stringify(shows)})`), `the ${name} section`, 25);
    await press(B.RIGHT); // into the section
  };
  await press(B.MENU);
  await waitFor(settingsOpen, "Settings", 25);
  check(await evalJs(`!!document.activeElement?.closest('nav[aria-label="Settings sections"]')`),
    "Menu opens Settings with focus on its side list");
  await openSection("Add-ons", "Add a source");
  check(await evalJs(`!document.body.textContent.includes("OpenSubtitles")`),
    "A on Add-ons in the side list shows that section alone");
  const onKeyField = () => evalJs(`document.activeElement === ${fieldByLabel(FIELD)}()`);
  let onField = false;
  let popped = false;
  for (let i = 0; i < 80 && !onField; i++) {
    await press(B.DOWN);
    popped ||= await evalJs(`!!document.querySelector('[aria-label="On-screen keyboard"]')`);
    onField = await onKeyField();
  }
  check(onField && !popped, "the D-pad walks Settings to a text field below others without a keyboard popping up on the way");
  await press(B.A);
  await waitFor(() => evalJs(`!!document.querySelector('[aria-label="On-screen keyboard"]')`), "the keyboard", 20);
  check(true, "A on the field opens the keyboard");
  await press(B.RB); // abc -> ABC
  await steerTo("A");
  await press(B.A);
  await press(B.LB); // ABC -> abc
  await steerTo("b");
  await press(B.A);
  await steerTo("9");
  await press(B.A);
  await press(B.A); // "99"
  await press(B.X); // X deletes
  const key = await evalJs(`${fieldByLabel(FIELD)}().value`);
  check(key === "Ab9", `typed with case using the bumpers, and X deleted: "${key}"`);
  await shoot("07b-api-key");
  await press(B.MENU); // Menu is Done
  await waitFor(() => evalJs(`!document.querySelector('[aria-label="On-screen keyboard"]')`), "the keyboard to close", 20);
  check(await onKeyField(), "Menu closes the keyboard (Done), and focus stays on the field");

  // Phase 12 E5: the review queue by pad alone. The side list's Library, then down to
  // "Check them"; then each A answers the focused (first) card, twenty times.
  await openSection("Library", "Add a folder");
  let onCheck = false;
  const walked = [];
  for (const dir of [B.DOWN, B.UP]) {
    for (let i = 0; i < 40 && !onCheck; i++) {
      await press(dir);
      const at = (await active())?.text;
      walked.push(at);
      onCheck = at === "Check them";
    }
  }
  if (!onCheck) console.log("  walked up through: " + walked.slice(-12).join(" | "));
  check(onCheck, "the D-pad reaches the review queue's 'Check them' in Settings");
  await press(B.A);
  await waitFor(() => evalJs(`document.body.textContent.includes("Check your library")`), "the review queue", 25);
  await shoot("07c-review");
  const t0 = Date.now();
  for (let left = 20; left > 0; left--) {
    await waitFor(
      () => evalJs(`document.activeElement?.textContent?.includes("Monster") && window.__review.length === ${left}`),
      `a card focused with ${left} waiting`,
      40,
    );
    await press(B.A);
  }
  await waitFor(() => evalJs(`document.body.textContent.includes("Every file in your library is identified")`), "an empty queue", 40);
  const seconds = (Date.now() - t0) / 1000;
  const answered = (await calls("resolve_review")).length;
  check(answered === 20 && seconds < 60, `20 answers by A alone in ${seconds.toFixed(1)} s (E5: under a minute)`);
  await press(B.B);
  await waitFor(settingsOpen, "Settings again", 25);
  check(true, "B returns from the queue to Settings");

  // Phase 15: the taste profile by pad. The D-pad finds "See Jon's taste" in Settings,
  // A opens it, and a mode is corrected with A alone.
  if (!(await evalJs(`!!document.activeElement && document.activeElement !== document.body`))) await press(B.LEFT);
  await openSection("Profile", "See Jon's taste");
  let onTaste = false;
  for (let i = 0; i < 80 && !onTaste; i++) {
    await press(B.DOWN);
    onTaste = (await active())?.text === "See Jon's taste";
  }
  for (let i = 0; i < 120 && !onTaste; i++) {
    await press(B.UP);
    onTaste = (await active())?.text === "See Jon's taste";
  }
  check(onTaste, "the D-pad reaches 'See Jon's taste' in Settings");
  await press(B.A);
  await waitFor(() => evalJs(`document.body.textContent.includes("You gravitate toward horror")`), "the taste profile", 25);
  check(await evalJs(`document.body.textContent.includes("60% of your taste")`), "A opens the taste profile: each mode said, with its share");
  await shoot("07d-taste");
  const inFirst = () =>
    evalJs(`document.activeElement?.closest('[role="radiogroup"]')?.getAttribute("aria-label") === "Taste 1"`);
  for (let i = 0; i < 8 && !(await inFirst()); i++) await press(B.DOWN);
  let onLess = false;
  for (let i = 0; i < 4 && !onLess; i++) {
    onLess = (await inFirst()) && (await active())?.text === "Less";
    if (!onLess) await press(B.RIGHT);
  }
  check(onLess, "the D-pad reaches the first mode's 'Less'");
  await press(B.A);
  const fixed = await waitFor(async () => (await calls("correct_taste"))[0], "the correction", 25);
  check(fixed.anchor === 31 && fixed.correction === "less", "A on Less corrects the first mode");
  await press(B.B);
  await waitFor(settingsOpen, "Settings after the taste", 25);
  check(true, "B returns from the taste profile to Settings");
  // The longest section, for the right stick.
  if (!(await evalJs(`!!document.activeElement && document.activeElement !== document.body`))) await press(B.LEFT);
  await openSection("Add-ons", "Add a source");

  // Phase 16: the right stick scrolls the page under focus, and focus follows onto what is
  // in view, so the next D-pad press starts from where the eye is.
  const scrolled = () => evalJs(`document.querySelector("main").scrollTop`);
  const top = await scrolled();
  await evalJs(`__padAxis(3, 0.9)`);
  await sleep(600);
  await evalJs(`__padAxis(3, 0)`);
  await sleep(200);
  const after = await scrolled();
  check(after > top + 200, `the right stick scrolls Settings down (${top} -> ${after})`);
  check(
    await evalJs(`(() => { const r = document.activeElement?.getBoundingClientRect(); return !!r && r.bottom > 0 && r.top < innerHeight; })()`),
    "focus follows onto something in view",
  );
  await press(B.B); // leave Settings
  // Phase 16: a page comes back where it was left, in real layout (Settings is long).
  await press(B.MENU);
  await waitFor(settingsOpen, "Settings again", 25);
  await sleep(150);
  const again = await scrolled();
  check(Math.abs(again - after) < 4, `Settings opens again where it was left (${Math.round(after)} -> ${Math.round(again)})`);
  await press(B.B);

  // Reduced motion: every transform goes, the ring stops gliding.
  await send("Emulation.setEmulatedMedia", { features: [{ name: "prefers-reduced-motion", value: "reduce" }] });
  const still = await tileProbe();
  check(
    still.transform === "none" && still.shadow === "none",
    `under reduced motion a focused tile has no transform or shadow (${still.transform})`,
  );
  await press(B.Y);
  await waitFor(() => evalJs(`!!document.querySelector('[aria-label="On-screen keyboard"]')`), "the keyboard again", 20);
  await sleep(250);
  const before = (await ringState())?.ring.x ?? 0;
  await startSampling(300);
  await press(B.RIGHT);
  const snap = await samples();
  const mid = snap.filter((x) => x > before + 1 && x < snap.at(-1) - 1).length;
  check(mid === 0, `under reduced motion the ring moves without gliding (${mid} frames in between)`);
} catch (e) {
  console.error(`controller: ${e.message}`);
  await shoot("failure").catch(() => {});
  failures++;
}

console.log(failures === 0 ? "controller: the whole flow works with only a pad" : `controller: ${failures} failed`);
process.exit(failures === 0 ? 0 : 1);
