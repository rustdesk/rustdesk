#!/usr/bin/env python3
import os
import sys
import json
import shutil
import subprocess
from pathlib import Path

# Helper script to fetch Flutter packages from pub.get
# with lockfile enforcement.
#
# It is designed as multi-step tool:
#
# 1. First, the caller makes sure that working tree does not
#    have uncommitted changes to 'flutter/pubspec.yaml' and
#    'flutter/pubspec.lock' by calling this script with
#    'check-local-pubspec-changes'. This invocation fails with
#    error if there are such changes, not discarding them.
#
# 2. Then, after pubspec files are possibly patched for bridge
#    or windows/arm64 or web builds, the actual download process
#    is started by calling this script with 'get-packages'
#    specifying the lockfile to temporary replace with.
#    Empty value of lockfile means pubspec.lock remains intact.
#    This step also patches '.dart_tool/package_config.json'
#    to disable Flutter package resolution during 'flutter build'
#    step.
#
# 3. The caller than invokes 'flutter build' as usual
#
# 4. Finally, 'flutter/pubspec.yaml', 'flutter/pubspec.lock'
#    and '.dart_tool/package_config.json' are restored by
#    calling this script with 'revert-local-pubspec-changes'.
#

# Get this script directory (resolved absolute path)
SCRIPT_DIR = Path(__file__).resolve().parent


def print_err(*args, **kwargs):
    """Helper to print to stderr."""
    print(*args, file=sys.stderr, **kwargs)


def check_local_pubspec_changes():
    """Check if there are uncommitted changes in pubspec.lock and pubspec.yaml."""
    lock_file = SCRIPT_DIR / "pubspec.lock"
    yaml_file = SCRIPT_DIR / "pubspec.yaml"

    res = subprocess.run(
        ["git", "diff", "--quiet", "--exit-code", "HEAD", "--", os.fspath(lock_file), os.fspath(yaml_file)],
        capture_output=True
    )

    if res.returncode != 0:
        print_err("ERROR: There are uncommitted changes in 'flutter/pubspec.lock' and 'flutter/pubspec.yaml'!")
        print_err("ERROR: Please commit them if you have changed these files and want to build the changes!")
        sys.exit(1)

    print("No uncommitted changes to Flutter lockfiles!")


def is_package_config_frozen():
    """
    Safely parses 'package_config.json' and checks the 'generator' field against 'pubForced'.
    Treats any error as unfrozen situation, because Flutter will recreate broken file.
    """
    config_dir = Path(f"{SCRIPT_DIR}/.dart_tool")
    if not config_dir.is_dir():
        print(f"Flutter package state directory '{config_dir}' does not exist - probably a clean build!")
        sys.exit(1)

    config_file = Path(f"{config_dir}/package_config.json")
    if not config_file.is_file():
        print_err(f"ERROR: Package config file '{config_file}' does not exist after 'flutter pub get'!")
        sys.exit(1)

    try:
        with open(config_file, "r", encoding="utf-8") as f:
            data = json.load(f)
    except Exception as e:
        print_err(f"ERROR: Could not parse package config file '{config_file}': {e}")
        sys.exit(1)

    if not isinstance(data, dict):
        print_err(f"ERROR: Malformed package config file '{config_file}'")
        # Not frozen
        sys.exit(1)

    if not "generator" in data:
        print_err(f"ERROR: Malformed package config file '{config_file}'")
        # Not frozen
        sys.exit(1)

    if "pubForced" != data["generator"]:
        # Not frozen
        sys.exit(1)


def patch_package_config(generator_value: str, file_must_exist: bool):
    """
    Safely parses 'package_config.json' and updates the 'generator' field.
    """
    config_file = Path(f"{SCRIPT_DIR}/.dart_tool/package_config.json")
    if not config_file.is_file():
        if file_must_exist:
            print_err(f"ERROR: Package config file '{config_file}' does not exist")
            print_err("ERROR: but it is required to exist here!")

            sys.exit(1)
        else:
            print(f"WARNING: Package config file '{config_file}' does not exist")
            print("WARNING: but file existence is not required here. Continuing ...")

            return

    try:
        with open(config_file, "r", encoding="utf-8") as f:
            data = json.load(f)
    except Exception as e:
        print_err(f"ERROR: Could not parse package config file '{config_file}': {e}")
        sys.exit(1)

    if not isinstance(data, dict):
        print_err(f"ERROR: Malformed package config file '{config_file}'")
        sys.exit(1)

    if not "generator" in data:
        print_err(f"ERROR: Malformed package config file '{config_file}'")
        sys.exit(1)

    data["generator"] = generator_value

    try:
        with open(config_file, "w", encoding="utf-8") as f:
            json.dump(data, f, indent=2)
    except Exception as e:
        print_err(f"ERROR: Could not write package config file '{config_file}': {e}")
        sys.exit(1)

    print(f"Successfully updated 'generator' to '{generator_value}' in '{config_file}'")


