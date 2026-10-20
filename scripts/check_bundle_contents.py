#!/usr/bin/env python3
"""Keep private runtime state outside distributable Schnauz app bundles."""
from pathlib import Path
import sys
import tempfile

EXECUTABLES = {"schnauz", "removebg", "cloudctl", "volumectl", "nextcloudctl", "todoistctl", "weblinkctl", "networkctl"}
LICENSES = {"Shuffle-MIT.txt", "gpui-terminal-MIT.txt", "gpui-terminal-Apache-2.0.txt", "Alacritty-Apache-2.0.txt", "portable-pty-MIT.txt", "THIRD_PARTY_NOTICES.txt"}
REQUIRED = {"Contents/Info.plist", "Contents/Resources/AppIcon.icns", "Contents/Resources/Credits.txt"}
REQUIRED |= {f"Contents/MacOS/{name}" for name in EXECUTABLES}
REQUIRED |= {f"Contents/Resources/Licenses/{name}" for name in LICENSES}
OPTIONAL = {"Contents/_CodeSignature/CodeResources"}
DIRECTORIES = {str(parent) for name in REQUIRED | OPTIONAL for parent in Path(name).parents if str(parent) != "."}


def check(app: Path) -> None:
    if app.is_symlink() or not app.is_dir():
        raise ValueError("The app must be a real directory")
    seen = set()
    for path in app.rglob("*"):
        relative = path.relative_to(app).as_posix()
        if path.is_symlink():
            raise ValueError(f"Unexpected bundle symlink: {relative}")
        if path.is_dir():
            if relative not in DIRECTORIES:
                raise ValueError(f"Unexpected bundle directory: {relative}")
        elif path.is_file() and relative in REQUIRED | OPTIONAL:
            seen.add(relative)
        else:
            raise ValueError(f"Unexpected bundle file: {relative}")
    if REQUIRED - seen:
        raise ValueError("Missing required app files: " + ", ".join(sorted(REQUIRED - seen)))


def self_test() -> None:
    with tempfile.TemporaryDirectory(prefix="schnauz-bundle-fixture-") as directory:
        app = Path(directory) / "Schnauz.app"
        for name in REQUIRED:
            path = app / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(b"fixture")
        check(app)
        for name in ["Contents/Resources/projects.json", "Contents/MacOS/.env", "Contents/Resources/session.json", "Contents/Resources/login.keychain-db"]:
            unexpected = app / name
            unexpected.write_bytes(b"synthetic private fixture")
            try:
                check(app)
            except ValueError:
                pass
            else:
                raise AssertionError("Private fixture accepted")
            unexpected.unlink()
        symlink = app / "Contents/Resources/Profile"
        symlink.symlink_to(Path(directory))
        try:
            check(app)
        except ValueError:
            pass
        else:
            raise AssertionError("External profile symlink accepted")
    print("PASS bundle allowlist rejects private settings, sessions, credentials and external profile links; disposable fixtures only")


if __name__ == "__main__":
    try:
        if sys.argv[1:] == ["--self-test"]:
            self_test()
        elif len(sys.argv) == 2:
            check(Path(sys.argv[1]))
            print("PASS distributable bundle contains only expected executables and resources")
        else:
            raise ValueError("Usage: check_bundle_contents.py APP | --self-test")
    except ValueError as error:
        sys.exit(str(error))
