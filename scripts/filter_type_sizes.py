import re
from pathlib import Path

TYPE_RE = re.compile(
    r"^print-type-size type: `(.*)`: (\d+) bytes"
)

import re
from pathlib import Path

ASYNC_BLOCK_RE = re.compile(
    r"^\{async block@(.+):(\d+):\d+:\s*\d+:\d+\}$"
)


def is_async_trait_box_pin(source_path: Path, line_no: int) -> bool:
    try:
        lines = source_path.read_text(
            encoding="utf-8",
            errors="replace",
        ).splitlines()
    except OSError:
        return False

    if line_no < 1 or line_no > len(lines):
        return False

    # Direct Box::pin on the async block line.
    if "Box::pin" in lines[line_no - 1]:
        return True

    if line_no - 2 >= 0 and "heap_future" in lines[line_no - 2]:
        return True

    # If the line is indented, walk upwards until reaching the containing
    # non-indented item (usually impl/trait/function declaration).
    idx = line_no - 1

    if lines[idx] and lines[idx][0].isspace():
        while idx > 0 and (
            not lines[idx] or lines[idx][0].isspace()
        ):
            idx -= 1

    # Search this line and up to 3 lines above for async_trait.
    start = max(0, idx - 3)

    for i in range(start, idx + 1):
        if "async_trait" in lines[i]:
            return True

    return False




def should_keep_type(type_name: str) -> bool:
    if type_name.startswith(("core::", "std::", "alloc::")):
        return False

    m = ASYNC_BLOCK_RE.match(type_name)
    if not m:
        return True

    source_path = Path(m.group(1))
    line_no = int(m.group(2))

    if is_async_trait_box_pin(source_path, line_no):
        return False

    return True


def filter_type_sizes(path: str | Path) -> None:
    path = Path(path)

    with path.open("r", encoding="utf-8", errors="replace") as f:
        lines = f.readlines()

    filtered = []
    current = None
    keep = False

    for line in lines:
        m = TYPE_RE.match(line)
        if m:
            # Flush previous block.
            if current is not None and keep:
                filtered.extend(current)

            type_name = m.group(1)
            keep = should_keep_type(type_name)

            current = [line]
        elif current is not None:
            current.append(line)

    # Flush last block.
    if current is not None and keep:
        filtered.extend(current)

    with path.open("w", encoding="utf-8") as f:
        f.writelines(filtered)
