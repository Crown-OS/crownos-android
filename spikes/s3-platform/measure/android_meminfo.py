#!/usr/bin/env python3
"""Top `dumpsys meminfo` entries inside Android. Needs root (`waydroid shell`)."""

import argparse
import re

import common

ENTRY = re.compile(r"^\s+(?P<kb>[\d,]+)K: (?P<name>.+?)(?: \(pid (?P<pid>\d+)[^)]*\))?\s*$")
SECTION = re.compile(r"^Total (PSS|RSS) by (process|OOM adjustment|category):$")
SUMMARY = re.compile(r"^\s*(Total RAM|Free RAM|Used RAM|Lost RAM|ZRAM):\s+([\d,]+)K")


def kilobytes(text: str) -> int:
    return int(text.replace(",", ""))


def top_level(rows: list[tuple[int, dict]], top: int) -> list[dict]:
    """dumpsys right-aligns sizes, so top-level rows are the ones whose `K:` sits furthest left."""
    column = min((row_column for row_column, _ in rows), default=0)
    return [row for row_column, row in rows if row_column == column][:top]


def parse(dump: str, top: int) -> dict:
    sections, summary, current = {}, {}, None
    for line in dump.splitlines():
        if heading := SECTION.match(line):
            current = sections.setdefault(f"{heading[1].lower()}_by_{heading[2].replace(' ', '_').lower()}", [])
        elif (entry := ENTRY.match(line)) and current is not None:
            current.append((entry.end("kb"), {"kb": kilobytes(entry["kb"]), "name": entry["name"],
                                              **({"pid": int(entry["pid"])} if entry["pid"] else {})}))
        elif match := SUMMARY.match(line):
            summary[match[1].lower().replace(" ", "_") + "_kb"] = kilobytes(match[2])
            current = None
        elif not line.strip():
            current = None
    return {"sections": {name: top_level(rows, top) for name, rows in sections.items()}, "summary": summary}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--top", type=int, default=15)
    parser.add_argument("--label", default="idle-home")
    args = parser.parse_args()

    dump = common.run(["waydroid", "shell", "--", "dumpsys", "meminfo"], timeout_s=180)
    if dump.returncode != 0 or not dump.stdout:
        common.emit("android_meminfo", {"label": args.label, "error": dump.stderr.strip()[:300]})
        raise SystemExit(1)
    common.emit("android_meminfo", {"label": args.label, **parse(dump.stdout, args.top)})


if __name__ == "__main__":
    main()
