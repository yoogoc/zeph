#!/usr/bin/env python3
"""Build native distributions and publish a verified, signed update manifest."""
import argparse
import base64
import copy
import hashlib
import json
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

PLATFORMS = tuple(f"{system}-{arch}" for system in ("macos", "windows", "linux") for arch in ("aarch64", "x86_64"))


def distribution_files(ver, platform):
    if platform not in PLATFORMS:
        raise ValueError("Unsupported distribution platform")
    prefix = f"Zeph_{ver}_{platform}"
    if platform.startswith("macos-"):
        return [(prefix + ".app.tar.gz", "app"), (prefix + ".dmg", None)]
    if platform.startswith("windows-"):
        return [(prefix + "-setup.exe", "nsis")]
    return [(prefix + ".deb", "deb"), (prefix + ".AppImage", "appimage")]


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
        resources = app / "Contents/Resources"
        resources.mkdir()
        shutil.copy2(ROOT / "assets/app-icon/zeph.icns", resources / "zeph.icns")
        (app / "Contents/Info.plist").write_bytes(plistlib.dumps({
            "CFBundleExecutable": "zeph", "CFBundleIdentifier": "dev.zeph.workbench",
            "CFBundleName": "Zeph", "CFBundleDisplayName": "Zeph", "CFBundlePackageType": "APPL",
            "CFBundleVersion": ver.split("-")[0], "CFBundleShortVersionString": ver.split("-")[0],
            "CFBundleIconFile": "zeph.icns",
            "NSHighResolutionCapable": True, "LSMinimumSystemVersion": "12.0",
        }))
        subprocess.run(["codesign", "--force", "--deep", "--sign", "-", str(app)], check=True)
        archive = output / f"Zeph_{ver}_macos-{arch}.app.tar.gz"
        with tarfile.open(archive, "w:gz") as tar:
            tar.add(app, arcname="Zeph.app")
        # The DMG and updater archive contain the same signed application.
        (Path(temporary) / "Applications").symlink_to("/Applications")
        subprocess.run(["hdiutil", "create", "-volname", "Zeph", "-srcfolder", temporary,
                        "-ov", "-format", "UDZO", str(output.resolve() / f"Zeph_{ver}_macos-{arch}.dmg")], check=True)
        return archive


def native_config(output, ver, platform, binary, package_format):
    version(ver)
    if platform not in PLATFORMS or platform.startswith("macos-"):
        raise ValueError("Expected a Windows or Linux platform")
    system, arch = platform.split("-")
    target = f"{arch}-{'pc-windows-msvc' if system == 'windows' else 'unknown-linux-gnu'}"
    metadata = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]["metadata"]["packager"]
    config = copy.deepcopy(metadata)
    config.update({"name": "zeph", "version": ver.replace("-dev.", "~dev.") if package_format == "deb" else ver,
                   "target-triple": target, "out-dir": str(output.resolve()), "binaries-dir": str(binary.resolve().parent)})
    config["icons"] = [str(ROOT / icon) for icon in config["icons"]]
    return config


def native_package(output, ver, platform, binary):
    files = distribution_files(ver, platform)
    if platform.startswith("macos-"):
        return package(output, ver, platform.split("-")[1], binary)
    output.mkdir(parents=True, exist_ok=True)
    # Isolated packager output avoids accidentally publishing stale distributions.
    with tempfile.TemporaryDirectory() as temporary:
        staging = Path(temporary)
        for name, kind in files:
            config = json.dumps(native_config(staging, ver, platform, binary, kind))
            subprocess.run(["cargo", "packager", "--config", config, "--formats", kind], cwd=ROOT, check=True)
            suffix = {"nsis": "*setup.exe", "deb": "*.deb", "appimage": "*.AppImage"}[kind]
            matches = list(staging.glob(suffix))
            if len(matches) != 1:
                raise ValueError(f"Expected exactly one {kind} distribution, found {len(matches)}")
            shutil.copy2(matches[0], output / name)


