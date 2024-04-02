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


def read_lines_and_start_index(file_path, start_tag, end_tag):
    with open(file_path, "r") as f:
        lines = f.readlines()
    start_index = -1
    end_index = -1
    for i, line in enumerate(lines):
        if start_tag in line:
            start_index = i
        if end_tag in line:
            end_index = i

    if start_index == -1 or end_index == -1:
        print("Error: start or end tag not found")
        return None, None
    return lines, start_index


def insert_components_between_tags(lines, start_index, app_name, build_dir):
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
            lines.insert(start_index + 1, to_insert_lines[1:])
            start_index += 1
            idx += 1
    return True


def gen_auto_component(app_name, build_dir):
    target_file = Path(sys.argv[0]).parent.joinpath("Package/Components/RustDesk.wxs")
    start_tag = "<!--$AutoComonentStart$-->"
    end_tag = "<!--$AutoComponentEnd$-->"

    lines, start_index = read_lines_and_start_index(target_file, start_tag, end_tag)
    if lines is None:
        return False

    if not insert_components_between_tags(lines, start_index, app_name, build_dir):
        return False

    with open(target_file, "w") as f:
        f.writelines(lines)

    return True


def gen_pre_vars(args, build_dir):
    target_file = Path(sys.argv[0]).parent.joinpath("Package/Includes.wxi")
    start_tag = "<!--$PreVarsStart$-->"
    end_tag = "<!--$PreVarsEnd$-->"

    lines, start_index = read_lines_and_start_index(target_file, start_tag, end_tag)
    if lines is None:
        return False

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
    ]

    for i, line in enumerate(to_insert_lines):
        lines.insert(start_index + i + 1, line)

    with open(target_file, "w") as f:
        f.writelines(lines)

    return True


def replace_app_name_in_lans(app_name):
    langs_dir = Path(sys.argv[0]).parent.joinpath("Package/Language")
    for file_path in langs_dir.glob("*.wxs"):
        with open(file_path, "r") as f:
            lines = f.readlines()
        for i, line in enumerate(lines):
            lines[i] = line.replace("RustDesk", app_name)
        with open(file_path, "w") as f:
            f.writelines(lines)


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

    if not gen_auto_component(app_name, build_dir):
        sys.exit(-1)

    replace_app_name_in_lans(args.app_name)
