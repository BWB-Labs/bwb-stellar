#!/usr/bin/env python3
from __future__ import annotations

import os
import sys
from pathlib import Path

THRESHOLD = float(os.environ.get("COVERAGE_THRESHOLD", "0.90"))

# Only enforce coverage for production contracts.
ENFORCE_PREFIX = Path(os.environ.get("COVERAGE_ENFORCE_PREFIX", "src")).as_posix().rstrip("/") + "/"
EXCLUDE_PREFIXES = tuple(
    p.strip().rstrip("/") + "/"
    for p in os.environ.get("COVERAGE_EXCLUDE_PREFIXES", "script/,test/,src/test/").split(",")
    if p.strip()
)

lcov_path = Path(sys.argv[1] if len(sys.argv) > 1 else "lcov.info")
if not lcov_path.exists():
    print(f"lcov file not found: {lcov_path}", file=sys.stderr)
    sys.exit(2)

bad: list[tuple[str, float, int, int]] = []

cur_file: str | None = None
total = hit = 0


def flush():
    global cur_file, total, hit
    if cur_file is None:
        return

    f = cur_file.replace("\\", "/")

    enforce = f.startswith(ENFORCE_PREFIX) and not f.startswith(EXCLUDE_PREFIXES)
    if enforce and total > 0:
        pct = hit / total
        if pct + 1e-12 < THRESHOLD:
            bad.append((f, pct, hit, total))

    cur_file = None
    total = hit = 0

for line in lcov_path.read_text().splitlines():
    if line.startswith("SF:"):
        flush()
        cur_file = line[3:].strip()
        # Make it relative-ish if forge wrote absolute paths
        # (keeps matching like "src/..." working)
        cur_file = cur_file.replace(str(Path.cwd()).replace("\\", "/") + "/", "")
    elif line.startswith("DA:") and cur_file is not None:
        # DA:<line>,<count>[,<checksum>]
        _, rest = line.split(":", 1)
        parts = rest.split(",")
        count = int(parts[1])
        total += 1
        if count > 0:
            hit += 1
    elif line.strip() == "end_of_record":
        flush()

flush()

if bad:
    print(f"Coverage threshold: {THRESHOLD*100:.2f}%")
    for f, pct, h, t in sorted(bad):
        print(f"FAIL {pct*100:6.2f}%  ({h}/{t})  {f}")
    sys.exit(1)

print(f"OK: all enforced files >= {THRESHOLD*100:.2f}%")
