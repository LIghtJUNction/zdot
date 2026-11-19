#!/usr/bin/env python3
"""Behavioral tests use disposable repositories and target directories only."""
from pathlib import Path
import fcntl
import json
import os
import shutil
import subprocess
import tempfile
import unittest

BINARY = Path(os.environ.get("ZDOT_BIN", "target/release/zdot")).resolve()

class Dotfiles(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="zdot-tests-")
        self.base = Path(self.temporary.name)
        self.repo = self.base / "repo"
        self.home = self.base / "home"
        self.home.mkdir()
        self.env = dict(os.environ, HOME=str(self.home), GIT_CONFIG_NOSYSTEM="1",
                        GIT_CONFIG_GLOBAL="/dev/null", GIT_TERMINAL_PROMPT="0")
        self.run_cli("init", target=False)
        (self.repo / "files/a").write_text("managed a\n")
        (self.repo / "files/b").write_text("managed b\n")

    def tearDown(self):
        for path in self.base.rglob("*"):
            if path.is_dir() and not path.is_symlink():
                path.chmod(0o755)
        self.temporary.cleanup()

    def profile(self, content):
        (self.repo / "profile.zdot").write_text(content)

    def run_cli(self, command, *args, expected=0, target=True):
        argv = [str(BINARY), command, "--root", str(self.repo)]
        if target:
            argv += ["--target", str(self.home)]
        completed = subprocess.run(argv + list(args), text=True, capture_output=True, env=self.env)
        self.assertEqual(completed.returncode, expected,
                         f"{argv + list(args)}\nstdout: {completed.stdout}\nstderr: {completed.stderr}")
        return completed

    def journal(self):
        return next((self.home / ".zdot-state").iterdir())

    def test_init_never_replaces_profile(self):
        original = (self.repo / "profile.zdot").read_bytes()
        self.run_cli("init", target=False, expected=2)
        self.assertEqual((self.repo / "profile.zdot").read_bytes(), original)

    def test_plan_is_read_only_and_roundtrip_is_idempotent(self):
        name = 'hello world 你好 "quoted"'
        (self.repo / "files" / name).write_text("unicode\n")
        (self.repo / "files/dir").mkdir()
        (self.repo / "files/dir/inside").write_text("directory\n")
        self.profile(f"files/{name}\t.config/{name}\nfiles/dir\t.theme\n")
        data = json.loads(self.run_cli("plan", "--json").stdout)
        self.assertEqual(data["schema"], "zdot.plan.v1")
        self.assertEqual([e["action"] for e in data["entries"]], ["link", "link"])
        self.assertEqual(list(self.home.iterdir()), [])
        self.run_cli("status", expected=3)
        first = json.loads(self.run_cli("apply", "--json").stdout)
        second = json.loads(self.run_cli("apply", "--json").stdout)
        self.assertEqual(first["changed"], 2)
        self.assertEqual(second["changed"], 0)
        self.assertEqual((self.home / ".config" / name).read_text(), "unicode\n")
        self.assertEqual((self.home / ".theme/inside").read_text(), "directory\n")
        self.run_cli("status")
        self.assertEqual(json.loads(self.run_cli("remove", "--json").stdout)["changed"], 2)
        self.assertEqual(json.loads(self.run_cli("remove", "--json").stdout)["changed"], 0)
        self.assertTrue((self.repo / "files" / name).is_file())

    def test_all_conflicts_preflight_before_any_write(self):
        self.profile("files/a\t.config/a\nfiles/b\t.b\n")
        (self.home / ".b").write_text("original\n")
        before = {p.name for p in self.home.iterdir()}
        self.run_cli("apply", expected=2)
        self.assertEqual({p.name for p in self.home.iterdir()}, before)
        self.assertEqual((self.home / ".b").read_text(), "original\n")

    def test_backup_and_restore_file_directory_and_broken_link(self):
        (self.repo / "files/dir").mkdir()
        (self.home / ".b").mkdir()
        (self.home / ".b/old").write_text("directory original")
        (self.home / ".a").write_text("file original")
        (self.home / ".a").chmod(0o640)
        (self.home / ".c").symlink_to("missing-target")
        self.profile("files/a\t.a\nfiles/dir\t.b\nfiles/b\t.c\n")
        self.run_cli("apply", "--backup")
        self.run_cli("apply", "--backup")
        self.run_cli("remove")
        self.assertEqual((self.home / ".a").read_text(), "file original")
        self.assertEqual((self.home / ".a").stat().st_mode & 0o777, 0o640)
        self.assertEqual((self.home / ".b/old").read_text(), "directory original")
        self.assertTrue((self.home / ".c").is_symlink())
        self.assertEqual(os.readlink(self.home / ".c"), "missing-target")
        self.assertEqual(list((self.home / ".zdot-state").iterdir()), [])

    def test_replaced_destination_prevents_all_removal(self):
        self.profile("files/a\t.a\nfiles/b\t.b\n")
        (self.home / ".b").write_text("preserved original")
        self.run_cli("apply", "--backup")
        (self.home / ".b").unlink()
        (self.home / ".b").write_text("new local content")
        self.run_cli("remove", expected=2)
        self.assertTrue((self.home / ".a").is_symlink())
        self.assertEqual((self.home / ".b").read_text(), "new local content")
        self.assertEqual((self.journal() / "backups/001").read_text(), "preserved original")

    def test_refuses_symlink_source_and_source_parent(self):
        (self.repo / "files/link").symlink_to(self.repo / "files/a")
        self.profile("files/link\t.a\n")
        self.run_cli("apply", expected=2)
        (self.repo / "alias").symlink_to(self.repo / "files", target_is_directory=True)
        self.profile("alias/a\t.a\n")
        self.run_cli("apply", expected=2)
        self.assertEqual(list(self.home.iterdir()), [])

    def test_refuses_symlink_target_parent(self):
        outside = self.base / "outside"
        outside.mkdir()
        (outside / "sentinel").write_text("outside")
        (self.home / ".config").symlink_to(outside, target_is_directory=True)
        self.profile("files/a\t.config/a\n")
        self.run_cli("apply", "--backup", expected=2)
        self.assertEqual(list(outside.iterdir()), [outside / "sentinel"])
        self.assertTrue((self.home / ".config").is_symlink())

    def test_invalid_profiles_fail_before_writing(self):
        for manifest in ["../escape\t.a", "files/a\t../escape",
                         "files/a\t.a\nfiles/b\t.a/nested",
                         "files/a\t.zdot-state/override",
                         "files/a\t.a\nmissing\t.b",
                         "\n".join(f"files/a\t.file-{i}" for i in range(129))]:
            with self.subTest(manifest=manifest[:100]):
                self.profile(manifest)
                self.run_cli("apply", expected=2)
                self.assertEqual(list(self.home.iterdir()), [])

    def test_manifest_change_requires_remove_and_saved_profile_survives(self):
        self.profile("files/a\t.a\n")
        self.run_cli("apply")
        self.profile("files/b\t.b\n")
        self.run_cli("apply", expected=2)
        (self.repo / "profile.zdot").unlink()
        self.run_cli("remove")
        self.assertFalse((self.home / ".a").exists())
        self.assertFalse((self.home / ".b").exists())

    def test_shell_metacharacters_are_literal_paths(self):
        name = "literal $(touch BAD) `echo BAD` ; name"
        (self.repo / "files" / name).write_text("safe")
        self.profile(f"files/{name}\t{name}\n")
        self.run_cli("apply")
        self.assertEqual((self.home / name).read_text(), "safe")
        self.assertFalse((self.home / "BAD").exists())
        self.run_cli("remove")

    @unittest.skipIf(os.geteuid() == 0, "permission-failure scenario needs an unprivileged user")
    def test_apply_rolls_back_after_real_filesystem_failure(self):
        readonly = self.home / "readonly"
        readonly.mkdir(mode=0o555)
        (self.home / ".a").write_text("original")
        self.profile("files/a\t.a\nfiles/b\t.new/deep/b\nfiles/b\treadonly/b\n")
        self.run_cli("apply", "--backup", expected=2)
        self.assertEqual((self.home / ".a").read_text(), "original")
        self.assertFalse((self.home / ".a").is_symlink())
        self.assertFalse((self.home / ".new").exists())
        self.assertFalse((readonly / "b").exists())
        self.assertEqual(list((self.home / ".zdot-state").iterdir()), [])

    def test_interrupted_backup_can_be_recovered(self):
        self.profile("files/a\t.a\n")
        (self.home / ".a").write_text("original")
        self.run_cli("apply", "--backup")
        journal = self.journal()
        (journal / "complete").unlink()
        (self.home / ".a").unlink()
        self.run_cli("apply", expected=2)
        self.run_cli("remove")
        self.assertEqual((self.home / ".a").read_text(), "original")

    def test_cooperative_lock_refuses_concurrent_mutation(self):
        self.profile("files/a\t.a\n")
        with (self.home / ".zdot.lock").open("w") as handle:
            fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
            self.run_cli("apply", expected=2)
        self.assertFalse((self.home / ".a").exists())
        self.assertFalse((self.home / ".zdot-state").exists())

    def test_source_destination_self_reference_is_refused(self):
        self.profile("files/a\tfiles/a\n")
        result = subprocess.run([str(BINARY), "apply", "--root", str(self.repo),
                                 "--target", str(self.repo), "--backup"],
                                text=True, capture_output=True, env=self.env)
        self.assertEqual(result.returncode, 2, result)
        self.assertEqual((self.repo / "files/a").read_text(), "managed a\n")
        self.assertFalse((self.repo / "files/a").is_symlink())

    def test_sync_uses_real_git_with_local_bare_remote(self):
        git = shutil.which("git")
        self.assertIsNotNone(git)
        remote = self.base / "remote.git"
        peer = self.base / "peer"
        def run(*args, cwd=None):
            result = subprocess.run([git, *args], cwd=cwd, env=self.env, text=True, capture_output=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            return result.stdout.strip()
        run("init", "--bare", "--initial-branch=main", str(remote))
        run("init", "--initial-branch=main", str(self.repo))
        run("config", "user.name", "zdot test", cwd=self.repo)
        run("config", "user.email", "test@example.invalid", cwd=self.repo)
        run("add", ".", cwd=self.repo)
        run("commit", "-m", "initial", cwd=self.repo)
        run("remote", "add", "origin", str(remote), cwd=self.repo)
        run("push", "-u", "origin", "main", cwd=self.repo)
        (self.repo / "files/a").write_text("second")
        run("commit", "-am", "second", cwd=self.repo)
        self.run_cli("sync", "push", target=False)
        self.assertEqual(run("rev-parse", "HEAD", cwd=self.repo),
                         run("rev-parse", "refs/heads/main", cwd=remote))
        run("clone", str(remote), str(peer))
        run("config", "user.name", "zdot test", cwd=peer)
        run("config", "user.email", "test@example.invalid", cwd=peer)
        (peer / "files/a").write_text("third")
        run("commit", "-am", "third", cwd=peer)
        run("push", cwd=peer)
        self.run_cli("sync", "pull", target=False)
        self.assertEqual((self.repo / "files/a").read_text(), "third")
        self.run_cli("sync", "status", target=False)
        # A real divergence is rejected by --ff-only, never merged silently.
        (self.repo / "files/a").write_text("local branch")
        run("commit", "-am", "local", cwd=self.repo)
        (peer / "files/b").write_text("remote branch")
        run("commit", "-am", "remote", cwd=peer)
        run("push", cwd=peer)
        result = subprocess.run([str(BINARY), "sync", "--root", str(self.repo), "pull"],
                                env=self.env, text=True, capture_output=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual((self.repo / "files/a").read_text(), "local branch")

if __name__ == "__main__":
    unittest.main(verbosity=2)
