#!/usr/bin/env python3
"""Update the package version, optionally validate and publish its release tag."""
import argparse
from pathlib import Path
import re
import subprocess
import tomllib

ROOT = Path(__file__).resolve().parents[1]

def run(*args, capture=False):
    return subprocess.run(args, cwd=ROOT, check=True, text=True,
                          stdout=subprocess.PIPE if capture else None).stdout

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("part", choices=("patch", "minor", "major"))
parser.add_argument("--release", action="store_true")
args = parser.parse_args()
if args.release:
    if run("git", "status", "--porcelain", capture=True).strip():
        parser.error("--release requires a clean worktree")
    if run("git", "branch", "--show-current", capture=True).strip() != "main":
        parser.error("--release requires the main branch")

manifest = ROOT / "Cargo.toml"
text = manifest.read_text()
current = tomllib.loads(text)["package"]["version"]
parts = [int(x) for x in current.split(".")]
index = {"major": 0, "minor": 1, "patch": 2}[args.part]
parts[index] += 1
for i in range(index + 1, 3):
    parts[i] = 0
version = ".".join(map(str, parts))
tag = f"v{version}"
if args.release:
    present = subprocess.run(["git", "rev-parse", "--verify", f"refs/tags/{tag}"],
                             cwd=ROOT, capture_output=True).returncode == 0
    if present:
        parser.error(f"tag {tag} already exists")

text, count = re.subn(r'(?m)^version = "' + re.escape(current) + r'"$',
                     f'version = "{version}"', text, count=1)
assert count == 1
manifest.write_text(text)
run("cargo", "generate-lockfile", "--offline")
print(f"{current} -> {version}")
if args.release:
    run("make", "check")
    run("git", "add", "Cargo.toml", "Cargo.lock")
    run("git", "commit", "-m", f"chore(release): {tag}")
    run("git", "tag", "-a", tag, "-m", f"zdot {tag}")
    run("git", "push", "--atomic", "origin", "HEAD:main", f"refs/tags/{tag}")
