# zdot

Small, explicit dotfile deployments and Git synchronization.

zdot links files from a repository into your home directory. You preview the
changes, apply them, and can restore any originals you chose to back up.
The native Linux executable uses Rust `no_std`, Project Z's
[zclap](https://github.com/LIghtJUNction/zclap), and direct system calls.
It has no allocator, libc dependency, daemon, or background network activity.

## Install

Download a matching archive and `SHA256SUMS` from
[Releases](https://github.com/LIghtJUNction/zdot/releases), verify the checksum,
and extract it into `~/.local`. The archive contains `bin/zdot`, its manual,
completion, and license. Linux x86_64 and aarch64 are supported.

From source, install Rust 1.100 and a C-compatible linker, then:

```sh
make check
make install PREFIX="$HOME/.local"
```

The linker produces a static ELF; no C library is linked.
Use `make uninstall PREFIX="$HOME/.local"` to remove the installed program,
manual and completion. Remove deployments separately with `zdot remove`.

## Use

```sh
zdot init --root "$HOME/dotfiles"
cp "$HOME/.zshrc" "$HOME/dotfiles/files/zshrc"
printf 'files/zshrc\t.zshrc\n' > "$HOME/dotfiles/profile.zdot"

zdot plan --root "$HOME/dotfiles" --target "$HOME" --json
zdot apply --root "$HOME/dotfiles" --target "$HOME" --backup
zdot status --root "$HOME/dotfiles" --target "$HOME"
zdot remove --root "$HOME/dotfiles" --target "$HOME"
```

Each profile line has `source<TAB>destination`. Paths are relative to the
repository and target respectively. Files, whole directories, spaces and
Unicode names work. Empty lines and `#` comments are ignored.

`plan` only reads. `apply` refuses existing files unless `--backup` is explicit.
Repeated application keeps matching links. `remove` uses the saved deployment
profile, restores originals, and refuses to remove a destination you replaced.
Edits made through a managed symlink edit the repository's source file.

## Sync

Initialize your repository and choose its remote using normal Git commands.
`zdot init` creates the file layout; it does not run Git or configure an account.

```sh
git -C "$HOME/dotfiles" init
git -C "$HOME/dotfiles" add profile.zdot files
git -C "$HOME/dotfiles" commit -m "Add chosen dotfiles"
git -C "$HOME/dotfiles" remote add origin YOUR_REMOTE

zdot sync --root "$HOME/dotfiles" status
zdot sync --root "$HOME/dotfiles" pull
zdot sync --root "$HOME/dotfiles" push
```

Sync uses the installed Git at `/usr/bin/git` or `/bin/git`.
Pull only fast-forwards. Commit creation, remote selection, merge resolution
and credentials stay with Git. A pull does not automatically deploy a changed
profile. No shell is used to construct Git arguments.

## Boundaries

- Up to 128 mappings and 64 KiB per profile; limits fail before deployment.
- Parent symlinks, source symlinks, traversal, overlapping destinations and
  source/destination containment are rejected.
- Change a profile by removing its previous deployment, editing it, then applying
  it again. Keep the repository at its original path until removal.
- Metadata stays in the target's `.zdot-state` directory. `.zdot.lock` serializes
  cooperating zdot processes. These are never uploaded by zdot.
- Ordinary apply failures roll back newly created links and moved originals.
  An interrupted deployment retains its journal for `remove` recovery.
  Concurrent edits by other programs are outside this transaction model.
- Linux 4.11 or later and mounted `/proc` are required.

[Design and recovery](docs/design.md) ·
[Performance measurements](docs/performance.md) ·
[Contributing](CONTRIBUTING.md) ·
[oh-my-ztty](https://github.com/LIghtJUNction/oh-my-ztty)

MIT.
