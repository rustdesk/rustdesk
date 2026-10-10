import base64
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from cryptography.hazmat.primitives import serialization
from cryptography.hazmat.primitives.asymmetric import ed25519

SCRIPT = Path(__file__).with_name("generate_update_metadata.py")
OFFICIAL_RELEASE_BASE_URL = "https://github.com/rustdesk/rustdesk/releases/download"


class GenerateUpdateMetadataTest(unittest.TestCase):
    def setUp(self):
        self.temp_dir = tempfile.TemporaryDirectory()
        self.root = Path(self.temp_dir.name)
        private_key = ed25519.Ed25519PrivateKey.generate()
        seed = private_key.private_bytes(
            serialization.Encoding.Raw,
            serialization.PrivateFormat.Raw,
            serialization.NoEncryption(),
        )
        public_key = private_key.public_key().public_bytes(
            serialization.Encoding.Raw,
            serialization.PublicFormat.Raw,
        )
        self.seed = base64.b64encode(seed).decode("ascii")
        self.public_key = base64.b64encode(public_key).decode("ascii")

    def tearDown(self):
        self.temp_dir.cleanup()

    def run_script(self, *args, seed=None, public_key=None):
        env = os.environ.copy()
        env.pop("RUSTDESK_UPDATE_ED25519_SEED", None)
        env.pop("RUSTDESK_UPDATE_ED25519_PUBLIC_KEY", None)
        if seed is not None:
            env["RUSTDESK_UPDATE_ED25519_SEED"] = seed
        if public_key is not None:
            env["RUSTDESK_UPDATE_ED25519_PUBLIC_KEY"] = public_key
        return subprocess.run(
            [sys.executable, str(SCRIPT), *args],
            cwd=SCRIPT.parents[2],
            env=env,
            text=True,
            capture_output=True,
        )

    def artifact(self, name="rustdesk-1.4.6-x86_64.exe", data=b"rustdesk"):
        path = self.root / name
        path.write_bytes(data)
        return path

    def sign(
        self,
        artifacts,
        *,
        version="1.4.6",
        release_id="v1.4.6",
        package_id="rustdesk",
        release_base_url=OFFICIAL_RELEASE_BASE_URL,
        seed=None,
    ):
        metadata = self.root / "rustdesk-update.json"
        signature = self.root / "rustdesk-update.json.sig"
        args = ["sign"]
        for platform, arch, file_format, path in artifacts:
            args.extend(["--artifact", platform, arch, file_format, str(path)])
        args.extend(
            [
                "--version",
                version,
                "--release-id",
                release_id,
                "--package-id",
                package_id,
                "--release-base-url",
                release_base_url,
                "--published-at",
                "2026-05-14T00:00:00Z",
                "--metadata-out",
                str(metadata),
                "--signature-out",
                str(signature),
            ]
        )
        result = self.run_script(*args, seed=self.seed if seed is None else seed)
        return metadata, signature, result

    def verify(
        self,
        metadata,
        signature,
        artifacts,
        *,
        public_key=None,
        package_id="rustdesk",
        release_base_url=OFFICIAL_RELEASE_BASE_URL,
    ):
        args = [
            "verify",
            "--metadata",
            str(metadata),
            "--signature",
            str(signature),
            "--version",
            "1.4.6",
            "--release-id",
            "v1.4.6",
            "--package-id",
            package_id,
            "--release-base-url",
            release_base_url,
        ]
        for artifact in artifacts:
            args.extend(["--artifact", str(artifact)])
        return self.run_script(
            *args,
            public_key=public_key or self.public_key,
        )

    def rust_source(self, public_key=None):
        key_bytes = base64.b64decode(public_key or self.public_key)
        source = self.root / "update_metadata.rs"
        source.write_text(
            'TrustedUpdateKey { key_id: "2026-ed25519-main", public_key: ['
            + ",".join(str(byte) for byte in key_bytes)
            + "] }",
            encoding="utf-8",
        )
        return source

    def test_signs_and_verifies_release_artifacts(self):
        exe = self.artifact()
        dmg = self.artifact("rustdesk-1.4.6-aarch64.dmg", b"dmg")
        specs = [("windows", "x86_64", "exe", exe), ("macos", "aarch64", "dmg", dmg)]

        metadata, signature, signed = self.sign(specs)
        verified = self.verify(metadata, signature, [exe, dmg])

        self.assertEqual(signed.returncode, 0, signed.stderr)
        self.assertEqual(verified.returncode, 0, verified.stderr)
        data = json.loads(metadata.read_text(encoding="utf-8"))
        self.assertEqual(data["release_id"], "v1.4.6")
        self.assertEqual({item["file_name"] for item in data["artifacts"]}, {exe.name, dmg.name})

    def test_verification_rejects_tampering(self):
        artifact = self.artifact()
        metadata, signature, _ = self.sign([("windows", "x86_64", "exe", artifact)])
        artifact.write_bytes(b"tampered")
        self.assertNotEqual(self.verify(metadata, signature, [artifact]).returncode, 0)

        artifact.write_bytes(b"rustdesk")
        data = json.loads(metadata.read_text(encoding="utf-8"))
        data["published_at"] = "2026-05-15T00:00:00Z"
        metadata.write_text(json.dumps(data), encoding="utf-8")
        self.assertNotEqual(self.verify(metadata, signature, [artifact]).returncode, 0)

    def test_verification_rejects_wrong_public_key(self):
        artifact = self.artifact()
        metadata, signature, _ = self.sign([("windows", "x86_64", "exe", artifact)])
        wrong_key = base64.b64encode(b"x" * 32).decode("ascii")
        self.assertNotEqual(
            self.verify(
                metadata,
                signature,
                [artifact],
                public_key=wrong_key,
            ).returncode,
            0,
        )

    def test_sign_rejects_invalid_release_inputs(self):
        artifact = self.artifact()
        spec = [("windows", "x86_64", "exe", artifact)]
        self.assertNotEqual(self.sign(spec, version="1.4.7")[2].returncode, 0)
        for release_id in ("bad/tag", "bad\tid", "bad%zz"):
            self.assertNotEqual(self.sign(spec, release_id=release_id)[2].returncode, 0)
        self.assertNotEqual(self.sign(spec, seed="invalid")[2].returncode, 0)
        duplicate = spec + [("windows", "x86_64", "exe", artifact)]
        self.assertNotEqual(self.sign(duplicate)[2].returncode, 0)

    def test_signs_and_verifies_custom_release_identity(self):
        package_id = "com.example.rustdesk-custom"
        release_base_url = "https://updates.example.com/releases/download"
        artifact = self.artifact("rustdesk-custom-1.4.6-aarch64.dmg", b"dmg")

        metadata, signature, signed = self.sign(
            [("macos", "aarch64", "dmg", artifact)],
            package_id=package_id,
            release_base_url=release_base_url,
        )
        verified = self.verify(
            metadata,
            signature,
            [artifact],
            package_id=package_id,
            release_base_url=release_base_url,
        )

        self.assertEqual(signed.returncode, 0, signed.stderr)
        self.assertEqual(verified.returncode, 0, verified.stderr)
        data = json.loads(metadata.read_text(encoding="utf-8"))
        self.assertEqual(data["package_id"], package_id)
        self.assertEqual(
            data["artifacts"][0]["url"],
            f"{release_base_url}/v1.4.6/{artifact.name}",
        )

    def test_rejects_unsafe_custom_release_policy(self):
        artifact = self.artifact("rustdesk-custom-1.4.6-aarch64.dmg", b"dmg")
        spec = [("macos", "aarch64", "dmg", artifact)]

        for package_id in ("", "com.example/custom", "com.example custom"):
            with self.subTest(package_id=package_id):
                self.assertNotEqual(
                    self.sign(spec, package_id=package_id)[2].returncode,
                    0,
                )
        for release_base_url in (
            "http://updates.example.com/releases",
            "https://user@updates.example.com/releases",
            "https://updates.example.com/releases/",
            "https://updates.example.com/releases?channel=stable",
            "https://updates.example.com/relea\nses",
        ):
            with self.subTest(release_base_url=release_base_url):
                self.assertNotEqual(
                    self.sign(spec, release_base_url=release_base_url)[2].returncode,
                    0,
                )

    def test_checks_embedded_public_key(self):
        source = self.rust_source()
        result = self.run_script(
            "check-key",
            "--rust-source",
            str(source),
            seed=self.seed,
            public_key=self.public_key,
        )
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_check_key_rejects_invalid_or_mismatched_seed(self):
        source = self.rust_source()
        invalid_seeds = [
            None,
            "invalid",
            base64.b64encode(b"x" * 31).decode("ascii"),
            base64.b64encode(b"x" * 32).decode("ascii"),
        ]

        for seed in invalid_seeds:
            with self.subTest(seed=seed):
                result = self.run_script(
                    "check-key",
                    "--rust-source",
                    str(source),
                    seed=seed,
                    public_key=self.public_key,
                )
                self.assertNotEqual(result.returncode, 0)

    def test_check_key_rejects_invalid_public_key(self):
        source = self.rust_source()
        invalid_keys = [
            None,
            "invalid",
            base64.b64encode(b"x" * 31).decode("ascii"),
        ]

        for public_key in invalid_keys:
            with self.subTest(public_key=public_key):
                result = self.run_script(
                    "check-key",
                    "--rust-source",
                    str(source),
                    seed=self.seed,
                    public_key=public_key,
                )
                self.assertNotEqual(result.returncode, 0)

    def test_check_key_rejects_mismatched_embedded_key(self):
        embedded_key = base64.b64encode(b"x" * 32).decode("ascii")
        result = self.run_script(
            "check-key",
            "--rust-source",
            str(self.rust_source(embedded_key)),
            seed=self.seed,
            public_key=self.public_key,
        )

        self.assertNotEqual(result.returncode, 0)

if __name__ == "__main__":
    unittest.main()