def sign(path, key, public):
    signature = path.with_name(path.name + ".minisig")
    subprocess.run(["minisign", "-S", "-s", str(key), "-m", str(path), "-x", str(signature)], check=True)
    subprocess.run(["minisign", "-V", "-p", str(public), "-m", str(path), "-x", str(signature)], check=True)
    path.with_name(path.name + ".sig").write_text(base64.b64encode(signature.read_bytes()).decode() + "\n")
    signature.unlink()


def sha256(path):
    with path.open("rb") as file:
        return hashlib.file_digest(file, "sha256").hexdigest()


def release_notes(ver):
    version(ver)
    lines = [f"Zeph {ver}", "", "选择对应系统与 CPU 架构的安装包：", "",
             "| 系统 | 架构 | 下载 |", "| --- | --- | --- |"]
    for platform in PLATFORMS:
        system, arch = platform.split("-")
        links = []
        for name, kind in distribution_files(ver, platform):
            if kind == "app":
                continue  # The archive is for automatic updates; offer DMG to users.
            label = {None: "DMG", "nsis": "安装器 EXE", "deb": "DEB", "appimage": "AppImage"}[kind]
            links.append(f"[{label}](https://github.com/yoogoc/zeph/releases/download/v{ver}/{quote(name)})")
        lines.append(f"| {system} | {'ARM64 / Apple Silicon' if platform == 'macos-aarch64' else 'ARM64' if arch == 'aarch64' else 'x64 / Intel' if system == 'macos' else 'x64'} | {' / '.join(links)} |")
    lines.extend(["", "macOS：打开 DMG，将 Zeph 拖入 Applications。Windows：运行安装器。Linux：安装 DEB，或给 AppImage 添加执行权限后运行。",
                  "", "附有 SHA256SUMS、各安装包的 Minisign 签名和自动更新清单。macOS 应用为 ad-hoc 签名，尚未进行 Apple Developer ID 公证；Windows 安装器尚无 Authenticode 签名。Linux 基于 Ubuntu 24.04 构建，需要兼容的系统运行库及图形驱动。"])
    return "\n".join(lines) + "\n"


def signed_manifest(directory, ver, key):
    version(ver)
    platforms = {}
    with tempfile.TemporaryDirectory() as temporary:
        public = Path(temporary) / "update.pub"
        public.write_bytes(base64.b64decode((ROOT / "assets/packaging/update-public-key").read_text().strip(), validate=True))
        packages = []
        for platform in PLATFORMS:
            for name, kind in distribution_files(ver, platform):
                matches = list(directory.rglob(name))
                if len(matches) != 1:
                    raise ValueError(f"Expected exactly one {name}")
                packages.append((platform, name, kind, matches[0]))
        for platform, name, kind, archive in packages:
            if not 0 < archive.stat().st_size <= 2 * 1024**3:
                raise ValueError("Invalid update package size")
            sign(archive, key, public)
            if kind is None:
                continue
            platforms[f"{platform}-{kind}"] = {
                "url": f"https://github.com/yoogoc/zeph/releases/download/v{ver}/{quote(name)}",
                "signature": archive.with_name(name + ".sig").read_text().strip(),
                "size": archive.stat().st_size, "format": kind,
            }
        checksums = directory / "SHA256SUMS"
        checksums.write_text("".join(f"{sha256(path)}  {name}\n"
                                    for _, name, _, path in packages))
        sign(checksums, key, public)
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
    n = sub.add_parser("native")
    n.add_argument("--output", type=Path, required=True)
    n.add_argument("--version", required=True)
    n.add_argument("--platform", choices=PLATFORMS, required=True)
    n.add_argument("--binary", type=Path, required=True)
    s = sub.add_parser("sign")
    s.add_argument("--directory", type=Path, required=True)
    s.add_argument("--version", required=True)
    s.add_argument("--key", type=Path, required=True)
    notes = sub.add_parser("notes")
    notes.add_argument("--version", required=True)
    notes.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.command == "package":
        package(args.output, args.version, args.arch, args.binary)
    elif args.command == "native":
        native_package(args.output, args.version, args.platform, args.binary)
    elif args.command == "notes":
        args.output.write_text(release_notes(args.version), encoding="utf-8")
    else:
        signed_manifest(args.directory, args.version, args.key)


if __name__ == "__main__":
    main()
