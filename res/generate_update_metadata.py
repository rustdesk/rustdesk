#!/usr/bin/env python3
import argparse
import base64
import binascii
import hashlib
import json
import os
import re
from datetime import datetime
from pathlib import Path
from urllib.parse import unquote

from cryptography.exceptions import InvalidSignature
from cryptography.hazmat.primitives import serialization
from cryptography.hazmat.primitives.asymmetric import ed25519

APP_NAME = "rustdesk"
SCHEMA_VERSION = 1
SIGNATURE_ALGORITHM = "ed25519"
SIGNATURE_CONTEXT = b"RustDesk update metadata v1\n"
KEY_ID = "2026-ed25519-main"
SEED_ENV = "RUSTDESK_UPDATE_ED25519_SEED"
PUBLIC_KEY_ENV = "RUSTDESK_UPDATE_ED25519_PUBLIC_KEY"
GITHUB_RELEASE_PREFIX = "https://github.com/rustdesk/rustdesk/releases/download"
RFC3339_TIMESTAMP = re.compile(
    r"[0-9]{4}-[0-9]{2}-[0-9]{2}[Tt][0-9]{2}:[0-9]{2}:[0-9]{2}(?:\.[0-9]+)?(?:[Zz]|[+-][0-9]{2}:[0-9]{2})"
)
ARTIFACT_FILE_NAME_PATTERN = re.compile(r"[A-Za-z0-9._-]+")


def fail(message):
    raise SystemExit(message)


def stable_json(value):
    return json.dumps(value, separators=(",", ":"), sort_keys=True).encode("utf-8")


def write_bytes(path, data):
    output = Path(path)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_bytes(data)


def sha256_hex(path):
    hasher = hashlib.sha256()
    with Path(path).open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            hasher.update(chunk)
    return hasher.hexdigest()


def decode_env(name, expected_size):
    encoded = os.environ.get(name)
    if not encoded:
        fail(f"missing environment variable: {name}")
    try:
        value = base64.b64decode(encoded, validate=True)
    except (ValueError, binascii.Error) as error:
        fail(f"invalid base64 in {name}: {error}")
    if len(value) != expected_size:
        fail(f"{name} must decode to {expected_size} bytes")
    if base64.b64encode(value).decode("ascii") != encoded:
        fail(f"{name} must use canonical base64")
    return value


def validate_release(release_id, version):
    if (
        not release_id
        or not release_id.isascii()
        or release_id in {".", ".."}
        or any(char in release_id for char in (" ", "/", "\\", "?", "#"))
        or unquote(release_id) != release_id
    ):
        fail(f"invalid release id: {release_id}")
    match = re.fullmatch(r"v?([0-9]+)\.([0-9]+)\.([0-9]+)", release_id)
    if not match:
        return
    segments = [int(part) for part in match.groups()]
    if any(part > 65535 for part in segments) or not any(segments):
        fail(f"invalid release version: {release_id}")
    display_version = ".".join(match.groups())
    if display_version != version:
        fail(f"release id {release_id} maps to {display_version}, not {version}")


def validate_published_at(value):
    if not RFC3339_TIMESTAMP.fullmatch(value):
        fail("published_at must be an RFC 3339 timestamp")
    normalized = value[:-1] + "+00:00" if value[-1] in "Zz" else value
    try:
        datetime.fromisoformat(normalized)
    except ValueError:
        fail("published_at must be an RFC 3339 timestamp")


def validate_artifact_file_name(file_name):
    if (
        not isinstance(file_name, str)
        or file_name in {".", ".."}
        or not ARTIFACT_FILE_NAME_PATTERN.fullmatch(file_name)
    ):
        fail(f"invalid artifact file name: {file_name}")


def artifact_metadata(spec, release_id):
    platform, arch, file_format, raw_path = spec
    if not all(value.strip() for value in (platform, arch, file_format)):
        fail("artifact selector fields must not be empty")
    path = Path(raw_path)
    file_name = path.name
    validate_artifact_file_name(file_name)
    return {
        "platform": platform,
        "arch": arch,
        "format": file_format,
        "url": f"{GITHUB_RELEASE_PREFIX}/{release_id}/{file_name}",
        "file_name": file_name,
        "size": path.stat().st_size,
        "sha256": sha256_hex(path),
    }


def command_sign(args):
    metadata_out = Path(args.metadata_out).resolve()
    signature_out = Path(args.signature_out).resolve()
    if metadata_out == signature_out:
        fail("metadata and signature outputs must be different files")
    validate_release(args.release_id, args.version)
    validate_published_at(args.published_at)
    artifacts = [artifact_metadata(spec, args.release_id) for spec in args.artifact]
    selectors = [(item["platform"], item["arch"], item["format"]) for item in artifacts]
    names = [item["file_name"] for item in artifacts]
    if len(set(selectors)) != len(selectors):
        fail("duplicate artifact selector")
    if len(set(names)) != len(names):
        fail("duplicate artifact file name")
    artifacts.sort(key=lambda item: (item["platform"], item["arch"], item["format"]))
    metadata = {
        "schema_version": SCHEMA_VERSION,
        "app": APP_NAME,
        "package_id": APP_NAME,
        "version": args.version,
        "release_id": args.release_id,
        "published_at": args.published_at,
        "signature_key_id": KEY_ID,
        "artifacts": artifacts,
    }
    private_key = ed25519.Ed25519PrivateKey.from_private_bytes(decode_env(SEED_ENV, 32))
    metadata_bytes = stable_json(metadata)
    signature = {
        "schema_version": SCHEMA_VERSION,
        "algorithm": SIGNATURE_ALGORITHM,
        "key_id": KEY_ID,
        "signature": base64.b64encode(
            private_key.sign(SIGNATURE_CONTEXT + metadata_bytes)
        ).decode("ascii"),
    }
    write_bytes(metadata_out, metadata_bytes)
    write_bytes(signature_out, stable_json(signature))


