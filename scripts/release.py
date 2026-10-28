#!/usr/bin/env python3
"""Build macOS archives and publish a verified, signed Zeph update manifest."""
import argparse
import base64
import json
import os
from pathlib import Path
import plistlib
import re
import shutil
import subprocess
import tarfile
import tempfile
import tomllib
from urllib.parse import quote

ROOT = Path(__file__).resolve().parent.parent


def version(value):
    if not re.fullmatch(r"\d+\.\d+\.\d+(?:-dev\.\d+)?", value):
        raise ValueError("Expected X.Y.Z or X.Y.Z-dev.N")
    expected = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]["version"]
    if value != expected:
        raise ValueError(f"Tag {value} does not match Cargo.toml version {expected}")
    return value


def package(output, ver, arch, binary):
    version(ver)
    if arch not in ("aarch64", "x86_64"):
        raise ValueError("Unsupported macOS architecture")
    output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory() as temporary:
        app = Path(temporary) / "Zeph.app"
        executable = app / "Contents/MacOS/zeph"
        executable.parent.mkdir(parents=True)
        shutil.copy2(binary, executable)
        executable.chmod(0o755)
        (app / "Contents/Info.plist").write_bytes(plistlib.dumps({
            "CFBundleExecutable": "zeph", "CFBundleIdentifier": "dev.zeph.workbench",
            "CFBundleName": "Zeph", "CFBundleDisplayName": "Zeph", "CFBundlePackageType": "APPL",
            "CFBundleVersion": ver.split("-")[0], "CFBundleShortVersionString": ver.split("-")[0],
            "NSHighResolutionCapable": True, "LSMinimumSystemVersion": "12.0",
        }))
        subprocess.run(["codesign", "--force", "--deep", "--sign", "-", str(app)], check=True)
        archive = output / f"Zeph_{ver}_macos-{arch}.app.tar.gz"
        with tarfile.open(archive, "w:gz") as tar:
            tar.add(app, arcname="Zeph.app")
        return archive


def sign(path, key, public):
    signature = path.with_name(path.name + ".minisig")
    subprocess.run(["minisign", "-S", "-s", str(key), "-m", str(path), "-x", str(signature)], check=True)
    subprocess.run(["minisign", "-V", "-p", str(public), "-m", str(path), "-x", str(signature)], check=True)
    path.with_name(path.name + ".sig").write_text(base64.b64encode(signature.read_bytes()).decode() + "\n")
    signature.unlink()


def signed_manifest(directory, ver, key):
    version(ver)
    platforms = {}
    with tempfile.TemporaryDirectory() as temporary:
        public = Path(temporary) / "update.pub"
        public.write_bytes(base64.b64decode((ROOT / "assets/packaging/update-public-key").read_text().strip(), validate=True))
        for arch in ("aarch64", "x86_64"):
            name = f"Zeph_{ver}_macos-{arch}.app.tar.gz"
            matches = list(directory.rglob(name))
            if len(matches) != 1:
                raise ValueError(f"Expected exactly one {arch} archive")
            archive = matches[0]
            if not 0 < archive.stat().st_size <= 2 * 1024**3:
                raise ValueError("Invalid update package size")
            sign(archive, key, public)
            platforms[f"macos-{arch}-app"] = {
                "url": f"https://github.com/yoogoc/zeph/releases/download/v{ver}/{quote(name)}",
                "signature": archive.with_name(name + ".sig").read_text().strip(),
                "size": archive.stat().st_size, "format": "app",
            }
        manifest = directory / "update.json"
        manifest.write_text(json.dumps({"version": ver, "platforms": platforms}, indent=2) + "\n")
        sign(manifest, key, public)
    return manifest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    p = sub.add_parser("package")
    p.add_argument("--output", type=Path, required=True)
    p.add_argument("--version", required=True)
    p.add_argument("--arch", required=True)
    p.add_argument("--binary", type=Path, default=ROOT / "target/release/zeph")
    s = sub.add_parser("sign")
    s.add_argument("--directory", type=Path, required=True)
    s.add_argument("--version", required=True)
    s.add_argument("--key", type=Path, required=True)
    args = parser.parse_args()
    if args.command == "package":
        package(args.output, args.version, args.arch, args.binary)
    else:
        signed_manifest(args.directory, args.version, args.key)


if __name__ == "__main__":
    main()
