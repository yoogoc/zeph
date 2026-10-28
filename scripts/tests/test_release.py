import base64
import importlib.util
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("release", Path(__file__).parents[1] / "release.py")
release = importlib.util.module_from_spec(spec)
spec.loader.exec_module(release)


class ReleaseTests(unittest.TestCase):
    def test_version_must_match_the_built_application(self):
        current = release.tomllib.loads((release.ROOT / "Cargo.toml").read_text())["package"]["version"]
        self.assertEqual(release.version(current), current)
        for value in ("v" + current, "0.0.0", "../../something", "1.0.0; echo no"):
            with self.assertRaises(ValueError):
                release.version(value)

    def test_both_architectures_are_required(self):
        current = release.tomllib.loads((release.ROOT / "Cargo.toml").read_text())["package"]["version"]
        with tempfile.TemporaryDirectory() as temp:
            with self.assertRaises(ValueError):
                release.signed_manifest(Path(temp), current, Path(temp) / "missing.key")

    @unittest.skipUnless(shutil.which("minisign"), "minisign is needed for signing tests")
    def test_manifest_and_both_archives_are_signed_with_the_embedded_key(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / "Cargo.toml").write_text('[package]\nversion="1.2.3"\n')
            keys = root / "assets/packaging"
            keys.mkdir(parents=True)
            key = root / "private.key"
            pub = root / "public.key"
            subprocess.run(["minisign", "-G", "-W", "-s", str(key), "-p", str(pub)], check=True, capture_output=True)
            (keys / "update-public-key").write_text(base64.b64encode(pub.read_bytes()).decode())
            dist = root / "dist"
            dist.mkdir()
            for platform in release.PLATFORMS:
                for name, _ in release.distribution_files("1.2.3", platform):
                    (dist / name).write_bytes(b"test-package-" + name.encode())
            with patch.object(release, "ROOT", root):
                manifest = release.signed_manifest(dist, "1.2.3", key)
            value = json.loads(manifest.read_text())
            self.assertEqual(len(value["platforms"]), 8)
            self.assertEqual(len((dist / "SHA256SUMS").read_text().splitlines()), 10)
            for platform, asset in value["platforms"].items():
                self.assertTrue(platform.endswith("-" + asset["format"]))
                self.assertIn("/v1.2.3/", asset["url"])
            for signed in [path.with_suffix("") for path in dist.glob("*.sig")]:
                signature = root / "signature.txt"
                signature.write_bytes(base64.b64decode(signed.with_name(signed.name + ".sig").read_text().strip(), validate=True))
                subprocess.run(["minisign", "-V", "-p", str(pub), "-m", str(signed), "-x", str(signature)], check=True, capture_output=True)
                signed.write_bytes(b"tampered")
                self.assertNotEqual(subprocess.run(["minisign", "-V", "-p", str(pub), "-m", str(signed), "-x", str(signature)], capture_output=True).returncode, 0)

    def test_native_configs_use_absolute_paths_and_debian_version_ordering(self):
        current = release.tomllib.loads((release.ROOT / "Cargo.toml").read_text())["package"]["version"]
        for platform in ("linux-x86_64", "linux-aarch64", "windows-x86_64", "windows-aarch64"):
            with self.subTest(platform=platform):
                config = release.native_config(Path("dist"), current, platform, Path("target/release/zeph"), "deb" if platform.startswith("linux") else "nsis")
                self.assertTrue(Path(config["binaries-dir"]).is_absolute())
                self.assertTrue(all(Path(icon).is_file() for icon in config["icons"]))
                self.assertIn(platform.split("-")[1], config["target-triple"])
        with patch.object(release, "version"):
            config = release.native_config(Path("dist"), "1.2.3-dev.4", "linux-x86_64", Path("zeph"), "deb")
        self.assertEqual(config["version"], "1.2.3~dev.4")

    def test_partial_distribution_sets_are_rejected_before_signing(self):
        current = release.tomllib.loads((release.ROOT / "Cargo.toml").read_text())["package"]["version"]
        with tempfile.TemporaryDirectory() as temp, patch.object(release, "sign") as signer:
            dist = Path(temp)
            for platform in release.PLATFORMS[:-1]:
                for name, _ in release.distribution_files(current, platform):
                    (dist / name).write_bytes(b"package")
            with self.assertRaises(ValueError):
                release.signed_manifest(dist, current, dist / "unused-key")
            signer.assert_not_called()

    def test_release_notes_offer_installers_for_every_platform(self):
        current = release.tomllib.loads((release.ROOT / "Cargo.toml").read_text())["package"]["version"]
        notes = release.release_notes(current)
        self.assertEqual(notes.count("| macos |"), 2)
        self.assertEqual(notes.count("| windows |"), 2)
        self.assertEqual(notes.count("| linux |"), 2)
        self.assertNotIn(".app.tar.gz", notes)
        for platform in release.PLATFORMS:
            for name, kind in release.distribution_files(current, platform):
                if kind != "app":
                    self.assertIn(name, notes)
