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
    optional_assets: () => ({ missing: [], total_bytes: 0 }),
    item_detail: () => ({ id: 7, title: "Sunrise", year: 1927, kind: "film", runtime_minutes: 94,
      synopsis: "A married farmer falls under the spell of a woman from the city.",
      credits: [{ name: "F. W. Murnau", role: "director", character: null }] }),
    prepare_play: () => ({ title: "Sunrise", open: "https://films.example/sunrise.mp4", start: null,
      from: "Internet Archive", offered: 2, languages: ["en", "de"], original_language: null,
      qualities: ["full_hd", "hd"], unmet: null, why: "1080p · served directly", candidates: [] }),
    player_pause_still: () => "data:image/gif;base64,R0lGODlhAQABAAAAACw=",
    dev_autoplay: () => null,
    subtitle_settings: () => ({ settings: { languages: ["en"], always: false, font: "Segoe UI", size: 46,
      position: 95, background: false }, has_key: false, used_today: 0, budget: 5, server_remaining: null }),
  };
  let nextId = 1;
  window.__calls = [];
  window.__TAURI_INTERNALS__ = {
    invoke: async (cmd, args) => {
      window.__calls.push([cmd, args ?? {}]);
      if (cmd === "plugin:event|listen") return nextId++;
      const f = answers[cmd];
      return f ? f(args ?? {}) : null;
    },
    transformCallback: () => nextId++,
    unregisterCallback: () => {},
    convertFileSrc: (s) => s,
    metadata: { currentWindow: { label: "main" }, currentWebview: { windowLabel: "main", label: "main" } },
  };
  const buttons = Array.from({ length: 17 }, () => ({ pressed: false, touched: false, value: 0 }));
  const pad = { id: "Xbox Wireless Controller (simulated)", index: 0, connected: true, mapping: "standard",
    timestamp: 0, axes: [0, 0, 0, 0], buttons };
  window.__padSet = (b, down) => { buttons[b] = { pressed: down, touched: down, value: down ? 1 : 0 }; pad.timestamp++; };
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

// ── the flow ────────────────────────────────────────────────────────────────
try {
  await shoot("01-home");

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

  // Phase 9: the language choice is reachable with the pad, and choosing re-plans.
  await press(B.DOWN);
  await waitFor(async () => (await active())?.text === "Auto", "the Audio row", 25);
  await press(B.RIGHT);
  await press(B.RIGHT);
  check((await active())?.text === "German", `the D-pad walks the Audio row (${(await active())?.text})`);
  await press(B.A);
  const replanned = await waitFor(async () => (await calls("prepare_play"))[1], "a new plan", 25);
  check(replanned.language === "de" && replanned.quality === null, "A on a language plans again with it");
  await shoot("04b-language");
  await press(B.UP);
  await waitFor(async () => (await active())?.text === "Play", "Play again", 25);

  await press(B.A);
  const opened = await waitFor(async () => (await calls("player_open"))[0], "the player", 25);
  check(opened.target === "https://films.example/sunrise.mp4", "A on Play starts the player");
  await shoot("05-playing");

  await press(B.A);
  await waitFor(() => evalJs(`document.body.textContent.includes("Paused")`), "the pause overlay", 25);
  check((await calls("player_pause_still")).length === 1, "A pauses, into a still frame");
  await shoot("06-paused");

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

  // ADR-0043 (Phase 11 E6): every text field, not just search. Menu opens Settings, the
  // D-pad walks down to the OpenSubtitles key, and the pad types a key with upper case.
  await press(B.MENU);
  await waitFor(() => evalJs(`document.body.textContent.includes("OpenSubtitles")`), "Settings", 25);
  const onKeyField = () =>
    evalJs(`(() => { const l = [...document.querySelectorAll("label")].find((x) => x.textContent.trim() === "API key");
      return !!l && document.activeElement === document.getElementById(l.htmlFor); })()`);
  let onField = false;
  let popped = false;
  for (let i = 0; i < 80 && !onField; i++) {
    await press(B.DOWN);
    popped ||= await evalJs(`!!document.querySelector('[aria-label="On-screen keyboard"]')`);
    onField = await onKeyField();
  }
  check(onField && !popped, "the D-pad walks Settings to the API key field without a keyboard popping up on the way");
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
  const key = await evalJs(`(() => { const l = [...document.querySelectorAll("label")].find((x) => x.textContent.trim() === "API key");
    return document.getElementById(l.htmlFor).value; })()`);
  check(key === "Ab9", `typed a key with case using the bumpers, and X deleted: "${key}"`);
  await shoot("07b-api-key");
  await press(B.MENU); // Menu is Done
  await waitFor(() => evalJs(`!document.querySelector('[aria-label="On-screen keyboard"]')`), "the keyboard to close", 20);
  check(await onKeyField(), "Menu closes the keyboard (Done), and focus stays on the field");
  await press(B.B); // leave Settings

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
