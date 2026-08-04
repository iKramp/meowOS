#!/usr/bin/env python3

from pathlib import Path


def sort_stack_sizes(path: str | Path) -> None:
    path = Path(path)

    entries = []

    with path.open("r", encoding="utf-8", errors="replace") as f:
        for line in f:
            stripped = line.lstrip()

            if not stripped or not stripped[0].isdigit():
                continue

            parts = stripped.split(maxsplit=1)
            try:
                size = int(parts[0])
            except ValueError:
                continue

            entries.append((size, line))

    entries.sort(key=lambda e: e[0], reverse=True)

    with path.open("w", encoding="utf-8") as f:
        for _, line in entries:
            f.write(line)


def main():
    import sys

    if len(sys.argv) != 2:
        print(f"Usage: {sys.argv[0]} <file>")
        raise SystemExit(1)

    sort_stack_sizes(sys.argv[1])


if __name__ == "__main__":
    main()
