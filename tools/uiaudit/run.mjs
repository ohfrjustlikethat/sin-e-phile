/**
 * tools/uiaudit — the things a screenshot cannot check.
 *
 * Phase 2's exit criteria include "60fps with 500 cards", "fully keyboard-
 * navigable with visible focus at every step", and "every component respects
 * prefers-reduced-motion". None of those can be signed off by looking at a
 * picture, and SPEC.md §10.8 does not accept "looks good" as evidence. So this
 * drives the real design gallery in headless Chrome over the DevTools Protocol
 * and measures them.
 *
 * Every assertion here exists because it caught a real bug the day it was written:
 *   - the rail's virtualisation was silently doing nothing. A grid item defaults
 *     to `min-width: auto`, so the 97,060px track expanded its column, the
 *     ResizeObserver measured a viewport wider than the content, and all 500
 *     cards mounted. Nothing looked wrong.
 *   - the rail rendered at zero height, because absolutely-positioned children
 *     contribute none.
 *   - Tab reached only the ~13 mounted cards and then left the rail entirely, so
 *     487 of 500 cards were unreachable by keyboard.
 *   - `prefers-reduced-motion` ADDED motion: setting `transition-duration` on `*`
 *     without pinning `transition-property` makes every property animate.
 *
 * Zero dependencies on purpose — ADR-0012's reasoning applied to JS. Node 24 has
 * a built-in WebSocket client, so there is no Puppeteer to pin or keep current.
 *
 * Usage:  node tools/uiaudit/run.mjs [--ci] [--out <dir>]
 */
import { spawn, spawnSync } from "node:child_process";
import { createServer } from "node:net";
import { tmpdir } from "node:os";
import { existsSync, mkdirSync, openSync, readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { setTimeout as sleep } from "node:timers/promises";

const ARGS = process.argv.slice(2);
const CI = ARGS.includes("--ci");
const OUT = ARGS.includes("--out") ? ARGS[ARGS.indexOf("--out") + 1] : "tools/uiaudit/out";
/** A free port, chosen now. Fixed ports made a leftover Chrome or Vite from a
 *  previous run silently serve the OLD code: the harness attached to the stale
 *  process, measured the wrong build, and reported a timeout with no clue why.
 *  Asking the OS for a free port removes the whole class of problem, and leaves
 *  any dev server you are already running untouched. */
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
const URL = `http://localhost:${PORT}/#design`;

/** Budgets. Breaking one of these is a regression, not a matter of taste. */
const BUDGET = {
  worstFrameMs: 34, // one dropped frame at 60fps; 16.7ms is the target cadence
  droppedFrames: 0, // frames over 33.3ms — a doubled frame is visible
  maxMountedCards: 60, // of 500. If this blows out, virtualisation is doing nothing.
  focusStopsWithoutRing: 0,
  nonOpacityTransitionsUnderReduce: 0,
};

const CHROME = [
  process.env["CHROME_PATH"],
  "C:/Program Files/Google/Chrome/Application/chrome.exe",
  "C:/Program Files (x86)/Google/Chrome/Application/chrome.exe",
  "/usr/bin/google-chrome",
].find((p) => p && existsSync(p));

if (!CHROME) {
  console.error("uiaudit: no Chrome found. Set CHROME_PATH to a Chrome executable.");
  process.exit(1);
}

// ── CDP plumbing ────────────────────────────────────────────────────────────
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
  const r = await send("Runtime.evaluate", {
    expression,
    awaitPromise: true,
    returnByValue: true,
  });
  if (r.exceptionDetails) throw new Error(r.exceptionDetails.exception?.description ?? "eval threw");
  return r.result.value;
};

const clickTab = async (text) => {
  await evalJs(
    `[...document.querySelectorAll('[role="tab"]')]` +
      `.find(b => b.textContent.toLowerCase().includes(${JSON.stringify(text.toLowerCase())})).click()`,
  );
  await sleep(900);
};

const press = async (key, vk) => {
  for (const type of ["rawKeyDown", "keyUp"])
    await send("Input.dispatchKeyEvent", { type, key, code: key, windowsVirtualKeyCode: vk });
  await sleep(40);
};

const shoot = async (name) => {
  const { data } = await send("Page.captureScreenshot", { format: "png" });
  writeFileSync(`${OUT}/${name}.png`, Buffer.from(data, "base64"));
};

// ── launch ──────────────────────────────────────────────────────────────────
const children = [];
process.on("exit", () => {
  for (const c of children) {
    try {
      // `npm run dev` is a shell -> npm -> node tree, and killing the shell on
      // Windows orphans the server that is actually holding the port. taskkill /T
      // takes the whole tree; kill() is the fallback everywhere else.
      if (process.platform === "win32")
        spawnSync("taskkill", ["/pid", String(c.pid), "/T", "/F"], { stdio: "ignore" });
      else c.kill();
    } catch {
      /* already gone */
    }
  }
});

