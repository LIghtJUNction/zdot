#!/usr/bin/env python3
"""Create a deterministic standard-prefix Linux release archive."""
from pathlib import Path
import argparse
import gzip
import io
import os
import tarfile
import tomllib

ROOT = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("target", choices=("x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu"))
parser.add_argument("--binary", type=Path, default=ROOT / "target/release/zdot")
args = parser.parse_args()
version = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]["version"]
dist = ROOT / "dist"
dist.mkdir(exist_ok=True)
output = dist / f"zdot-v{version}-{args.target}.tar.gz"
files = [
    (args.binary, "bin/zdot", 0o755),
    (ROOT / "packaging/man/zdot.1", "share/man/man1/zdot.1", 0o644),
    (ROOT / "completions/_zdot", "share/zsh/site-functions/_zdot", 0o644),
    (ROOT / "README.md", "share/doc/zdot/README.md", 0o644),
    (ROOT / "LICENSE", "share/licenses/zdot/LICENSE", 0o644),
]
with output.open("wb") as raw, gzip.GzipFile(fileobj=raw, mode="wb", filename="", mtime=0) as gz:
    with tarfile.open(fileobj=gz, mode="w") as archive:
        for source, name, mode in files:
            data = source.read_bytes()
            entry = tarfile.TarInfo(name)
            entry.size = len(data)
            entry.mode = mode
            entry.mtime = 0
            archive.addfile(entry, io.BytesIO(data))
print(output.name)
