# zdot 0.2.0

This release adds explicit private recovery storage for desktop configuration suites.
The installed executable still uses no Rust standard library, allocator, libc,
resident process or background network activity.

- Add --state-dir to plan, apply, status and remove; default layout stays compatible.
- Keep originals and saved profiles under the selected 0700 directory.
- Include the target in explicit-store owner records and state_dir in JSON plans.
- Refuse symlink state paths, unsafe permissions and overlap with managed files.
- Lock the target directory so different explicit stores coordinate without adding
  a top-level lock file; honor existing legacy locks.
- Verify read-only planning, idempotence, saved-profile removal, interrupted recovery
  and rollback after cross-filesystem and permission failures.

Local release gates cover 4 parser tests, 25 CLI integration cases, formatting,
Clippy and static ELF/dependency inspection. Original files remain available for
restoration. See README and docs/design.md for the stable-path and same-filesystem
requirements.

## zdot 0.1.1

Initial Linux release of the standalone dotfile manager used by oh-my-ztty.

- Read-only text and JSON plans; bounded no_std profile parser.
- Idempotent symlink deployment, explicit backups and original restoration.
- Refusal of traversal, overlapping destinations, source symlinks and symlink parents.
- Saved deployment profiles, cooperative locking and interrupted-apply recovery.
- Explicit Git status, fast-forward pull and push using the user's existing Git setup.
- Static Linux x86_64 and aarch64 executables, manual and zsh completion.

See the README for the filesystem transaction boundary and current input limits.
No claim of universal performance superiority is made.

The initial v0.1.0 tag did not publish binaries. Release validation found that LLD retained an ELF interpreter and that ARM64 needs its own Linux open flags. Version 0.1.1 explicitly removes the interpreter and uses the verified architecture-specific flags.
