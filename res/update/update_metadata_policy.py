import re
from datetime import datetime
from string import ascii_letters, digits
from urllib.parse import urlsplit

PACKAGE_ID_CHARACTERS = frozenset(ascii_letters + digits + ".-")
RFC3339_TIMESTAMP = re.compile(
    r"[0-9]{4}-[0-9]{2}-[0-9]{2}[Tt][0-9]{2}:[0-9]{2}:[0-9]{2}(?:\.[0-9]+)?(?:[Zz]|[+-][0-9]{2}:[0-9]{2})"
)
ARTIFACT_FILE_NAME_PATTERN = re.compile(r"[A-Za-z0-9._-]+")


def validate_release(release_id, version):
    if (
        not release_id
        or not release_id.isascii()
        or release_id in {".", ".."}
        or any(
            char.isspace() or not char.isprintable() or char in "/\\?%#"
            for char in release_id
        )
    ):
        raise ValueError(f"invalid release id: {release_id}")
    match = re.fullmatch(r"v?([0-9]+)\.([0-9]+)\.([0-9]+)", release_id)
    if not match:
        return
    segments = [int(part) for part in match.groups()]
    if any(part > 65535 for part in segments) or not any(segments):
        raise ValueError(f"invalid release version: {release_id}")
    display_version = ".".join(match.groups())
    if display_version != version:
        raise ValueError(
            f"release id {release_id} maps to {display_version}, not {version}"
        )


def validate_published_at(value):
    if not RFC3339_TIMESTAMP.fullmatch(value):
        raise ValueError("published_at must be an RFC 3339 timestamp")
    normalized = value[:-1] + "+00:00" if value[-1] in "Zz" else value
    try:
        datetime.fromisoformat(normalized)
    except ValueError as error:
        raise ValueError(
            "published_at must be an RFC 3339 timestamp"
        ) from error


def validate_artifact_file_name(file_name):
    if (
        not isinstance(file_name, str)
        or file_name in {".", ".."}
        or not ARTIFACT_FILE_NAME_PATTERN.fullmatch(file_name)
    ):
        raise ValueError(f"invalid artifact file name: {file_name}")


def validate_package_id(value):
    if (
        not isinstance(value, str)
        or not value
        or not value.isascii()
        or any(char not in PACKAGE_ID_CHARACTERS for char in value)
        or not any(char.isalnum() for char in value)
    ):
        raise ValueError(f"invalid package id: {value}")
    return value


def validate_release_base_url(value):
    if (
        not isinstance(value, str)
        or not value
        or not value.isascii()
        or any(char.isspace() or not char.isprintable() for char in value)
        or value.endswith("/")
        or "\\" in value
        or "?" in value
        or "#" in value
    ):
        raise ValueError(f"invalid release base URL: {value}")
    try:
        parsed = urlsplit(value)
        parsed.port
    except ValueError as error:
        raise ValueError(f"invalid release base URL: {value}") from error
    if (
        parsed.scheme != "https"
        or parsed.hostname is None
        or parsed.username is not None
        or parsed.password is not None
    ):
        raise ValueError(
            "release base URL must be HTTPS and must not contain credentials"
        )
    return value


def release_artifact_url(release_base_url, release_id, file_name):
    return f"{release_base_url}/{release_id}/{file_name}"
