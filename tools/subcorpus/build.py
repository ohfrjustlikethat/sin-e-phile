"""Build fixtures/subtitles/: the subtitle-alignment corpus (SPEC.md §12.2, Phase 10).

What the spec asks for: "subtitle files deliberately offset by known amounts,
framerate-scaled (23.976<->25), and with inserted/removed intro segments, paired with
audio VAD traces". So, for each of a handful of short synthetic films:

1. A script of dialogue lines, spoken by Windows' own speech synthesiser (tts.ps1), with
   a quiet room, a street's worth of noise or a music bed under it.
2. The lines placed at known times and mixed into one 16 kHz track. The *true* subtitle
   file is written from those times, the way a person would time it: a little before the
   line starts, a little after it ends, two close lines sometimes sharing a subtitle, and
   now and then a voice with no subtitle at all.
3. The track run through the app's own speech detector (`eval subs --trace`), and only the
   resulting trace kept: the corpus is traces and subtitle files, which are small, and
   do not depend on a speech synthesiser being installed where the tests run.
4. Eight damaged copies of the true file per film, each with its damage recorded.

Stdlib only (ADR-0012). Deterministic from the seeds, apart from the synthesiser itself.

    python tools/subcorpus/build.py            # needs Windows, PowerShell and cargo
"""

import array
import json
import math
import os
import random
import shutil
import subprocess
import sys
import tempfile
import wave

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
OUT = os.path.join(ROOT, "fixtures", "subtitles")
RATE = 16_000

# (id, seed, minutes of film, bed: "room" | "street" | "music")
FILMS = [
    ("parlour", 11, 9, "room"),
    ("harbour", 23, 10, "street"),
    ("ballroom", 37, 9, "music"),
    ("station", 41, 11, "street"),
    ("garden", 53, 8, "room"),
    ("cabaret", 67, 10, "music"),
]
# Held out: built after the aligner was tuned on the six above, and never tuned on.
# eval subs reports them separately, so the tuned number and the honest one are both seen.
HOLDOUT = [
    ("lighthouse", 101, 12, "street"),
    ("orchard", 113, 9, "music"),
    ("tenement", 127, 11, "room"),
    ("carnival", 131, 10, "music"),
]

SUBJECTS = ["the captain", "your sister", "the old man", "nobody", "the doctor", "my father",
            "the girl from the station", "everyone here", "the inspector", "a stranger"]
VERBS = ["saw", "never trusted", "was waiting for", "asked about", "remembers", "followed",
         "will not forgive", "talked to", "lied about", "found"]
OBJECTS = ["the letter", "the last train", "that house", "the money", "her brother",
           "the harbour lights", "a red umbrella", "the second key", "the photograph", "you"]
TAILS = ["", "", " last night", " before the storm", " in the morning", " twice", " again",
         " on purpose", " without a word", " years ago"]
SHORT = ["Yes.", "No.", "Why?", "Listen to me.", "Not now.", "Where were you?", "I know.",
         "Go on.", "Wait.", "Of course.", "Are you sure?", "Come here."]
VOICES = ["Microsoft David Desktop", "Microsoft Zira Desktop"]


def sentence(rng):
    if rng.random() < 0.25:
        return rng.choice(SHORT)
    s = f"{rng.choice(SUBJECTS)} {rng.choice(VERBS)} {rng.choice(OBJECTS)}{rng.choice(TAILS)}"
    if rng.random() < 0.3:
        s += f", and {rng.choice(SUBJECTS)} {rng.choice(VERBS)} {rng.choice(OBJECTS)}"
    return s[0].upper() + s[1:] + rng.choice([".", ".", "?", "!"])


