#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
flutter_root="$repo_root/flutter"
profile="$flutter_root/ohos/build-profile.json5"
build_mode="${OHOS_BUILD_MODE:-debug}"

if [[ "$build_mode" != "debug" && "$build_mode" != "profile" && "$build_mode" != "release" ]]; then
  echo "Unsupported OHOS_BUILD_MODE: $build_mode" >&2
  exit 2
fi
if ! command -v flutter >/dev/null 2>&1; then
  echo "Flutter-OH is not available on PATH" >&2
  exit 2
fi
if ! command -v hvigorw >/dev/null 2>&1; then
  echo "hvigorw is not available on PATH" >&2
  exit 2
fi

backup=""
cleanup() {
  if [[ -n "$backup" && -f "$backup" ]]; then
    cp "$backup" "$profile"
    rm -f "$backup"
  fi
}
trap cleanup EXIT

if [[ -n "${RUSTDESK_SIGNING_DIR:-}" ]]; then
  signing_json="$(cd "$RUSTDESK_SIGNING_DIR" && pwd)/signingConfigs.json"
  if [[ ! -r "$signing_json" ]]; then
    echo "RUSTDESK_SIGNING_DIR must contain signingConfigs.json" >&2
    exit 2
  fi
  backup="$(mktemp /tmp/rustdesk-flutter-build-profile.XXXXXX)"
  cp "$profile" "$backup"
  PROFILE="$profile" SIGNING_JSON="$signing_json" python3 - <<'PY'
import json
import os
from pathlib import Path

profile_path = Path(os.environ["PROFILE"])
signing_path = Path(os.environ["SIGNING_JSON"])
profile = json.loads(profile_path.read_text(encoding="utf-8"))
configs = json.loads(signing_path.read_text(encoding="utf-8"))
if not isinstance(configs, list):
    raise SystemExit("signingConfigs.json must contain an array")
config = next((item for item in configs if item.get("name") == "default"), None)
if config is None:
    raise SystemExit("signingConfigs.json has no default configuration")
material = config.get("material", {})
for field in ("storeFile", "profile", "certpath"):
    value = material.get(field)
    if not isinstance(value, str) or not Path(value).is_file():
        raise SystemExit(f"default signing material {field} is missing")
profile["app"]["signingConfigs"] = [config]
for product in profile["app"].get("products", []):
    if product.get("name") == "default":
        product["signingConfig"] = "default"
profile_path.write_text(json.dumps(profile, indent=2) + "\n", encoding="utf-8")
PY
fi

cd "$flutter_root"
env -u RUSTDESK_SIGNING_DIR flutter build hap "--$build_mode" "$@"
