# Performance

The latest x86_64 measurement used Rust 1.100.0 and the release build of zdot
0.1.1 on 2026-11-20 (UTC+08). Raw values, input scope and binary/source hashes
are in [linux-x86_64.json](benchmarks/linux-x86_64.json).

| Measurement | Result | Scope |
| --- | ---: | --- |
| Measured executable | 53,776 bytes (52.52 KiB) | Stripped static ELF; no libc or dynamic loader |
| Version command, median | 0.241 ms | 50 warm runs including Python parent spawn and wait |
| Version command, p95 | 0.307 ms | Same 50-run sample |
| JSON plan, median | 5.222 ms | 128 valid mappings with long names; 50 warm runs |
| JSON plan, p95 | 9.332 ms | Same plan workload |
| Sampled RSS | 232 KiB | 128-entry plan blocked on its output pipe, 3 samples |
| Sampled PSS | 228 KiB | Same sample; shared pages apportioned |

The output pipe is reduced to 4096 bytes so the process stays alive at a known
point during the memory read. The measurement reads `/proc/PID/smaps_rollup`;
it is a live resident-page sample, not a whole-run maximum. The larger buffers
used during apply and rollback can produce a different peak.

There is no idle zdot service. Each command exits, so no dotfile synchronization
daemon adds background memory. The `sync` command replaces zdot with the
installed Git; Git's memory and network behavior are outside the figures above.

These numbers describe one machine, one compiler and two workloads. They do
not establish a ranking against other dotfile managers. Reproduce the
measurement with `make benchmark`; its JSON output includes the complete
measurement scope and checksum of the measured binary.
