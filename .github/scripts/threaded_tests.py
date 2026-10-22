"""Lists the test functions of a Rust file using threads, as libtest filters (`name::`).

Function calls are not followed: a test is threaded only if its own body contains `thread::`.

Usage: threaded_tests.py [path]
"""

import re
import sys

path = sys.argv[1] if len(sys.argv) > 1 else "tests/hazarc.rs"

threaded = {}
current = None
with open(path, encoding="utf-8") as file:
    for line in file:
        # top-level functions only (tests are never nested)
        if m := re.match(r"fn (\w+)", line):
            current = m.group(1)
            threaded[current] = False
        elif current and "thread::" in line:
            threaded[current] = True

print(" ".join(f"{name}::" for name, t in threaded.items() if t))
