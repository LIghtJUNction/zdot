#!/usr/bin/env python3
"""Reject accidental dynamic linking or extra production dependencies."""
import pathlib
import subprocess
import sys
binary = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else "target/release/zdot")
headers = subprocess.check_output(["readelf", "-l", str(binary)], text=True)
assert "INTERP" not in headers, "unexpected dynamic loader"
dynamic = subprocess.check_output(["readelf", "-d", str(binary)], text=True)
assert "(NEEDED)" not in dynamic, "unexpected dynamic library"
tree = subprocess.check_output(["cargo", "tree", "--locked", "-e", "normal,features"], text=True)
assert 'feature "std"' not in tree, tree
assert set(line.strip(" │├└─").split()[0] for line in tree.splitlines() if " v" in line) <= {"zdot", "zclap"}, tree
print(f"Static ELF verified: {binary.stat().st_size} bytes; production dependency: zclap (core only).")
