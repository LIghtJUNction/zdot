# zdot 0.1.1

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
