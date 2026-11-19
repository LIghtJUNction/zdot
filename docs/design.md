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
does not grant ownership of another deployment. The saved profile remains
available if the current profile changes or is deleted.

Originals move into numbered backup slots using `renameat2(RENAME_NOREPLACE)`.
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

The target retains an empty `.zdot-state` and the small `.zdot.lock` after
removal. Keeping the lock inode prevents two cooperating processes from locking
different files during concurrent operations. No process remains running.

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
| Preview does not write | Integration test compares an empty target before and after plan |
| Reapplication is idempotent | Two applications report 1 then 0 changes per new mapping |
| Conflicts preserve user data | File, directory, broken-link and replaced-target tests |
| Symlink parents cannot redirect deployment | Source and destination parent fixtures |
| Rollback works for an ordinary filesystem failure | A read-only destination forces failure after earlier links and backups |
| Recovery uses saved state | Missing current profile and interrupted-backup tests |
| Git synchronization is real | Temporary local bare remote, push, pull and divergence rejection |
| No dynamic runtime library is linked | `scripts/check_binary.py` inspects ELF headers and Cargo features |

Review questions: Is the purpose narrow enough to verify? Does each command have
one meaning? Are failure tests representative? Are limits stated where users
need them? Does recovery preserve existing data? These are checked by
`make check` and the cases above. Performance claims remain measurements with
a stated workload; this project does not claim a universal speed ranking.