def command_verify(args):
    metadata_bytes = Path(args.metadata).read_bytes()
    metadata = json.loads(metadata_bytes)
    signature = json.loads(Path(args.signature).read_bytes())
    if (
        signature.get("schema_version") != SCHEMA_VERSION
        or signature.get("algorithm") != SIGNATURE_ALGORITHM
        or signature.get("key_id") != KEY_ID
    ):
        fail("invalid signature metadata")
    public_key = ed25519.Ed25519PublicKey.from_public_bytes(decode_env(PUBLIC_KEY_ENV, 32))
    try:
        public_key.verify(
            base64.b64decode(signature["signature"], validate=True),
            SIGNATURE_CONTEXT + metadata_bytes,
        )
    except (InvalidSignature, ValueError, binascii.Error):
        fail("invalid metadata signature")
    expected_fields = {
        "schema_version": SCHEMA_VERSION,
        "app": APP_NAME,
        "package_id": APP_NAME,
        "version": args.version,
        "release_id": args.release_id,
        "signature_key_id": KEY_ID,
    }
    if any(metadata.get(key) != value for key, value in expected_fields.items()):
        fail("metadata does not match the release")
    validate_release(args.release_id, args.version)
    validate_published_at(metadata.get("published_at", ""))
    local_paths = [Path(path) for path in args.artifact]
    local_artifacts = {path.name: path for path in local_paths}
    if len(local_artifacts) != len(local_paths):
        fail("duplicate local artifact file name")
    artifacts = metadata.get("artifacts")
    if not isinstance(artifacts, list) or not artifacts:
        fail("metadata must contain artifacts")
    selector_fields = ("platform", "arch", "format")
    required_artifact_fields = selector_fields + (
        "file_name",
        "url",
        "size",
        "sha256",
    )
    if any(
        not isinstance(artifact, dict)
        or any(field not in artifact for field in required_artifact_fields)
        for artifact in artifacts
    ):
        fail("artifact metadata is missing required fields")
    names = [artifact.get("file_name") for artifact in artifacts]
    for file_name in names:
        validate_artifact_file_name(file_name)
    selectors = [tuple(artifact[field] for field in selector_fields) for artifact in artifacts]
    if any(
        not all(isinstance(value, str) and value.strip() for value in selector)
        for selector in selectors
    ):
        fail("artifact selector fields must be non-empty strings")
    if len(set(names)) != len(names) or set(names) != set(local_artifacts):
        fail("artifact file set mismatch")
    if len(set(selectors)) != len(selectors):
        fail("duplicate artifact selector")
    for artifact in artifacts:
        file_name = artifact["file_name"]
        if artifact["url"] != f"{GITHUB_RELEASE_PREFIX}/{args.release_id}/{file_name}":
            fail(f"artifact URL mismatch for {file_name}")
        path = local_artifacts[file_name]
        if path.stat().st_size != artifact["size"]:
            fail(f"artifact size mismatch for {file_name}")
        if sha256_hex(path) != artifact["sha256"]:
            fail(f"artifact sha256 mismatch for {file_name}")


def command_check_key(args):
    seed = decode_env(SEED_ENV, 32)
    configured_key = decode_env(PUBLIC_KEY_ENV, 32)
    derived_key = ed25519.Ed25519PrivateKey.from_private_bytes(seed).public_key()
    derived_key_bytes = derived_key.public_bytes(
        serialization.Encoding.Raw,
        serialization.PublicFormat.Raw,
    )
    if derived_key_bytes != configured_key:
        fail(f"{SEED_ENV} does not match {PUBLIC_KEY_ENV}")

    source = Path(args.rust_source).read_text(encoding="utf-8")
    match = re.search(
        rf'TrustedUpdateKey\s*\{{[^{{}}]*key_id:\s*"{KEY_ID}"[^{{}}]*public_key:\s*\[([^\]]+)\]',
        source,
        re.S,
    )
    if not match:
        fail("failed to find embedded update public key")
    try:
        embedded_key = bytes(
            int(part) for part in match.group(1).split(",") if part.strip()
        )
    except ValueError as error:
        fail(f"invalid embedded update public key: {error}")
    if embedded_key != configured_key:
        fail(f"embedded update public key does not match {PUBLIC_KEY_ENV}")


def build_parser():
    parser = argparse.ArgumentParser()
    commands = parser.add_subparsers(dest="command", required=True)
    sign = commands.add_parser("sign")
    sign.add_argument("--artifact", action="append", nargs=4, required=True)
    sign.add_argument("--version", required=True)
    sign.add_argument("--release-id", required=True)
    sign.add_argument("--published-at", required=True)
    sign.add_argument("--metadata-out", required=True)
    sign.add_argument("--signature-out", required=True)
    sign.set_defaults(func=command_sign)
    verify = commands.add_parser("verify")
    verify.add_argument("--metadata", required=True)
    verify.add_argument("--signature", required=True)
    verify.add_argument("--artifact", action="append", required=True)
    verify.add_argument("--version", required=True)
    verify.add_argument("--release-id", required=True)
    verify.set_defaults(func=command_verify)
    check_key = commands.add_parser("check-key")
    check_key.add_argument(
        "--rust-source", default="src/update_metadata.rs"
    )
    check_key.set_defaults(func=command_check_key)
    return parser


def main():
    args = build_parser().parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