mkdirSync(OUT, { recursive: true });

// One command string, not shell:true plus an args array — Node deprecated the
// latter (DEP0190) because the args are concatenated rather than escaped.
// The dev server's output goes to a file, never to /dev/null. When it died after
// serving one request the harness had nothing to report but a timeout.
const viteLog = `${OUT}/vite.log`;
const viteOut = openSync(viteLog, "w");
children.push(
  spawn(`npm run dev -- --port ${PORT} --strictPort`, {
    shell: true,
    stdio: ["ignore", viteOut, viteOut],
  }),
);

/** Poll rather than sleep a guessed amount — CI runners are slower than laptops. */
const waitFor = async (probe, what, tries = 90) => {
  for (let i = 0; i < tries; i++) {
    try {
      const v = await probe();
      if (v) return v;
    } catch {
      /* not up yet */
    }
    await sleep(500);
  }
  throw new Error(`uiaudit: timed out waiting for ${what}`);
};

await waitFor(() => fetch(`http://localhost:${PORT}/`).then((r) => r.ok), "the dev server");

// Chrome opens ON the gallery rather than on about:blank followed by
// Page.navigate. Navigating an about:blank tab left the document loaded but the
// module scripts never executing — body empty, no exception, no clue. Starting at
// the real URL is both simpler and what actually works.
children.push(
  spawn(
    CHROME,
    [
      "--headless=new",
      "--disable-gpu",
      "--hide-scrollbars",
      "--no-first-run",
      `--remote-debugging-port=${CDP_PORT}`,
      // The profile lives in the OS temp dir, NOT under the project. Vite watches
      // the project tree, and Chrome keeps `Default/Network/Cookies` locked —
      // the watcher hit EBUSY and Vite exited, mid-run, after serving exactly one
      // request. The symptom was an empty page with no error at all. Absolute
      // path because Chrome on Windows refuses to start on a relative one.
      `--user-data-dir=${resolve(tmpdir(), `sinephile-uiaudit-${process.pid}`)}`,
      "--window-size=1500,1000",
      URL,
    ],
    { stdio: "ignore" },
  ),
);

const target = await waitFor(
  async () =>
    (await (await fetch(`http://127.0.0.1:${CDP_PORT}/json/list`)).json()).find(
      (t) => t.type === "page",
    ),
  "Chrome",
);

ws = new WebSocket(target.webSocketDebuggerUrl);
await new Promise((r) => (ws.onopen = r));
ws.onmessage = (e) => {
  const m = JSON.parse(e.data);
  if (!m.id || !pending.has(m.id)) return;
  const { res, rej } = pending.get(m.id);
  pending.delete(m.id);
  if (m.error) rej(new Error(JSON.stringify(m.error)));
  else res(m.result);
};

await send("Page.enable");
await send("Runtime.enable");
await send("Log.enable");

// A silent failure must never pass as a green run — and it must never be
// undiagnosable either. `Runtime.exceptionThrown` alone is not enough: a module
// script that fails to FETCH never throws, so the page simply stays empty. That
// happened, and cost an hour. `Log.entryAdded` carries those network errors.
const pageErrors = [];
const pageLog = [];
ws.addEventListener("message", (e) => {
  const m = JSON.parse(e.data);
  if (m.method === "Runtime.exceptionThrown")
    pageErrors.push(m.params.exceptionDetails.exception?.description ?? "unknown");
  if (m.method === "Log.entryAdded") {
    const { level, text, url } = m.params.entry;
    pageLog.push(`${level}: ${text}${url ? ` (${url})` : ""}`);
    // Only assets the DESIGN depends on are failures. A browser complaining
    // about some incidental request is noise; a missing still, font, style or
    // module means the thing being audited is not the thing that shipped.
    const critical = /\/(src|stills|fonts|assets)\/|\.css|\.woff2|\.tsx?$/.test(url ?? "");
    if (level === "error" && critical) pageErrors.push(`${text} — ${url}`);
  }
});

