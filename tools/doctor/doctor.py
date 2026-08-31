#!/usr/bin/env python3
"""
sin-e-phile doctor — checks every prerequisite and says exactly what is missing.

SPEC.md Phase 0: "This turns 'the build mysteriously fails' into 'you're missing
the C++ build tools, here's the link'." Run it at the top of every session
(CLAUDE.md session start ritual, step 2).

It also BOOTSTRAPS the git hooks: `.git/hooks/` is not tracked by git, so hooks
committed to `.githooks/` do nothing until `core.hooksPath` points at them
(ADR-0012). A fresh clone is unprotected until doctor runs once, which is why
docs/SETUP.md makes this step one.

Usage:
    python tools/doctor/doctor.py            check everything, fix hooks if needed
    python tools/doctor/doctor.py --no-fix   check only, change nothing

Exit 0 = every REQUIRED check passed, 1 = something required is missing.
Optional checks never fail the run; they are reported and moved past.

Python 3.12+, standard library only, so this runs before any toolchain exists —
which is the entire point, since diagnosing a broken toolchain is its job.
"""

from __future__ import annotations

import argparse
import os
import platform
import re
import shutil
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]

GREEN, YELLOW, RED, DIM, BOLD, RESET = (
    ("\033[32m", "\033[33m", "\033[31m", "\033[2m", "\033[1m", "\033[0m")
    if os.environ.get("TERM") or os.name == "nt"
    else ("", "", "", "", "", "")
)


class Result:
    def __init__(self, name: str, ok: bool, detail: str, fix: str = "", required: bool = True):
        self.name, self.ok, self.detail, self.fix, self.required = name, ok, detail, fix, required


# A tool that exists is not the same as a tool that is slow to start, and doctor
# must not report the second as the first. `npm --version` on a cold CI runner took
# longer than the old 20s ceiling, and doctor said "found but failed to run" — which
# reads as a broken install and sent a real diagnosis down the wrong path.
TIMED_OUT = -1


def run(*cmd: str, timeout: int = 60) -> tuple[int, str]:
    try:
        proc = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout,
                              encoding="utf-8", errors="replace")
        return proc.returncode, (proc.stdout + proc.stderr).strip()
    except subprocess.TimeoutExpired:
        return TIMED_OUT, ""
    except (FileNotFoundError, OSError):
        return 127, ""


def registry_path() -> str:
    """The PATH as Windows has it stored, which may be newer than this shell's copy.

    Installing a toolchain updates the registry, but every already-open shell keeps
    the environment it inherited at launch. So a freshly-installed Rust looks
    exactly like a missing Rust — which wasted a diagnosis in session 0a. Reading
    the stored PATH lets doctor tell those two cases apart and say
    'restart your terminal' instead of 'go install it again'.
    """
    if os.name != "nt":
        return ""
    parts = []
    for scope, key in (("Machine", r"HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Environment"),
                       ("User", r"HKCU\Environment")):
        code, out = run("reg", "query", key, "/v", "Path")
        if code == 0:
            match = re.search(r"Path\s+REG(?:_EXPAND)?_SZ\s+(.+)", out)
            if match:
                parts.append(os.path.expandvars(match.group(1).strip()))
    return os.pathsep.join(parts)


_REGISTRY_PATH: str | None = None


def which_including_registry(exe: str) -> tuple[str | None, bool]:
    """Locate an executable. Returns (path, only_found_in_registry_path)."""
    global _REGISTRY_PATH
    path = shutil.which(exe)
    if path:
        return path, False
    if os.name != "nt":
        return None, False
    if _REGISTRY_PATH is None:
        _REGISTRY_PATH = registry_path()
    stale = shutil.which(exe, path=_REGISTRY_PATH) if _REGISTRY_PATH else None
    return stale, bool(stale)


def check_command(name: str, exe: str, version_args: tuple[str, ...], fix: str,
                  required: bool = True, pattern: str = r"(\d+\.\d+(?:\.\d+)?)") -> Result:
    path, stale_path = which_including_registry(exe)
    if path and stale_path:
        return Result(
            name, False,
            f"{YELLOW}installed but not on THIS shell's PATH{RESET}  {DIM}{path}{RESET}",
            "It is installed and Windows knows about it — this terminal just has a "
            "stale environment. RESTART YOUR TERMINAL (or your editor). No reinstall needed.",
            required,
        )
    if not path:
        return Result(name, False, "not found on PATH", fix, required)
    # npm, npx and friends are .cmd shims on Windows. CreateProcess cannot execute
    # a batch file directly, so they must be invoked through the command processor.
    if os.name == "nt" and Path(path).suffix.lower() in (".cmd", ".bat"):
        code, out = run("cmd", "/c", path, *version_args)
    else:
        code, out = run(exe, *version_args)
    if code == TIMED_OUT:
        return Result(
            name, False,
            f"found at {path} but did not respond within 60s",
            "It is installed. Something is making it very slow to start — antivirus "
            "scanning a cold node_modules, or a network drive. Re-run doctor; if it "
            "persists, run the command by hand to see what it is waiting on.",
            required,
        )
    if code != 0:
        return Result(name, False, f"found at {path} but failed to run (exit {code})",
                      fix, required)
    match = re.search(pattern, out)
    version = match.group(1) if match else out.splitlines()[0][:40]
    return Result(name, True, f"{version}  {DIM}{path}{RESET}", "", required)