def get_packages(lockfile_param):
    """Get packages using the specified lockfile and patch 'package_config.json'."""
    target_lockfile = SCRIPT_DIR / "pubspec.lock"
    flutter_pubspec_lockfile = None

    # Install provided lockfile to 'pubspec.lock' if exists
    if lockfile_param:
        flutter_pubspec_lockfile = Path(lockfile_param)

        if not flutter_pubspec_lockfile.is_file():
            print_err(f"ERROR: Lockfile '{flutter_pubspec_lockfile}' does not exist!")
            print_err("ERROR:")
            print_err("ERROR: Please run 'flutter packages pub get', review resulting 'pubspec.lock''")
            print_err(f"ERROR: and copy it to '{flutter_pubspec_lockfile}'")
            print_err("ERROR:")

            sys.exit(1)

        print(f"Installing lockfile '{flutter_pubspec_lockfile}' into '{target_lockfile}' to get packages")

        try:
            shutil.copy2(flutter_pubspec_lockfile, target_lockfile)
        except Exception:
            print_err(f"ERROR: Could not copy lockfile '{flutter_pubspec_lockfile}' into '{target_lockfile}'")
            sys.exit(1)

    else:
        print(f"Using default lockfile '{target_lockfile}' to get packages")

    flutter_res = subprocess.run(
        ["flutter", "packages", "pub", "get", "--enforce-lockfile"],
        cwd=os.fspath(SCRIPT_DIR),
        shell=os.name == "nt"  # Resolve .bat/.cmd extension wrapping implicitly on Windows
    )

    if flutter_res.returncode != 0:
        # Restore original 'pubspec.lock'
        git_res = subprocess.run(
            ["git", "checkout", "HEAD", "--", os.fspath(target_lockfile)]
        )
        if git_res.returncode != 0:
             print_err(f"ERROR: Could not restore original '{target_lockfile}'!")
             print_err("ERROR: Please inspect working tree manually")

             sys.exit(1)

        if flutter_pubspec_lockfile:
            print_err(f"ERROR: Lockfile '{flutter_pubspec_lockfile}' needs to be updated!")
            print_err("ERROR:")
            print_err("ERROR: Please run 'flutter packages pub get' manually,")
            print_err("ERROR: review resulting 'pubspec.lock'")
            print_err(f"ERROR: and copy it to '{flutter_pubspec_lockfile}'")
        else:
            print_err(f"ERROR: Lockfile '{target_lockfile}' needs to be updated!")
            print_err("ERROR:")
            print_err("ERROR: Please run 'flutter packages pub get' manually,")
            print_err(f"ERROR: review resulting '{target_lockfile}'")
            print_err("ERROR: and commit it to the tree!")

        print_err("ERROR:")

        sys.exit(1)

    patch_package_config("pubForced", True)

    print("Flutter packages successfully downloaded using enforced lockfile!")
    print("Please dont forget running 'flutter/flutter_pub_get.py revert-local-pubspec-changes'")
    print("after 'flutter build' to restore package resolutions by 'flutter build'!")


def revert_local_pubspec_changes():
    """Restore changes made in 'get-packages'."""
    lock_file = SCRIPT_DIR / "pubspec.lock"
    yaml_file = SCRIPT_DIR / "pubspec.yaml"

    # Restore pubspec files
    git_res = subprocess.run(
        ["git", "checkout", "HEAD", "--", os.fspath(lock_file), os.fspath(yaml_file)]
    )
    if git_res.returncode != 0:
        print_err("ERROR: Could not restore pubspec file changes!")
        print_err("ERROR: Please inspect working tree manually")
        sys.exit(1)

    # Restore package config file
    patch_package_config("pub", False)

    print("Flutter lockfiles successfully restored!")


def main():
    args = sys.argv[1:]
    if not args:
        print_err("ERROR: No command provided!")
        sys.exit(1)

    idx = 0
    while idx < len(args):
        command = args[idx]

        if command == "check-local-pubspec-changes":
            check_local_pubspec_changes()
            break
        elif command == "is-package-config-frozen":
            is_package_config_frozen()
            break
        elif command == "get-packages":
            lockfile_param = None
            if idx + 1 < len(args):
                lockfile_param = args[idx + 1]
            get_packages(lockfile_param)
            break
        elif command == "revert-local-pubspec-changes":
            revert_local_pubspec_changes()
            break
        else:
            print_err(f"Unknown command '{command}'!")
            sys.exit(1)


if __name__ == "__main__":
    main()
