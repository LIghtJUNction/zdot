#!/usr/bin/env python3
"""Measure wall time and a declared, blocked-output memory sample; no rankings."""
import argparse
from datetime import datetime, timezone
import fcntl
import hashlib
import json
import os
from pathlib import Path
import platform
import statistics
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--runs", type=int, default=50)
parser.add_argument("--build-note", default="Local release build; consult toolchain configuration")
parser.add_argument("--output", type=Path, default=ROOT / "bench-results.json")
args = parser.parse_args()
if not 5 <= args.runs <= 1000:
    parser.error("--runs must be between 5 and 1000")
binary = ROOT / "target/release/zdot"

def timings(argv):
    for _ in range(3):
        subprocess.run(argv, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
    samples = []
    for _ in range(args.runs):
        start = time.perf_counter_ns()
        subprocess.run(argv, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
        samples.append((time.perf_counter_ns() - start) / 1_000_000)
    samples.sort()
    return {"runs": len(samples), "median_ms": statistics.median(samples),
            "p95_ms": samples[min(len(samples) - 1, int(len(samples) * .95))],
            "minimum_ms": min(samples)}

with tempfile.TemporaryDirectory(prefix="zdot-benchmark-") as temporary:
    base = Path(temporary)
    repo = base / "repo"
    target = base / "target"
    repo.mkdir()
    target.mkdir()
    (repo / "files").mkdir()
    # Long but valid names keep a 128-entry JSON plan larger than the pipe.
    source = "files/" + "a" * 230
    (repo / source).write_text("benchmark input\n")
    (repo / "profile.zdot").write_text("".join(
        source + "\t" + f"{i:03}-" + "b" * 230 + "\n" for i in range(128)
    ))
    command = [str(binary), "plan", "--root", str(repo), "--target", str(target), "--json"]
    measurements = {
        "schema": "zdot.benchmark.v1",
        "measured_at": datetime.now(timezone.utc).isoformat(),
        "architecture": platform.machine(),
        "build_note": args.build_note,
        "rustc": subprocess.check_output(["rustc", "--version"], cwd=ROOT, text=True).strip(),
        "version": subprocess.check_output([str(binary), "--version"], text=True).strip(),
        "binary_bytes": binary.stat().st_size,
        "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
        "wall_time_scope": "Python parent spawn, native execution and wait; warm filesystem cache",
        "version_wall": timings([str(binary), "--version"]),
        "plan_128_wall": timings(command),
        "memory_scope": "128-entry JSON plan blocked on a 4096-byte stdout pipe; /proc smaps_rollup, not an idle daemon or worst-case bound",
    }
    memory = []
    for _ in range(3):
        process = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        fcntl.fcntl(process.stdout.fileno(), fcntl.F_SETPIPE_SZ, 4096)
        time.sleep(.02)
        if process.poll() is not None:
            raise RuntimeError("plan exited before memory sampling")
        values = {}
        for line in Path(f"/proc/{process.pid}/smaps_rollup").read_text().splitlines():
            if line.startswith(("Rss:", "Pss:")):
                key, number, unit = line.split()
                assert unit == "kB"
                values[key.rstrip(":").lower() + "_kib"] = int(number)
        memory.append(values)
        stdout, stderr = process.communicate(timeout=10)
        assert process.returncode == 0, stderr
        assert len(json.loads(stdout)["entries"]) == 128
    measurements["memory_samples"] = memory
    measurements["sampled_rss_kib_median"] = statistics.median(v["rss_kib"] for v in memory)
    measurements["sampled_pss_kib_median"] = statistics.median(v["pss_kib"] for v in memory)
    source_hash = hashlib.sha256()
    for relative in ("Cargo.toml", "Cargo.lock", "build.rs", "src/lib.rs", "src/linux.rs", "src/main.rs"):
        source_hash.update(relative.encode() + b"\0" + (ROOT / relative).read_bytes())
    measurements["source_manifest_sha256"] = source_hash.hexdigest()

args.output.parent.mkdir(parents=True, exist_ok=True)
args.output.write_text(json.dumps(measurements, indent=2) + "\n")
print(json.dumps(measurements, indent=2))