def script(seed, minutes):
    """Dialogue lines, each with the gap after it; scenes end in a longer quiet break.

    Enough lines to fill the film at about 2.6 s a line; place_and_mix stops when the film
    is full. `at` on the first line is when the dialogue begins, after the titles.
    """
    rng = random.Random(seed)
    lines = []
    estimate = rng.randint(20_000, 60_000)
    first = estimate
    while estimate < minutes * 60_000:
        voice_rate = [rng.randint(-2, 2), rng.randint(-2, 2)]
        scene = rng.randint(6, 20)
        for k in range(scene):
            who = k % 2 if rng.random() < 0.85 else rng.randint(0, 1)
            gap = rng.uniform(150, 900) if rng.random() < 0.6 else rng.uniform(900, 2500)
            lines.append({"text": sentence(rng), "voice": VOICES[who], "rate": voice_rate[who], "gap": gap})
            estimate += 2_600 + gap
        lines[-1]["gap"] = rng.uniform(4_000, 25_000)
        estimate += lines[-1]["gap"]
    lines[0]["at"] = first
    return lines


def speak(lines, workdir):
    tsv = os.path.join(workdir, "lines.tsv")
    with open(tsv, "w", encoding="utf-8") as f:
        for i, l in enumerate(lines):
            f.write(f"{i}\t{l['voice']}\t{l['rate']}\t{l['text']}\n")
    subprocess.run(["powershell", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File",
                    os.path.join(ROOT, "tools", "subcorpus", "tts.ps1"), "-Script", tsv, "-Out", workdir],
                   check=True)


def read_wav(path):
    with wave.open(path, "rb") as w:
        assert w.getframerate() == RATE and w.getnchannels() == 1 and w.getsampwidth() == 2, path
        return array.array("h", w.readframes(w.getnframes()))


def voiced(samples):
    """The span of a synthesised line that is actually sound, in samples (TTS pads)."""
    loud = [i for i in range(0, len(samples), 80) if abs(samples[i]) > 300]
    return (loud[0], loud[-1]) if loud else (0, len(samples))


