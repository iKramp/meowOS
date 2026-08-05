#!/usr/bin/env python3

import re
from collections import defaultdict

STACK_SIZES_FILE = "./perf_analysis/stack_sizes.txt"
CALLGRAPH_FILE = "./perf_analysis/callgraph.txt"

# Fill this in with your kernel entry points.
# Example:
# ROOTS = [
#     "_RNvCs...kernel_main",
#     "_RNvCs...irq_timer",
#     "_RNvCs...page_fault",
# ]
ROOTS = [
    # "_start",
    # "_RNvNtNtCsfO4Hqg3m9yM_6kernel4proc7syscall7handler",
    # "_RNvNtNtCsfO4Hqg3m9yM_6kernel10interrupts6macros25general_interrupt_handler"
]

# Fill this in with indirect/dynamic dispatches that the static callgraph misses.
# Format:
#     caller: [callee1, callee2, ...]
MANUAL_CALLS = {
    # "_RNv...scheduler": [
    #     "_RNv...task_a",
    #     "_RNv...task_b",
    # ],
}

# ------------------------------------------------------------

stack_size = {}
graph = {}

# Read stack sizes
with open(STACK_SIZES_FILE) as f:
    for line in f:
        line = line.strip()
        if not line:
            continue
        size, func = line.split(maxsplit=1)
        stack_size[func] = int(size)

# Read callgraph
with open(CALLGRAPH_FILE) as f:
    current = None

    for line in f:
        line = line.rstrip()

        m = re.match(r"Call graph node for function: '([^']+)'", line)
        if m:
            current = m.group(1)
            continue

        if current is None:
            continue

        m = re.search(r"calls function '([^']+)'", line)
        if m:
            graph.setdefault(current, []).append(m.group(1))

# Ensure every function exists in graph
for fn in stack_size:
    graph.setdefault(fn, [])

# Add manually specified indirect calls.
for caller, callees in MANUAL_CALLS.items():
    graph.setdefault(caller, [])

    for callee in callees:
        if callee not in graph[caller]:
            graph[caller].append(callee)

        # Ensure the callee also exists as a node.
        graph.setdefault(callee, [])

memo = {}
memo_path = {}

def dfs(fn, visiting):
    if fn in memo:
        return memo[fn]

    if fn in visiting:
        # Ignore recursive calls.
        return 0, [fn]

    visiting.add(fn)

    best_size = 0
    best_path = []

    for callee in graph.get(fn, ()):
        child_size, child_path = dfs(callee, visiting)
        if child_size > best_size:
            best_size = child_size
            best_path = child_path

    visiting.remove(fn)

    total = stack_size.get(fn, 0) + best_size

    memo[fn] = (total, [fn] + best_path)
    return memo[fn]

# Compute every function once.
for fn in graph:
    dfs(fn, set())

if ROOTS:
    best_root = None
    best_total = -1
    best_path = None

    for root in ROOTS:
        total, path = memo.get(root, (stack_size.get(root, 0), [root]))
        if total > best_total:
            best_total = total
            best_root = root
            best_path = path

    print(f"Maximum stack from roots: {best_total} bytes")
    print()

    running = 0
    for fn in best_path:
        sz = stack_size.get(fn, 0)
        running += sz
        print(f"{running:8} (+{sz:5}) {fn}")

else:
    best_fn = None
    best_total = -1
    best_path = None

    for fn, (total, path) in memo.items():
        if total > best_total:
            best_total = total
            best_fn = fn
            best_path = path

    print("ROOTS list is empty.")
    print("Showing function with largest downstream stack usage.\n")

    print(f"Maximum: {best_total} bytes")
    print()

    running = 0
    for fn in best_path:
        sz = stack_size.get(fn, 0)
        running += sz
        print(f"{running:8} (+{sz:5}) {fn}")
