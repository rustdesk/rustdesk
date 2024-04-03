#!/usr/bin/env python3
# -*- coding: utf-8 -*-

import sys
import uuid
import argparse
from pathlib import Path

g_indent_unit = "\t"


def make_parser():
    parser = argparse.ArgumentParser(description="Msi preprocess script.")
    parser.add_argument(
        "-d", "--debug", action="store_true", help="Is debug", default=False
    )
    parser.add_argument(
        "-c", "--custom", action="store_true", help="Is custom client", default=False
    )
    parser.add_argument(
        "-an", "--app-name", type=str, default="RustDesk", help="The app name."
    )
    parser.add_argument(
        "-v", "--version", type=str, default="1.2.4", help="The app version."
    )
    parser.add_argument(
        "-m",
        "--manufacturer",
        type=str,
        default="Purslane Ltd",
        help="The app manufacturer.",
    )
    return parser


def read_lines_and_start_index(file_path, tag_start, tag_end):
    with open(file_path, "r") as f:
        lines = f.readlines()
    index_start = -1
    index_end = -1
    for i, line in enumerate(lines):
        if tag_start in line:
            index_start = i
        if tag_end in line:
            index_end = i

    if index_start == -1:
        print(f'Error: start tag "{tag_start}" not found')
        return None, None
    if index_end == -1:
        print(f'Error: end tag "{tag_end}" not found')
        return None, None
    return lines, index_start


def insert_components_between_tags(lines, index_start, app_name, build_dir):
    indent = g_indent_unit * 3
    path = Path(build_dir)
    idx = 1
    for file_path in path.glob("**/*"):
        if file_path.is_file():
            if file_path.name.lower() == f"{app_name}.exe".lower():
                continue

            relative_file_path = file_path.relative_to(path)
            guid = uuid.uuid5(
                uuid.NAMESPACE_OID, app_name + "/" + str(relative_file_path)
            )

            subdir = str(file_path.parent.relative_to(path))
            dir_attr = ""
            if subdir != ".":
                dir_attr = f'Subdirectory="{subdir}"'

            # Don't generate Component Id and File Id like 'Component_{idx}' and 'File_{idx}'
            # because it will cause error
            # "Error WIX0130	The primary key 'xxxx' is duplicated in table 'Directory'"
            to_insert_lines = f"""
{indent}<Component Guid="{guid}" {dir_attr}>
{indent}{g_indent_unit}<File Source="{file_path.as_posix()}" KeyPath="yes" Checksum="yes" />
{indent}</Component>
"""
            lines.insert(index_start + 1, to_insert_lines[1:])
            index_start += 1
            idx += 1
    return True


def gen_auto_component(app_name, build_dir):
    return gen_content_between_tags(
        "Package/Components/RustDesk.wxs",
        "<!--$AutoComonentStart$-->",
        "<!--$AutoComponentEnd$-->",
        lambda lines, index_start: insert_components_between_tags(
            lines, index_start, app_name, build_dir
        ),
    )


def gen_pre_vars(args, build_dir):
    def func(lines, index_start):
        upgrade_code = uuid.uuid5(uuid.NAMESPACE_OID, app_name + ".exe")

        indent = g_indent_unit * 1
        to_insert_lines = [
            f'{indent}<?define Version="{args.version}" ?>\n',
            f'{indent}<?define Manufacturer="{args.manufacturer}" ?>\n',
            f'{indent}<?define Product="{args.app_name}" ?>\n',
            f'{indent}<?define Description="{args.app_name} Installer" ?>\n',
            f'{indent}<?define ProductLower="{args.app_name.lower()}" ?>\n',
            f'{indent}<?define RegKeyRoot=".$(var.ProductLower)" ?>\n',
            f'{indent}<?define RegKeyInstall="$(var.RegKeyRoot)\Install" ?>\n',
            f'{indent}<?define BuildDir="{build_dir}" ?>\n',
            "\n",
            f"{indent}<!-- The UpgradeCode must be consistent for each product. ! -->\n"
            f'{indent}<?define UpgradeCode = "{upgrade_code}" ?>\n',
        ]

        for i, line in enumerate(to_insert_lines):
            lines.insert(index_start + i + 1, line)

    return gen_content_between_tags(
        "Package/Includes.wxi", "<!--$PreVarsStart$-->", "<!--$PreVarsEnd$-->", func
    )


def replace_app_name_in_lans(app_name):
    langs_dir = Path(sys.argv[0]).parent.joinpath("Package/Language")
    for file_path in langs_dir.glob("*.wxs"):
        with open(file_path, "r") as f:
            lines = f.readlines()
        for i, line in enumerate(lines):
            lines[i] = line.replace("RustDesk", app_name)
        with open(file_path, "w") as f:
            f.writelines(lines)


def gen_upgrade_info(version):
    def func(lines, index_start):
        indent = g_indent_unit * 3

        major, _, _ = version.split(".")
        upgrade_id = uuid.uuid4()
        to_insert_lines = [
            f'{indent}<Upgrade Id="{upgrade_id}">\n',
            f'{indent}{g_indent_unit}<UpgradeVersion Property="OLD_VERSION_FOUND" Minimum="{major}.0.0.0" Maximum="{major}.99.99" IncludeMinimum="yes" IncludeMaximum="yes" OnlyDetect="no" IgnoreRemoveFailure="yes" MigrateFeatures="yes" />\n',
            f"{indent}</Upgrade>\n",
        ]

        for i, line in enumerate(to_insert_lines):
            lines.insert(index_start + i + 1, line)
        return lines

    return gen_content_between_tags(
        "Package/Fragments/Upgrades.wxs",
        "<!--$UpgradeStart$-->",
        "<!--$UpgradeEnd$-->",
        func,
    )


def gen_content_between_tags(filename, tag_start, tag_end, func):
    target_file = Path(sys.argv[0]).parent.joinpath(filename)
    lines, index_start = read_lines_and_start_index(target_file, tag_start, tag_end)
    if lines is None:
        return False

    func(lines, index_start)

    with open(target_file, "w") as f:
        f.writelines(lines)

    return True


if __name__ == "__main__":
    parser = make_parser()
    args = parser.parse_args()

    app_name = args.app_name
    build_dir = (
        Path(sys.argv[0])
        .parent.joinpath(
            f'../../flutter/build/windows/x64/runner/{"Debug" if args.debug else "Release"}'
        )
        .resolve()
    )

    if not gen_pre_vars(args, build_dir):
        sys.exit(-1)

    if not gen_upgrade_info(args.version):
        sys.exit(-1)

    if not gen_auto_component(app_name, build_dir):
        sys.exit(-1)

    replace_app_name_in_lans(args.app_name)
