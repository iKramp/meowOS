#!/usr/bin/env python3

import re
from pathlib import Path


TYPE_RE = re.compile(r"^print-type-size type: `.*`: (\d+) bytes")


def sort_type_sizes(path: str | Path) -> None:
    path = Path(path)

    with path.open("r", encoding="utf-8", errors="replace") as f:
        lines = [l for l in f if l.startswith("print-type-size")]

    blocks = []
    current = None

    for line in lines:
        m = TYPE_RE.match(line)
        if m:
            if current is not None:
                blocks.append(current)
            current = {
                "size": int(m.group(1)),
                "lines": [line],
            }
        elif current is not None:
            current["lines"].append(line)

    if current is not None:
        blocks.append(current)

    blocks.sort(key=lambda b: b["size"], reverse=True)

    with path.open("w", encoding="utf-8") as f:
        for block in blocks:
            f.writelines(block["lines"])


def main():
    import sys

    if len(sys.argv) != 2:
        print(f"Usage: {sys.argv[0]} <file>")
        raise SystemExit(1)

    sort_type_sizes(sys.argv[1])


if __name__ == "__main__":
    main()