// Poll for the gallery rather than sleeping a guessed amount. Vite's first
// request triggers dependency optimisation and a full transform pass, which on a
// cold cache takes far longer than any sleep worth hard-coding.
await waitFor(
  async () => (await evalJs(`document.querySelectorAll('[role="tab"]').length`)) >= 4,
  "the design gallery to render",
).catch(async (e) => {
  // Say what the page actually contained, rather than only that it timed out.
  const dump = await evalJs(`({
    url: location.href,
    title: document.title,
    bodyChars: document.body ? document.body.innerText.length : -1,
    head: document.body ? document.body.innerText.slice(0, 400) : '(no body)',
    tabs: document.querySelectorAll('[role="tab"]').length,
  })`).catch((x) => ({ evalFailed: String(x) }));
  console.error("uiaudit: page state at timeout:", JSON.stringify(dump, null, 2));
  console.error("page errors:", pageErrors);
  console.error("browser log:", pageLog.slice(0, 12));
  console.error(`dev server log (${viteLog}):`, readFileSync(viteLog, "utf8").slice(-1200));
  throw e;
});

const results = { url: URL, when: new Date().toISOString() };

// ── 2.5: frame timing while flicking the 500-card rail ──────────────────────
await clickTab("500");
results.rail = await evalJs(`
  (async () => {
    const rails = [...document.querySelectorAll('[data-rail-viewport]')];
    if (!rails.length) return { error: 'no [data-rail-viewport] in the DOM' };
    const rail = rails.reduce((a, b) => (b.scrollWidth > a.scrollWidth ? b : a));
    const max = rail.scrollWidth - rail.clientWidth;
    if (max < 200)
      return { error: 'rail is not scrollable — its track is not width-constrained',
               scrollWidth: rail.scrollWidth, clientWidth: rail.clientWidth };

    rail.scrollLeft = 0;
    await new Promise(r => requestAnimationFrame(r));

    const deltas = [];
    let last = performance.now(), x = 0;
    await new Promise((done) => {
      function step(now) {
        deltas.push(now - last); last = now;
        x += 55;                      // a fast flick: ~3,300px per second
        if (x >= max) x = 0;          // wrap, so the sample is always full length
        rail.scrollLeft = x;
        if (deltas.length < 200) requestAnimationFrame(step); else done();
      }
      requestAnimationFrame(step);
    });

    const d = deltas.slice(5).sort((a, b) => a - b);    // drop warm-up frames
    const at = p => d[Math.min(d.length - 1, Math.floor(d.length * p))];
    return {
      framesSampled: d.length,
      mountedCards: rail.querySelectorAll('[data-rail-index]').length,
      totalCards: 500,
      medianMs: +at(0.5).toFixed(2),
      p95Ms: +at(0.95).toFixed(2),
      worstMs: +d[d.length - 1].toFixed(2),
      droppedFrames: d.filter(v => v > 33.3).length,
    };
  })()`);

// ── 2.6a: the roving tabindex must reach the far end of all 500 ─────────────
await evalJs(`
  (async () => {
    const rail = [...document.querySelectorAll('[data-rail-viewport]')]
      .reduce((a, b) => (b.scrollWidth > a.scrollWidth ? b : a));
    rail.scrollLeft = 0;
    rail.dispatchEvent(new Event('scroll'));
    await new Promise(r => setTimeout(r, 350));
    rail.querySelector('[data-rail-index="0"] button')?.focus();
  })()`);

const railState = () =>
  evalJs(`
  (() => {
    const w = document.activeElement && document.activeElement.closest('[data-rail-index]');
    const rail = [...document.querySelectorAll('[data-rail-viewport]')]
      .reduce((a, b) => (b.scrollWidth > a.scrollWidth ? b : a));
    return {
      index: w ? Number(w.dataset.railIndex) : null,
      tabStops: [...rail.querySelectorAll('[data-rail-index] button')].filter(b => b.tabIndex === 0).length,
    };
  })()`);

const kb = { start: await railState() };
for (let i = 0; i < 25; i++) await press("ArrowRight", 39);
kb.afterArrows = await railState();
await press("End", 35);
kb.afterEnd = await railState();
results.railKeyboard = kb;

// ── 2.6b: every focus stop must show a ring and stay on screen ──────────────
await clickTab("primitives");
await evalJs(`document.querySelector('[role="tab"]').focus()`);

const walk = [];
for (let i = 0; i < 45; i++) {
  await press("Tab", 9);
  walk.push(
    await evalJs(`
    (() => {
      const a = document.activeElement;
      if (!a || a === document.body) return { lost: true };
      const cs = getComputedStyle(a);
      const outline = (parseFloat(cs.outlineWidth) || 0) > 0 && cs.outlineStyle !== 'none';
      const r = a.getBoundingClientRect();
      return {
        name: (a.getAttribute('aria-label') || a.textContent || a.tagName).trim().slice(0, 40),
        visibleRing: outline || (cs.boxShadow !== 'none' && cs.boxShadow !== ''),
        onScreen: r.width > 0 && r.height > 0,
      };
    })()`),
  );
}
results.focus = {
  stops: walk.length,
  distinct: new Set(walk.filter((s) => !s.lost).map((s) => s.name)).size,
  withoutRing: walk.filter((s) => !s.lost && !s.visibleRing).map((s) => s.name),
  offScreen: walk.filter((s) => !s.lost && !s.onScreen).map((s) => s.name),
};
await shoot("primitives");