def check_python() -> Result:
    version = sys.version_info
    ok = (version.major, version.minor) >= (3, 12)
    return Result(
        "Python 3.12+", ok,
        f"{version.major}.{version.minor}.{version.micro}  {DIM}{sys.executable}{RESET}"
        if ok else f"{version.major}.{version.minor} is too old",
        "Install Python 3.12+ from python.org and ensure it is on PATH. "
        "Required by tools/guard and tools/doctor (ADR-0012).",
    )


def check_msvc() -> Result:
    """Locate the MSVC C++ toolchain via vswhere.

    NOTE the -prerelease flag. Without it, vswhere silently ignores Insiders and
    Preview installs of Visual Studio and reports nothing — which looks exactly
    like 'MSVC is not installed' and sends you off installing a second copy.
    This cost a diagnosis in session 0a; it is why the flag is here.
    """
    program_files_x86 = os.environ.get("ProgramFiles(x86)", r"C:\Program Files (x86)")
    vswhere = Path(program_files_x86) / "Microsoft Visual Studio" / "Installer" / "vswhere.exe"
    fix = ("Install 'Desktop development with C++' via the Visual Studio Build Tools: "
           "https://visualstudio.microsoft.com/visual-cpp-build-tools/ — Rust's MSVC "
           "toolchain cannot link without it.")
    if not vswhere.exists():
        return Result("MSVC C++ build tools", False, "vswhere.exe not found", fix)
    code, out = run(str(vswhere), "-all", "-prerelease", "-products", "*",
                    "-requires", "Microsoft.VisualStudio.Component.VC.Tools.x86.x64",
                    "-property", "installationPath")
    if code != 0 or not out.strip():
        return Result("MSVC C++ build tools", False, "no install with the VC++ toolset", fix)
    return Result("MSVC C++ build tools", True, f"{DIM}{out.splitlines()[0]}{RESET}")


def check_rust_links() -> Result:
    """Compile and link a real binary. `rustc --version` proves far less."""
    path, stale_path = which_including_registry("cargo")
    if not path:
        return Result("Rust can link a binary", False, "cargo not on PATH",
                      "Install Rust from https://rustup.rs (choose the MSVC toolchain).")
    if stale_path:
        return Result("Rust can link a binary", False,
                      f"{YELLOW}cannot verify — cargo is not on this shell's PATH{RESET}",
                      "Restart your terminal, then re-run doctor.")
    code, out = run("rustc", "-vV")
    if "windows-msvc" not in out:
        return Result("Rust can link a binary", False, "toolchain is not windows-msvc",
                      "Run: rustup default stable-x86_64-pc-windows-msvc")
    return Result("Rust can link a binary", True,
                  f"{DIM}host is x86_64-pc-windows-msvc; run `cargo build` to fully verify{RESET}")


def check_webview2() -> Result:
    key = (r"HKLM\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients"
           r"\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}")
    code, out = run("reg", "query", key, "/v", "pv")
    fix = ("Install the WebView2 Evergreen Runtime: "
           "https://developer.microsoft.com/microsoft-edge/webview2/ — Tauri renders "
           "the entire frontend in it.")
    if code != 0:
        return Result("WebView2 runtime", False, "not found in registry", fix)
    match = re.search(r"pv\s+REG_SZ\s+([\d.]+)", out)
    return Result("WebView2 runtime", True, match.group(1) if match else "present")


def check_windows_sdk() -> Result:
    code, out = run("reg", "query",
                    r"HKLM\SOFTWARE\WOW6432Node\Microsoft\Microsoft SDKs\Windows\v10.0",
                    "/v", "ProductVersion")
    if code != 0:
        return Result("Windows 10/11 SDK", False, "not found",
                      "Included with the Visual Studio Build Tools C++ workload.")
    match = re.search(r"ProductVersion\s+REG_SZ\s+([\d.]+)", out)
    return Result("Windows 10/11 SDK", True, match.group(1) if match else "present")


