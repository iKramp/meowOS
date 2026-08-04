#!/usr/bin/env python3

import subprocess
import sys
import os
from pathlib import Path
from sort_type_sizes import sort_type_sizes
from filter_type_sizes import filter_type_sizes
from sort_stack_sizes import sort_stack_sizes

OUTPUT_FILE = Path("./perf_analysis/type_sizes.txt")
STACK_OUTPUT_FILE = Path("./perf_analysis/stack_sizes.txt")

def run_cargo_command(cmd, rustflags):
    env = os.environ.copy()
    env["RUSTFLAGS"] = rustflags
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


def dump_stack_sizes():
    elf_file = Path("./kernel_build_files/kernel.elf")

    if not elf_file.exists():
        print(f"Error: {elf_file} not found.", file=sys.stderr)
        sys.exit(1)

    result = subprocess.run(
        [
            "llvm-readelf",
            "--stack-sizes",
            str(elf_file),
        ],
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        check=False,
    )

    STACK_OUTPUT_FILE.write_text(result.stdout, encoding="utf-8")

def main():
    cmd = ["cargo", "clean"]
    run_cargo_command(cmd, "")

    cmd = ["cargo", "build"]
    result = run_cargo_command(cmd, "-Zprint-type-sizes -Zemit-stack-sizes")
    OUTPUT_FILE.write_text(result.stdout, encoding="utf-8")

    filter_type_sizes(OUTPUT_FILE)
    sort_type_sizes(OUTPUT_FILE)

    dump_stack_sizes()
    sort_stack_sizes(STACK_OUTPUT_FILE)

    # Preserve cargo's exit status.
    sys.exit(result.returncode)


if __name__ == "__main__":
    main()
