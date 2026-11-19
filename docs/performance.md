# Performance

The latest x86_64 measurement used Rust 1.100.0 and the release build of zdot
0.2.0 on 2026-11-20 (UTC+08). Other development jobs were active on the host.
Raw inputs, timing scope and binary/source hashes are in
[linux-x86_64-0.2.0.json](benchmarks/linux-x86_64-0.2.0.json).
The [0.1.1 sample](benchmarks/linux-x86_64.json) remains available; these
separate runs do not isolate version overhead from changing host load.

| Measurement | Result | Scope |
| --- | ---: | --- |
| Measured executable | 58,344 bytes (56.98 KiB) | Stripped static ELF; no libc or dynamic loader |
| Version command, median | 0.275 ms | 50 warm runs including Python parent spawn and wait |
| Version command, p95 | 0.357 ms | Same 50-run sample |
| Default JSON plan, median | 7.472 ms | 128 valid mappings with long names; 50 warm runs |
| Default JSON plan, p95 | 12.220 ms | Same plan workload |
| External-state JSON plan, median | 6.725 ms | Same mappings; nested state path absent |
| External-state JSON plan, p95 | 11.441 ms | Same external-state workload |
| Sampled RSS | 236 KiB | 128-entry plan blocked on its output pipe, 3 samples |
| Sampled PSS | 232 KiB | Same sample; shared pages apportioned |

The output pipe is reduced to 4096 bytes so the process stays alive at a known
point during the memory read. The measurement reads `/proc/PID/smaps_rollup`;
it is a live resident-page sample, not a whole-run maximum. The larger buffers
used during apply and rollback can produce a different peak.

There is no idle zdot service. Each command exits, so no dotfile synchronization
daemon adds background memory. The `sync` command replaces zdot with the
installed Git; Git's memory and network behavior are outside the figures above.

These numbers describe one machine, one compiler and three workloads. They do
not establish a ranking against other dotfile managers. Reproduce the
measurement with `make benchmark`; its JSON output includes the complete
measurement scope and checksum of the measured binary.
