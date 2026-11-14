#!/usr/bin/env bash

set -eo pipefail

SCRIPTDIR="$(readlink -f "${0}")"
SCRIPTDIR="$(dirname "${SCRIPTDIR}")"

# https://docs.flutter.dev/deployment/ios
# flutter build ipa --release --obfuscate --split-debug-info=./split-debug-info
# no obfuscate, because no easy to check errors

cd "$(dirname "$(dirname "$(which flutter)")")"
if ! git apply "${SCRIPTDIR}/../.github/patches/flutter_3.24.4_dropdown_menu_enableFilter.diff"; then
	if ! git apply -R "${SCRIPTDIR}/../.github/patches/flutter_3.24.4_dropdown_menu_enableFilter.diff"; then
		echo 'ERROR: Patch "${SCRIPTDIR}/../.github/patches/flutter_3.24.4_dropdown_menu_enableFilter.diff" does not apply!' >&2
		exit 1
	fi
	git apply "${SCRIPTDIR}/../.github/patches/flutter_3.24.4_dropdown_menu_enableFilter.diff"
fi
cd -

cd "${SCRIPTDIR}"

# Bail out on uncommitted changes to pubspec files
python3 "${SCRIPTDIR}/flutter_pub_get.py" check-local-pubspec-changes
# Revert possible pubspec alterations and package config freeze for idempotency
python3 "${SCRIPTDIR}/flutter_pub_get.py" revert-local-pubspec-changes
# Download flutter packages with default lockfile
python3 "${SCRIPTDIR}/flutter_pub_get.py" get-packages

# Revert possible pubspec and package config alterations
# on both successful operation and on error for idempotency
trap 'python3 "${SCRIPTDIR}/flutter_pub_get.py" revert-local-pubspec-changes' 0 1 2 3 6 15

flutter build ipa --release
