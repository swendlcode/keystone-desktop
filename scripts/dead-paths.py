#!/usr/bin/env python3
"""Find backend commands and service methods that nothing calls.

Run with `python3 scripts/dead-paths.py`. Exit code is always 0: this is a
report to read, not a gate — some of what it lists is deliberate.

Two separate questions, and they fail differently:

1. A registered command with no `invoke('name')` anywhere is unreachable from
   the UI. It compiles, it is in the handler table, and no button leads to it.
2. A service method that calls `invoke` but has no caller outside the service
   layer is a wired path with no user. This is the shape that hid
   `set_track_tempo` and `get_cue_counts`: the IPC existed end to end and the
   only missing link was the last one.

Commands used only by the MCP server or the web bridge are not dead — they are
reached from Rust rather than from the webview — so those are reported
separately rather than counted as findings.
"""
import re
import subprocess
import sys
from pathlib import Path

# Control flow reads like a method call at one indent level.
KEYWORDS = {
    "constructor", "if", "for", "while", "switch", "return", "catch",
    "function", "await", "typeof", "new", "do", "else",
}

_HERE = Path(__file__).resolve().parent.parent

APPS = [("Keystone", str(_HERE), ["src-tauri/src/handlers.rs"])]


def registered_commands(root: Path, files) -> set[str]:
    """Names in the invoke handler table."""
    names = set()
    for f in files:
        p = root / f
        if not p.exists():
            continue
        text = p.read_text()
        # `module::command,` entries inside the handler list
        for m in re.finditer(r'(?:^|\s)(?:\$crate::)?commands::([a-z_]+)::([a-z_0-9]+)\s*,', text):
            names.add(m.group(2))
        for m in re.finditer(r'(?:^|\s)(?:\$crate::)?([a-z_]+_commands)::([a-z_0-9]+)\s*,', text):
            names.add(m.group(2))
    return names


def grep(root: Path, pattern: str, globs) -> list[str]:
    cmd = ["grep", "-rn", "-E", pattern, str(root / "src")]
    for g in globs:
        cmd += ["--include", g]
    r = subprocess.run(cmd, capture_output=True, text=True)
    return [l for l in r.stdout.splitlines() if l.strip()]


def rust_grep(root: Path, pattern: str) -> list[str]:
    r = subprocess.run(
        ["grep", "-rn", "-E", pattern, str(root / "src-tauri/src"), "--include", "*.rs"],
        capture_output=True, text=True,
    )
    return [l for l in r.stdout.splitlines() if l.strip()]


total_findings = 0
for app, root_s, handler_files in APPS:
    root = Path(root_s)
    print(f"\n{'=' * 64}\n{app}\n{'=' * 64}")

    cmds = registered_commands(root, handler_files)
    print(f"registered commands: {len(cmds)}")
    if not cmds:
        print("  !! could not parse the handler table")
        continue

    # --- 1. commands with no invoke() from the frontend ---------------
    invoked = set()
    for line in grep(root, r"invoke<?[^(]*\(\s*'[a-z_0-9]+'", ["*.ts", "*.tsx"]):
        for m in re.finditer(r"invoke<?[^(]*\(\s*'([a-z_0-9]+)'", line):
            invoked.add(m.group(1))

    unreachable = sorted(cmds - invoked)
    # Separate the ones Rust itself dispatches (MCP tools, web bridge).
    rust_only, truly_dead = [], []
    for name in unreachable:
        hits = rust_grep(root, rf'"{name}"')
        if hits:
            rust_only.append((name, len(hits)))
        else:
            truly_dead.append(name)

    print(f"\n  reachable from the UI      : {len(cmds & invoked)}")
    print(f"  dispatched from Rust only  : {len(rust_only)}"
          + (f"  {[n for n, _ in rust_only]}" if rust_only else ""))
    if truly_dead:
        print(f"  NO CALLER ANYWHERE ({len(truly_dead)}):")
        for n in truly_dead:
            print(f"      - {n}")
        total_findings += len(truly_dead)
    else:
        print("  NO CALLER ANYWHERE         : none")

    # --- 2. service methods nothing uses ------------------------------
    svc_dir = root / "src/services"
    if svc_dir.exists():
        dead_methods = []
        for svc in svc_dir.glob("*.ts"):
            text = svc.read_text()
            # `methodName(args): Promise<...> {` at one indent level
            for m in re.finditer(r'^  ([a-zA-Z_][a-zA-Z0-9_]*)\s*(?:<[^>]*>)?\(', text, re.M):
                name = m.group(1)
                if name in KEYWORDS:
                    continue
                # Any call site other than the definition line itself. A
                # helper used elsewhere in its own file is not dead; excluding
                # the whole services directory made four `writeAscii` calls
                # look unused when they are called three lines below.
                defn = f"{svc}:{text[:m.start()].count(chr(10)) + 1}:"
                callers = [
                    c for c in grep(root, rf'[.\s]{name}\(', ["*.ts", "*.tsx"])
                    if not c.startswith(defn)
                ]
                if not callers:
                    dead_methods.append(f"{svc.name}:{name}")
        if dead_methods:
            print(f"  SERVICE METHODS WITH NO CALLER ({len(dead_methods)}):")
            for d in dead_methods:
                print(f"      - {d}")
            total_findings += len(dead_methods)
        else:
            print("  service methods with no caller: none")

print(f"\n{'=' * 64}\nfindings to review: {total_findings}\n{'=' * 64}")
sys.exit(0)
