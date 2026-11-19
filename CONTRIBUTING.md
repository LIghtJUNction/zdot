# Contributing

Run `make check` before committing. It formats no files: it checks formatting,
runs library tests, builds the native binary, applies strict Clippy, exercises
temporary filesystem and Git scenarios, and checks the ELF dependency boundary.

`make benchmark` records startup, plan time and a sampled process memory value
with the workload in the output. Keep performance evidence separate from a
claim about all machines or all competing tools.

## Release

```sh
python3 scripts/bump.py patch
make check
git add Cargo.toml Cargo.lock
git commit -m "chore(release): bump version"
```

Or run `python3 scripts/bump.py patch --release` from a clean main branch.
This updates both Cargo files, runs the local gate, commits, tags and atomically
pushes the commit and tag. `minor` and `major` work the same way.

Tagging `vVERSION` runs the official release workflow on native x86_64 and
aarch64 Linux runners. Each runner validates the tagged version, builds, tests
and packages a static ELF. The publishing job emits tar archives and a combined
SHA256SUMS file.

Routine validation is local. The check workflow is manually dispatched; it does
not run on every commit or pull request. The tag workflow is reserved for
official release artifacts.
