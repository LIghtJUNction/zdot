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

## Dependency and consumer propagation

For a base-library update used by zdot or a zdot release affecting consumers, enumerate direct and transitive consumers with the Project Z graph. Include repository-level runtime and installer edges by running `python3 scripts/project_z.py impact --repositories zdot` in the coordinator checkout. Register every new downstream and its required-repository edge.

Coordinate each affected checkout with its existing owner. Update applicable dependency versions, source pins, lockfiles, bundled binaries, install scripts and package checksums. Validate the consumer behavior locally, commit and push the changes, and retain the exact tested commit and outcome. Close each affected consumer with a successful result, or record a concrete blocker, its owner and the next action.

Known consumers are oh-my-ztty, which bundles zdot and depends on its profile/state contract, and the Project Z coordinator snapshot. Changes to apply, rollback, state ownership or remove semantics require downstream lifecycle validation as well as the local filesystem tests. Work already owned by another maintainer must be coordinated with that owner.