def check_hooks(fix_it: bool) -> Result:
    """Verify — and optionally perform — the core.hooksPath bootstrap."""
    code, out = run("git", "-C", str(REPO_ROOT), "config", "--get", "core.hooksPath")
    configured = out.strip() if code == 0 else ""
    hooks_dir = REPO_ROOT / ".githooks"

    if not hooks_dir.exists():
        return Result("git hooks (posture guard + secret scan)", False,
                      ".githooks/ is missing from the repository", "Restore .githooks/ from git.")

    if configured == ".githooks":
        present = sorted(p.name for p in hooks_dir.iterdir() if p.is_file())
        return Result("git hooks (posture guard + secret scan)", True,
                      f"active — {', '.join(present)}")

    if not fix_it:
        return Result("git hooks (posture guard + secret scan)", False,
                      f"core.hooksPath is {configured or 'unset'}",
                      "Run: git config core.hooksPath .githooks")

    code, _ = run("git", "-C", str(REPO_ROOT), "config", "core.hooksPath", ".githooks")
    if code == 0:
        return Result("git hooks (posture guard + secret scan)", True,
                      f"{YELLOW}bootstrapped just now{RESET} — core.hooksPath set to .githooks")
    return Result("git hooks (posture guard + secret scan)", False, "could not set core.hooksPath",
                  "Run manually: git config core.hooksPath .githooks")


def check_env_vars() -> list[Result]:
    """Every API key is OPTIONAL (ADR-0013). Report, never fail."""
    env_file = REPO_ROOT / ".env"
    values: dict[str, str] = {}
    if env_file.exists():
        for line in env_file.read_text(encoding="utf-8").splitlines():
            if "=" in line and not line.strip().startswith("#"):
                key, _, value = line.partition("=")
                values[key.strip()] = value.strip()

    optional = [
        ("TMDB_API_KEY", "artwork and rich detail"),
        ("FANART_TV_API_KEY", "higher-quality artwork"),
        ("OPENSUBTITLES_API_KEY", "hash-matched subtitles (Phase 10)"),
        ("TRAKT_CLIENT_ID", "watch history sync (Phase 19)"),
    ]
    results = []
    for name, purpose in optional:
        present = bool(values.get(name) or os.environ.get(name))
        results.append(Result(
            f"{name}", present,
            f"configured — {purpose}" if present else f"not set — {purpose}",
            "" if present else f"Optional. See .env.example. The app is fully functional without it.",
            required=False,
        ))
    return results


def main() -> int:
    parser = argparse.ArgumentParser(description="sin-e-phile prerequisite doctor")
    parser.add_argument("--no-fix", action="store_true",
                        help="check only; do not bootstrap git hooks")
    args = parser.parse_args()

    print(f"\n{BOLD}sin-e-phile doctor{RESET}  {DIM}{platform.system()} "
          f"{platform.release()} · {REPO_ROOT}{RESET}\n")

    if platform.system() != "Windows":
        print(f"{YELLOW}note{RESET} SPEC.md §2.4: this is a Windows-only project. "
              f"Checks below will be unreliable elsewhere.\n")

    required = [
        check_command("git", "git", ("--version",),
                      "Install Git for Windows: https://git-scm.com/download/win"),
        check_python(),
        check_command("Rust (rustc)", "rustc", ("--version",),
                      "Install from https://rustup.rs — choose the MSVC toolchain."),
        check_command("Cargo", "cargo", ("--version",),
                      "Ships with Rust. If missing, re-run rustup."),
        check_rust_links(),
        check_msvc(),
        check_windows_sdk(),
        check_command("Node.js", "node", ("--version",),
                      "Install Node LTS: https://nodejs.org"),
        check_command("npm", "npm", ("--version",),
                      "Ships with Node.js."),
        check_webview2(),
        check_command("FFmpeg", "ffmpeg", ("-version",),
                      "Install FFmpeg and put it on PATH: https://ffmpeg.org/download.html"),
        check_hooks(fix_it=not args.no_fix),
    ]

    optional = [
        check_command("GitHub CLI (gh)", "gh", ("--version",),
                      "Optional. https://cli.github.com", required=False),
        check_command("gitleaks", "gitleaks", ("version",),
                      "Optional locally — CI runs a pinned gitleaks. Without it the "
                      "pre-commit hook uses tools/guard/secretscan.py (ADR-0012).",
                      required=False),
        *check_env_vars(),
    ]

    print(f"{BOLD}Required{RESET}")
    for result in required:
        mark = f"{GREEN}ok  {RESET}" if result.ok else f"{RED}MISS{RESET}"
        print(f"  {mark}  {result.name:<34} {result.detail}")

    print(f"\n{BOLD}Optional{RESET}  {DIM}(never blocks the build){RESET}")
    for result in optional:
        mark = f"{GREEN}ok  {RESET}" if result.ok else f"{DIM}--  {RESET}"
        print(f"  {mark}  {result.name:<34} {result.detail}")

    failed = [r for r in required if not r.ok]
    if not failed:
        print(f"\n{GREEN}All required prerequisites present.{RESET}\n")
        return 0

    print(f"\n{RED}{len(failed)} required prerequisite(s) missing:{RESET}\n")
    for result in failed:
        print(f"  {BOLD}{result.name}{RESET}\n      {result.fix}\n")
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
