#!/usr/bin/env python3
"""Merge verified thin apps, then sign every universal helper and the bundle."""
from pathlib import Path
import argparse
import plistlib
import shutil
import subprocess
import tempfile

from check_bundle_contents import check, EXECUTABLES


def run(*args):
    return subprocess.check_output(args, text=True, stderr=subprocess.STDOUT).strip()


def architectures(path):
    return set(run("lipo", "-archs", str(path)).split())


def authority(path):
    return [line for line in run("codesign", "-dvv", str(path)).splitlines()
            if line.startswith("Authority=")]


def merge(arm, intel, destination, identity):
    if destination.exists() or destination.is_symlink():
        raise ValueError("Universal destination already exists; preserve it and use a new output path")
    for app, expected in [(arm, {"arm64"}), (intel, {"x86_64"})]:
        check(app)
        run("codesign", "--verify", "--deep", "--strict", str(app))
        for name in EXECUTABLES:
            if architectures(app / "Contents/MacOS" / name) != expected:
                raise ValueError(f"Unexpected source architecture: {name}")
    if authority(arm) != authority(intel) or not authority(arm):
        raise ValueError("Both source bundles must use the same certificate authority")
    arm_info = plistlib.loads((arm / "Contents/Info.plist").read_bytes())
    intel_info = plistlib.loads((intel / "Contents/Info.plist").read_bytes())
    if arm_info != intel_info or arm_info.get("CFBundleIdentifier") != "local.schnauz.app":
        raise ValueError("Source app versions or identities differ")
    for path in arm.rglob("*"):
        relative = path.relative_to(arm)
        if path.is_file() and "MacOS" not in relative.parts and "_CodeSignature" not in relative.parts:
            if path.read_bytes() != (intel / relative).read_bytes():
                raise ValueError(f"Architecture resources differ: {relative}")
    destination.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="universal-stage-", dir=destination.parent) as directory:
        staged = Path(directory) / "Schnauz.app"
        shutil.copytree(arm, staged)
        for name in sorted(EXECUTABLES):
            output = staged / "Contents/MacOS" / name
            run("lipo", "-create", str(arm / "Contents/MacOS" / name),
                str(intel / "Contents/MacOS" / name), "-output", str(output))
            run("codesign", "--force", "--timestamp=none", "--sign", identity, str(output))
            if architectures(output) != {"arm64", "x86_64"}:
                raise ValueError(f"Incomplete universal executable: {name}")
        run("codesign", "--force", "--timestamp=none", "--sign", identity,
            "--identifier", "local.schnauz.app", str(staged))
        run("codesign", "--verify", "--deep", "--strict", str(staged))
        check(staged)
        if authority(staged) != authority(arm):
            raise ValueError("Universal app signer differs from the source app signer")
        staged.rename(destination)
    print(f"Verified universal app: {destination}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("arm", type=Path)
    parser.add_argument("intel", type=Path)
    parser.add_argument("destination", type=Path)
    parser.add_argument("--identity", required=True)
    args = parser.parse_args()
    merge(args.arm, args.intel, args.destination, args.identity)
