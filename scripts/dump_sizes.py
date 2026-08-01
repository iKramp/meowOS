#!/usr/bin/env python3

import subprocess
import sys
import os
from pathlib import Path
from sort_type_sizes import sort_type_sizes
from filter_type_sizes import filter_type_sizes

OUTPUT_FILE = Path("./type_sizes.txt")

def run_cargo_command(cmd):
    env = os.environ.copy()
    env["RUSTFLAGS"] = "-Zprint-type-sizes"
    try:
        result = subprocess.run(
            cmd,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            check=False,
            env=env
        )
    except FileNotFoundError:
        print(f"Error: '{cmd[0]}' was not found in PATH.", file=sys.stderr)
        sys.exit(1)

    return result

def main():
    cmd = [
        "cargo",
        "clean"
    ]
    run_cargo_command(cmd)

    cmd = [
        "cargo",
        "build",
    ]
    result = run_cargo_command(cmd)
    OUTPUT_FILE.write_text(result.stdout, encoding="utf-8")

    filter_type_sizes(OUTPUT_FILE)
    sort_type_sizes(OUTPUT_FILE)

    # Preserve cargo's exit status.
    sys.exit(result.returncode)


if __name__ == "__main__":
    main()
