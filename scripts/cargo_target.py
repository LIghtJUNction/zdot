"""Locate artifacts using Cargo's configured target directory."""
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def target_directory():
    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--locked", "--no-deps", "--format-version=1"],
        cwd=ROOT, text=True))
    return Path(metadata["target_directory"])


def release_binary():
    return target_directory() / "release" / "zdot"


if __name__ == "__main__":
    print(target_directory())