// ── 2.7: under reduce, only opacity may transition and nothing may animate ──
const readMotion = () =>
  evalJs(`
  (() => {
    const ms = (v) => parseFloat(v) * (v.trim().endsWith('ms') ? 1 : 1000) || 0;
    const offenders = [];
    let animating = 0;
    for (const el of document.querySelectorAll('body *')) {
      const cs = getComputedStyle(el);
      const props = cs.transitionProperty.split(',').map(v => v.trim())
        .filter(p => p && p !== 'none' && p !== 'opacity');
      const dur = Math.max(0, ...cs.transitionDuration.split(',').map(ms));
      if (cs.animationName !== 'none' && Math.max(0, ...cs.animationDuration.split(',').map(ms)) > 1)
        animating++;
      if (props.length && dur > 1)
        offenders.push({ tag: el.tagName, props: props.slice(0, 3).join('|'), ms: Math.round(dur) });
    }
    return { nonOpacityTransitions: offenders.length, runningAnimations: animating, sample: offenders.slice(0, 5) };
  })()`);

const setMotion = (value) =>
  send("Emulation.setEmulatedMedia", {
    features: [{ name: "prefers-reduced-motion", value }],
  });

await setMotion("no-preference");
await sleep(400);
const motionNormal = await readMotion();
await setMotion("reduce");
await sleep(400);
const motionReduced = await readMotion();
await setMotion("no-preference");
results.reducedMotion = { normal: motionNormal, reduced: motionReduced };
results.pageErrors = pageErrors;

// ── verdict ─────────────────────────────────────────────────────────────────
const fail = [];
const r = results.rail;

if (r.error) {
  fail.push(`rail: ${r.error}`);
} else {
  if (r.worstMs > BUDGET.worstFrameMs)
    fail.push(`worst frame ${r.worstMs}ms exceeds ${BUDGET.worstFrameMs}ms`);
  if (r.droppedFrames > BUDGET.droppedFrames) fail.push(`${r.droppedFrames} dropped frames`);
  if (r.mountedCards > BUDGET.maxMountedCards)
    fail.push(`${r.mountedCards}/500 cards mounted — virtualisation is not working`);
}
if (results.railKeyboard.afterEnd.index !== 499)
  fail.push(`End reached card ${results.railKeyboard.afterEnd.index}, not 499 — rail is not keyboard-complete`);
if (results.railKeyboard.afterArrows.tabStops !== 1)
  fail.push(`rail exposes ${results.railKeyboard.afterArrows.tabStops} tab stops, expected exactly 1`);
if (results.focus.withoutRing.length > BUDGET.focusStopsWithoutRing)
  fail.push(`no visible focus ring on: ${results.focus.withoutRing.join(", ")}`);
if (results.focus.offScreen.length)
  fail.push(`focus moved off-screen: ${results.focus.offScreen.join(", ")}`);
if (motionReduced.nonOpacityTransitions > BUDGET.nonOpacityTransitionsUnderReduce)
  fail.push(
    `${motionReduced.nonOpacityTransitions} non-opacity transitions survive prefers-reduced-motion`,
  );
if (motionReduced.runningAnimations)
  fail.push(`${motionReduced.runningAnimations} animations run under prefers-reduced-motion`);
if (pageErrors.length) fail.push(`the page threw: ${pageErrors[0]}`);

writeFileSync(`${OUT}/results.json`, JSON.stringify(results, null, 2));

console.log(
  [
    "",
    `  rail       ${r.error ?? `${r.mountedCards}/500 mounted · median ${r.medianMs}ms · p95 ${r.p95Ms}ms · worst ${r.worstMs}ms · ${r.droppedFrames} dropped`}`,
    `  keyboard   rail is ${results.railKeyboard.afterArrows.tabStops} tab stop; End reaches card ${results.railKeyboard.afterEnd.index} of 500`,
    `  focus      ${results.focus.stops} stops, ${results.focus.distinct} distinct, ${results.focus.withoutRing.length} without a ring`,
    `  motion     ${motionNormal.nonOpacityTransitions} non-opacity transitions normally → ${motionReduced.nonOpacityTransitions} under reduce`,
    `  artefacts  ${OUT}/results.json`,
    "",
  ].join("\n"),
);

if (fail.length) {
  console.error("uiaudit FAILED:\n" + fail.map((f) => `  - ${f}`).join("\n"));
  process.exit(1);
}
console.log("uiaudit: all budgets met.");
process.exit(CI && pageErrors.length ? 1 : 0);
