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
      from: "Internet Archive", offered: 2 }),
    player_pause_still: () => "data:image/gif;base64,R0lGODlhAQABAAAAACw=",
    dev_autoplay: () => null,
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
      "--window-size=1440,900",
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
const B = { A: 0, B: 1, X: 2, Y: 3, LB: 4, RB: 5, LT: 6, RT: 7, UP: 12, DOWN: 13, LEFT: 14, RIGHT: 15 };
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

  for (const ch of "SUNRISE") {
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
} catch (e) {
  console.error(`controller: ${e.message}`);
  await shoot("failure").catch(() => {});
  failures++;
}

console.log(failures === 0 ? "controller: the whole flow works with only a pad" : `controller: ${failures} failed`);
process.exit(failures === 0 ? 0 : 1);
