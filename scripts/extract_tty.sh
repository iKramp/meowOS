#!/usr/bin/env bash

INPUT="./qemu_serial.log"
OUTPUT="./tty_out.log"
TARGET="[kernel/src/tty.rs:271]:"

awk -v target="$TARGET" '
    /^\[[^]]+\]:/ {
        capture = (index($0, target) == 1)
    }

    capture {
        print
    }
' "$INPUT" > "$OUTPUT"

echo "Extracted lines to $OUTPUT"