def place_and_mix(lines, workdir, seed, minutes, bed):
    """Place each spoken line after the previous one plus its gap; mix with the bed."""
    rng = random.Random(seed * 7 + 1)
    total = minutes * 60 * RATE
    mix = array.array("f", bytes(4 * total))
    cursor = lines[0]["at"] * RATE // 1000
    placed = []
    for i, l in enumerate(lines):
        clip = read_wav(os.path.join(workdir, f"{i}.wav"))
        a, b = voiced(clip)
        if cursor + (b - a) >= total - RATE:
            break
        for j in range(a, b):
            mix[cursor + j - a] += clip[j] / 32768.0
        start_ms = cursor * 1000 // RATE
        end_ms = (cursor + b - a) * 1000 // RATE
        placed.append({"text": l["text"], "start": start_ms, "end": end_ms})
        cursor += (b - a) + int(l["gap"] * RATE / 1000)
    # The bed: a hiss for every film, louder for the street, a modulated chord for music.
    level = {"room": 0.004, "street": 0.03, "music": 0.006}[bed]
    state = seed & 0xFFFFFFFF or 1
    chord = [220.0, 277.18, 329.63]
    for n in range(total):
        state = (state * 1664525 + 1013904223) & 0xFFFFFFFF
        x = (state / 4294967296.0 - 0.5) * 2 * level
        if bed == "street" and (n // (RATE * 7)) % 3 == 0:  # passing traffic swells
            x *= 2.5
        if bed == "music":
            t = n / RATE
            swell = 0.5 + 0.5 * math.sin(2 * math.pi * 0.07 * t)
            x += 0.05 * swell * sum(math.sin(2 * math.pi * f * t) for f in chord) / 3
        mix[n] += x
    path = os.path.join(workdir, "film.wav")
    with wave.open(path, "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(RATE)
        w.writeframes(array.array("h", (max(-32767, min(32767, int(v * 32767))) for v in mix)).tobytes())
    return placed, path


def true_subtitles(placed, seed):
    """Time the lines as a subtitler would: lead in, linger, merge close pairs, skip some."""
    rng = random.Random(seed * 13 + 5)
    cues = []
    i = 0
    while i < len(placed):
        p = placed[i]
        if rng.random() < 0.05:  # a voice nobody subtitled
            i += 1
            continue
        start, end, text = p["start"] - rng.randint(0, 250), p["end"] + rng.randint(200, 800), p["text"]
        nxt = placed[i + 1] if i + 1 < len(placed) else None
        if nxt and nxt["start"] - p["end"] < 600 and len(text) + len(nxt["text"]) < 70 and rng.random() < 0.5:
            end, text = nxt["end"] + rng.randint(200, 800), f"{text}\n{nxt['text']}"
            i += 1
        cues.append([max(0, start), end, text])
        i += 1
    # No overlaps: a subtitle ends before the next begins.
    for a, b in zip(cues, cues[1:]):
        a[1] = min(a[1], b[0] - 40)
    return cues


def clock(ms):
    ms = max(0, int(round(ms)))
    return f"{ms // 3600000:02}:{ms // 60000 % 60:02}:{ms // 1000 % 60:02},{ms % 1000:03}"


def srt(cues):
    return "".join(f"{i}\n{clock(s)} --> {clock(e)}\n{t}\n\n" for i, (s, e, t) in enumerate(cues, 1))


def damaged(cues, seed, film_minutes):
    """Eight damaged copies: (name, description, scale, offset, split at, jump).

    No damage moves a cue before zero: a file cannot say where a subtitle clamped to
    00:00:00 belonged, so that would test the clamp, not the aligner.
    """
    rng = random.Random(seed * 31 + 7)
    room = (cues[0][0] - 1_000) / 1000  # seconds the first cue can move earlier

    def o(lo, hi):
        sign = rng.choice([-1, 1])
        v = rng.uniform(lo, hi)
        return sign * (min(v, room) if sign < 0 else v)

    out = [("aligned", {"kind": "none"}, 1.0, 0.0, None, 0.0)]
    out.append(("offset-small", {"kind": "offset"}, 1.0, o(0.4, 4.0) * 1000, None, 0.0))
    out.append(("offset-large", {"kind": "offset"}, 1.0, o(15, 90) * 1000, None, 0.0))
    out.append(("pal-speedup", {"kind": "scale", "from_fps": 23.976, "to_fps": 25}, 23.976 / 25, o(0, 8) * 1000, None, 0.0))
    out.append(("pal-slowdown", {"kind": "scale", "from_fps": 25, "to_fps": 23.976}, 25 / 23.976, o(0, 8) * 1000, None, 0.0))
    out.append(("24-to-25", {"kind": "scale", "from_fps": 24, "to_fps": 25}, 24 / 25, o(0, 8) * 1000, None, 0.0))
    # An intro REMOVED from the video: the subtitles were timed with it, so everything
    # after it is late by its length, and the file has a gap where it was.
    t_cut = rng.uniform(0.15, 0.4) * film_minutes * 60_000
    out.append(("intro-removed", {"kind": "split"}, 1.0, abs(o(0, 5)) * 1000, t_cut, rng.uniform(20, 90) * 1000))
    # An intro ADDED to the video: the subtitles never had it, so everything after it is
    # early. The added footage sits in a quiet stretch of the true film (logos, a title
    # card), so it is at most as long as a gap between subtitles there: a real file is
    # never out of order.
    gaps = [(i, cues[i][0] - cues[i - 1][1]) for i in range(1, len(cues))
            if cues[i][0] - cues[i - 1][1] >= 8_000 and 0.15 <= cues[i][0] / (film_minutes * 60_000) <= 0.6]
    i, gap = rng.choice(gaps) if gaps else (len(cues) // 3, 4_000)
    added = rng.uniform(0.6, 0.95) * gap
    out.append(("intro-added-and-scaled", {"kind": "split+scale", "from_fps": 25, "to_fps": 23.976},
                25 / 23.976, abs(o(0, 5)) * 1000, cues[i][0], -added))
    return out


def write_variants(manifest, film_id, seed, minutes, cues):
    for name, desc, scale, offset, cut, jump in damaged(cues, seed, minutes):
        # A cue moves as a whole, by where it starts: one straddling the cut must not
        # become a ninety-second subtitle.
        def f(t, start):
            return scale * t + offset + (jump if cut is not None and start >= cut else 0.0)
        bad = [[f(s, s), f(e, s), txt] for s, e, txt in cues]
        assert all(s >= 0 for s, _, _ in bad), (film_id, name)
        vid = f"{film_id}-{name}"
        with open(os.path.join(OUT, "variants", f"{vid}.srt"), "w", encoding="utf-8", newline="\n") as fh:
            fh.write(srt(bad))
        manifest["variants"].append({"id": vid, "film": film_id, **desc, "scale": scale,
                                     "offset_ms": round(offset), "split_at_ms": None if cut is None else round(cut),
                                     "split_jump_ms": round(jump)})


def read_srt(path):
    """A true file back from disk, for --variants-only."""
    def ms(c):
        return ((int(c[0:2]) * 60 + int(c[3:5])) * 60 + int(c[6:8])) * 1000 + int(c[9:12])
    cues = []
    for block in open(path, encoding="utf-8").read().strip().split("\n\n"):
        lines = block.split("\n")
        a, b = lines[1].split(" --> ")
        cues.append([ms(a), ms(b), "\n".join(lines[2:])])
    return cues


def variants_only():
    """Rewrite the damaged copies from the committed true files; no speech synthesis."""
    shutil.rmtree(os.path.join(OUT, "variants"), ignore_errors=True)
    os.makedirs(os.path.join(OUT, "variants"))
    with open(os.path.join(OUT, "manifest.json"), encoding="utf-8") as f:
        manifest = json.load(f)
    manifest["variants"] = []
    seeds = {film_id: (seed, minutes) for film_id, seed, minutes, _ in FILMS + HOLDOUT}
    for film in manifest["films"]:
        seed, minutes = seeds[film["id"]]
        cues = read_srt(os.path.join(OUT, "films", f"{film['id']}.srt"))
        write_variants(manifest, film["id"], seed, minutes, cues)
    with open(os.path.join(OUT, "manifest.json"), "w", encoding="utf-8", newline="\n") as f:
        json.dump(manifest, f, indent=1)
        f.write("\n")


def main():
    if "--variants-only" in sys.argv:
        return variants_only()
    if os.name != "nt":
        sys.exit("needs Windows' speech synthesiser")
    if os.path.isdir(OUT):
        for sub in ("films", "variants"):
            shutil.rmtree(os.path.join(OUT, sub), ignore_errors=True)
    os.makedirs(os.path.join(OUT, "films"), exist_ok=True)
    os.makedirs(os.path.join(OUT, "variants"), exist_ok=True)
    subprocess.run(["cargo", "build", "--release", "-p", "eval"], cwd=ROOT, check=True)
    evalbin = os.path.join(ROOT, "target", "release", "eval.exe")
    manifest = {"films": [], "variants": []}
    for film_id, seed, minutes, bed in FILMS + HOLDOUT:
        with tempfile.TemporaryDirectory() as work:
            lines = script(seed, minutes)
            speak(lines, work)
            placed, wav = place_and_mix(lines, work, seed, minutes, bed)
            trace = os.path.join(OUT, "films", f"{film_id}.trace")
            subprocess.run([evalbin, "subs", "--trace", wav, trace], check=True)
        cues = true_subtitles(placed, seed)
        with open(os.path.join(OUT, "films", f"{film_id}.srt"), "w", encoding="utf-8", newline="\n") as f:
            f.write(srt(cues))
        with open(os.path.join(OUT, "films", f"{film_id}.voice.tsv"), "w", encoding="utf-8", newline="\n") as f:
            f.writelines(f"{p['start']}\t{p['end']}\n" for p in placed)
        manifest["films"].append({"id": film_id, "minutes": minutes, "bed": bed, "lines": len(placed), "cues": len(cues),
                                  "holdout": any(h[0] == film_id for h in HOLDOUT)})
        write_variants(manifest, film_id, seed, minutes, cues)
        print(f"{film_id}: {len(placed)} lines, {len(cues)} cues")
    with open(os.path.join(OUT, "manifest.json"), "w", encoding="utf-8", newline="\n") as f:
        json.dump(manifest, f, indent=1)
        f.write("\n")


if __name__ == "__main__":
    main()
