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
            for arch in ("aarch64", "x86_64"):
                (dist / f"Zeph_1.2.3_macos-{arch}.app.tar.gz").write_bytes(b"test-archive-" + arch.encode())
            with patch.object(release, "ROOT", root):
                manifest = release.signed_manifest(dist, "1.2.3", key)
            value = json.loads(manifest.read_text())
            self.assertEqual(len(value["platforms"]), 2)
            for signed in [manifest, *dist.glob("*.app.tar.gz")]:
                signature = root / "signature.txt"
                signature.write_bytes(base64.b64decode(signed.with_name(signed.name + ".sig").read_text().strip(), validate=True))
                subprocess.run(["minisign", "-V", "-p", str(pub), "-m", str(signed), "-x", str(signature)], check=True, capture_output=True)
                signed.write_bytes(b"tampered")
                self.assertNotEqual(subprocess.run(["minisign", "-V", "-p", str(pub), "-m", str(signed), "-x", str(signature)], capture_output=True).returncode, 0)
