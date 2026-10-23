#!/usr/bin/env python3
"""Install only while the destination app is idle; never unlink retired code."""
from pathlib import Path
import argparse
import hashlib
import json
import os
import plistlib
import shlex
import subprocess
import sys
import uuid


class InstallError(RuntimeError):
    pass


def running_processes():
    # comm is the executable path, not argv: no terminal command lines collected.
    result = subprocess.run(["/bin/ps", "-axww", "-o", "pid=,comm="],
                            check=True, capture_output=True, text=True)
    rows = []
    for line in result.stdout.splitlines():
        fields = line.strip().split(None, 1)
        if len(fields) == 2 and fields[0].isdigit():
            rows.append((int(fields[0]), fields[1]))
    if not rows:
        raise InstallError("Cannot inspect running applications; installation stopped")
    return rows


def active_pids(app, rows):
    app = app.resolve()
    prefix = str(app) + os.sep
    result = []
    for pid, executable in rows:
        path = Path(executable)
        if not path.is_absolute():
            # An incomplete process path cannot establish that Schnauz is idle.
            if path.name == "schnauz":
                result.append(pid)
        elif str(path.resolve()).startswith(prefix):
            result.append(pid)
    return result


def verify(app):
    if app.is_symlink() or not app.is_dir():
        raise InstallError("The staged bundle must be a real directory")
    with (app / "Contents/Info.plist").open("rb") as stream:
        info = plistlib.load(stream)
    if info.get("CFBundleIdentifier") != "local.schnauz.app":
        raise InstallError("Unexpected staged app identity")
    subprocess.run(["/usr/bin/codesign", "--verify", "--deep", "--strict", str(app)], check=True)


def pending_path(destination, cache):
    key = hashlib.sha256(os.fsencode(str(destination.resolve()))).hexdigest()[:16]
    return cache / "pending-apps" / key / "Schnauz.app"


def install(source, destination, cache, inventory=running_processes, verify_app=verify):
    source, destination, cache = source.resolve(), destination.resolve(), cache.resolve()
    if source == destination or source in destination.parents or destination in source.parents:
        raise InstallError("Staged and installed apps must be separate")
    verify_app(source)
    rows = inventory()  # Failure must leave both bundles untouched.
    if active_pids(source, rows):
        raise InstallError("The staged app is running; installation stopped")
    if active_pids(destination, rows):
        pending = pending_path(destination, cache)
        pending.parent.mkdir(parents=True, exist_ok=True)
        if source != pending:
            if pending.exists():
                if active_pids(pending, rows):
                    raise InstallError("The pending app is running; it was preserved")
                # Retain even superseded candidates rather than unlinking code
                # that could have been launched between the scan and this move.
                retain(pending, cache)
            source.rename(pending)
        return "deferred", pending
    backup = retain(destination, cache) if destination.exists() else None
    try:
        destination.parent.mkdir(parents=True, exist_ok=True)
        source.rename(destination)
    except OSError:
        if backup is not None:
            backup.rename(destination)
        raise
    # Retained bundles live outside the temporary packaging directory. Do not
    # delete them here: a launch may have raced with our process-list snapshot.
    return "installed", destination


def retain(app, cache):
    backup = cache / "retained-apps" / uuid.uuid4().hex / "Schnauz.app"
    backup.parent.mkdir(parents=True, exist_ok=True)
    app.rename(backup)
    return backup


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("destination", type=Path)
    parser.add_argument("--source", type=Path, help="A verified build; omit to apply the pending update")
    args = parser.parse_args()
    cache = Path(__file__).resolve().parent.parent / ".build-cache"
    source = args.source or pending_path(args.destination, cache)
    status, path = install(source, args.destination, cache)
    if status == "deferred":
        print(f"Update ready at {path}. Running Schnauz and its app bundle were preserved.")
        command = ["python3", str(Path(__file__).resolve()), str(args.destination.resolve())]
        print("When you choose to quit Schnauz, apply it with: " + shlex.join(command))
        print("No app restart, terminal closure or privacy change was performed.")
    else:
        print(f"Installed {path}; the previous bundle was retained for rollback. No app was launched.")


if __name__ == "__main__":
    try:
        main()
    except (InstallError, OSError, ValueError, subprocess.SubprocessError) as error:
        sys.exit(f"Installation stopped: {error}")
