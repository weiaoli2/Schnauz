#!/usr/bin/env python3
"""Disposable lifecycle regressions; never touch the user's running app or TCC."""
from pathlib import Path
import subprocess
import tempfile
import time
import unittest
from unittest.mock import patch

import install_bundle as installer


class InstallTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="schnauz-install-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.dest = self.root / "folder with spaces" / "Schnauz.app"
        self.source = self.root / "staging" / "Schnauz.app"
        self.cache = self.root / "cache"
        for app, value in [(self.dest, "old"), (self.source, "new")]:
            app.mkdir(parents=True)
            (app / "version").write_text(value)
        self.verify = lambda app: None

    def install(self, rows):
        return installer.install(self.source, self.dest, self.cache,
                                 inventory=lambda: rows, verify_app=self.verify)

    def test_live_bundle_never_moves_and_pending_update_applies_only_after_exit(self):
        inode = self.dest.stat().st_ino
        rows = [(42, str(self.dest / "Contents/MacOS/schnauz"))]
        status, pending = self.install(rows)
        self.assertEqual(status, "deferred")
        self.assertEqual(self.dest.stat().st_ino, inode)
        self.assertEqual((self.dest / "version").read_text(), "old")
        status, _ = installer.install(pending, self.dest, self.cache,
                                     inventory=lambda: rows, verify_app=self.verify)
        self.assertEqual(status, "deferred")
        status, _ = installer.install(pending, self.dest, self.cache,
                                     inventory=lambda: [], verify_app=self.verify)
        self.assertEqual(status, "installed")
        self.assertEqual((self.dest / "version").read_text(), "new")
        backups = list((self.cache / "retained-apps").glob("*/Schnauz.app"))
        self.assertEqual(backups[0].stat().st_ino, inode)

    def test_failed_process_inspection_or_signature_preserves_both_apps(self):
        for keyword in ["inventory", "verify_app"]:
            def fail(*args):
                raise installer.InstallError("fixture failure")
            kwargs = dict(inventory=lambda: [], verify_app=self.verify)
            kwargs[keyword] = fail
            with self.assertRaises(installer.InstallError):
                installer.install(self.source, self.dest, self.cache, **kwargs)
            self.assertEqual((self.dest / "version").read_text(), "old")
            self.assertEqual((self.source / "version").read_text(), "new")

    def test_failed_install_restores_previous_bundle(self):
        rename = Path.rename
        def fail_source(path, target):
            if path == self.source:
                raise OSError("fixture install failure")
            return rename(path, target)
        with patch.object(Path, "rename", fail_source):
            with self.assertRaises(OSError):
                self.install([])
        self.assertEqual((self.dest / "version").read_text(), "old")

    def test_unrelated_app_does_not_block_and_symlink_alias_is_detected(self):
        alias = self.root / "alias"
        alias.symlink_to(self.dest.parent, target_is_directory=True)
        self.assertEqual(installer.active_pids(self.dest, [(42, str(alias / "Schnauz.app/Contents/MacOS/schnauz"))]), [42])
        self.assertEqual(installer.active_pids(self.dest, [(43, str(self.dest) + ".other/Contents/MacOS/schnauz")]), [])
        self.assertEqual(installer.active_pids(self.dest, [(44, "schnauz")]), [44])
        self.assertEqual(self.install([(43, "/elsewhere/Schnauz.app/Contents/MacOS/schnauz")])[0], "installed")

    def test_actual_owned_executable_is_preserved_with_same_pid_and_path(self):
        binary = self.dest / "Contents/MacOS/schnauz"
        binary.parent.mkdir(parents=True)
        subprocess.run(["/usr/bin/xcrun", "clang", "-x", "c", "-", "-o", str(binary)],
                       input="#include <unistd.h>\nint main(void) { sleep(30); return 0; }\n",
                       text=True, capture_output=True, check=True)
        process = subprocess.Popen([str(binary)])
        try:
            for _ in range(100):
                if process.pid in installer.active_pids(self.dest, installer.running_processes()):
                    break
                time.sleep(0.01)
            else:
                self.fail("Owned fixture process was not detected")
            inode = binary.stat().st_ino
            status, _ = installer.install(self.source, self.dest, self.cache, verify_app=self.verify)
            self.assertEqual(status, "deferred")
            self.assertIsNone(process.poll())
            self.assertEqual(binary.stat().st_ino, inode)
            self.assertIn(process.pid, installer.active_pids(self.dest, installer.running_processes()))
        finally:
            process.terminate()
            process.wait(timeout=5)


if __name__ == "__main__":
    unittest.main()
