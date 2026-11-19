# Design and recovery

## A profile is a set of links

The repository owns source files. The target directory owns links to their
absolute paths. A tab-separated profile makes the mapping explicit and permits
spaces without a shell parser. Configuration content is never read by the
planner; it checks source types, destination types and link identity.

The reusable library borrows the manifest buffer and validates all mappings.
It rejects traversal, ambiguous components and overlapping destinations.
The executable opens parent directories with `openat` and `O_NOFOLLOW`.
This prevents a pre-existing symlink parent from redirecting an operation.

## Deployment

The planner inspects every source and destination before any deployment writes.
A conflict stops the operation unless `--backup` was requested. The process
then takes a nonblocking, target-wide `flock`, checks the destinations again,
and creates a journal.

A journal namespace includes the canonical repository path and profile path.
A separate owner record verifies both values, so a namespace hash collision
does not grant ownership of another deployment. Explicit state stores also bind
the canonical target path; another target cannot reuse the same journal. The saved profile remains
available if the current profile changes or is deleted.

Originals move into numbered backup slots using `renameat2(RENAME_NOREPLACE)`.
The state store and originals must therefore share a filesystem; a cross-device
move fails without silently copying or deleting an original.
A new symlink never overwrites a newly appearing destination. Successful file
and directory mutations are synchronized to disk. When an ordinary system call
fails, links created in that attempt are removed, originals are moved back, and
new empty parent directories are cleaned up.

## Removal and recovery

Removal preflights the entire saved deployment. A matching absolute symlink can
be removed. A missing target can receive its preserved original. A replaced
target causes removal to stop before changing other mappings. Incomplete
journals also permit untouched originals to remain in place.

If apply was interrupted, first inspect `zdot plan` and the destinations.
Run `zdot remove --root REPO --target TARGET --profile PROFILE` to remove
known links and restore recorded originals, then apply again. If a destination
has been replaced, move that replacement to a chosen safe location before
retrying removal; zdot does not choose which version to discard.

The repository path and selected profile name identify the journal. Keep that
repository directory available for removal. Changing the profile content does
not lose the saved deployment; moving the repository requires removing and
redeploying its links.

Default mode retains an empty `.zdot-state` and the small `.zdot.lock` after
removal. An explicit `--state-dir` retains only its selected empty store. New
versions lock the target directory itself, so different state directories do
not create independent locks for the same target. Default mode also takes the
legacy lock; explicit mode honors it if it already exists. No process remains
running.

The explicit state path is opened one component at a time with `O_NOFOLLOW`.
Its final directory, journals and backup directories must be owned by the
current effective user and inaccessible to group and other users. Parent
directories are created with mode `0700` only during apply. The state path
cannot contain the target or overlap the repository or any mapping. Plan and
status check those conditions without creating directories. Removal validates
the saved profile against the same state boundaries.

A stable state path is part of a deployment's identity. Changing this argument
does not migrate existing journals. Remove the old deployment with its original
state path before choosing a new one. All clients of an explicit store must
support `--state-dir`; an old binary does not know that store's recovery data.

## Transaction boundary

The lock coordinates zdot processes, not arbitrary editors or Git hooks.
Other programs should not rename parent directories or replace metadata while
an operation is running. The journal enables recovery after process
interruption; it is not a claim of whole-filesystem atomicity under every power
failure. A corrupt or foreign journal causes a refusal.

Source repositories are trusted configuration. Installing a directory link
does not recursively audit its contents or sandbox programs that use those
contents. Git hooks and credential helpers run under the user's Git
configuration only when the user explicitly calls sync.

## Runtime budget

The production dependency tree is `zdot → zclap` with zclap's default features
disabled. Core parsing uses no allocator. The Linux executable defines its
entry point and a small syscall boundary, and links neither Rust `std` nor libc.
Build scripts, unit tests and Python developer tools may use their normal
standard libraries; they are not part of the installed binary.

Buffers have explicit limits: 64 KiB for a profile, 128 mappings, 4096 bytes per
path and 64 components per relative path. Invalid or oversized inputs are
rejected, rather than growing memory usage without a bound.

## Evidence and review

| Claim | Evidence |
| --- | --- |
| Preview does not write | Default and nested external-store tests compare paths before and after plan/status |
| Reapplication is idempotent | Two applications report 1 then 0 changes per new mapping |
| Conflicts preserve user data | File, directory, broken-link and replaced-target tests |
| Symlink parents cannot redirect deployment | Source and destination parent fixtures |
| Rollback works for an ordinary filesystem failure | A read-only destination forces failure after earlier links and backups |
| Recovery uses saved state | Missing current profile and interrupted-backup tests for both layouts |
| External state cannot target another deployment | Target owner, path containment, private-mode and symlink tests |
| External state does not create home metadata | Roundtrip checks absence of top-level `.zdot-state` and `.zdot.lock` |
| Different stores cooperate on one target | Directory flock and legacy-lock tests |
| Cross-filesystem backups fail without data loss | Two-filesystem fixture verifies rollback after a failed move |
| Git synchronization is real | Temporary local bare remote, push, pull and divergence rejection |
| No dynamic runtime library is linked | `scripts/check_binary.py` inspects ELF headers and Cargo features |

Review questions: Is the purpose narrow enough to verify? Does each command have
one meaning? Are failure tests representative? Are limits stated where users
need them? Does recovery preserve existing data? These are checked by
`make check` and the cases above. Performance claims remain measurements with
a stated workload; this project does not claim a universal speed ranking.
