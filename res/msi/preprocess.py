#!/usr/bin/env python3
# -*- coding: utf-8 -*-

import json
import sys
import uuid
import argparse
import datetime
import re
from pathlib import Path

g_indent_unit = "\t"
g_version = ""
g_build_date = datetime.datetime.now().strftime("%Y-%m-%d %H:%M")

# Replace the following links with your own in the custom arp properties.
# https://learn.microsoft.com/en-us/windows/win32/msi/property-reference
g_arpsystemcomponent = {
    "Comments": {
        "msi": "ARPCOMMENTS",
        "t": "string",
        "v": "!(loc.AR_Comment)",
    },
    "Contact": {
        "msi": "ARPCONTACT",
        "v": "https://github.com/rustdesk/rustdesk",
    },
    "HelpLink": {
        "msi": "ARPHELPLINK",
        "v": "https://github.com/rustdesk/rustdesk/issues/",
    },
    "ReadMe": {
        "msi": "ARPREADME",
        "v": "https://github.com/fufesou/rustdesk",
    },
}


def make_parser():
    parser = argparse.ArgumentParser(description="Msi preprocess script.")
    parser.add_argument(
        "-d",
        "--dist-dir",
        type=str,
        default="../../rustdesk",
        help="The dist direcotry to install.",
    )
    parser.add_argument(
        "-arp",
        "--arp",
        action="store_true",
        help="Is ARPSYSTEMCOMPONENT",
        default=False,
    )
    parser.add_argument(
        "-custom-arp",
        "--custom-arp",
        type=str,
        default="{}",
        help='Custom arp properties, e.g. \'["Comments": {"msi": "ARPCOMMENTS", "v": "Remote control application."}]\'',
    )
    parser.add_argument(
        "-c", "--custom", action="store_true", help="Is custom client", default=False
    )
    parser.add_argument(
        "-an", "--app-name", type=str, default="RustDesk", help="The app name."
    )
    parser.add_argument(
        "-v", "--version", type=str, default="", help="The app version."
    )
    parser.add_argument(
        "-m",
        "--manufacturer",
        type=str,
        default="PURSLANE",
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


def insert_components_between_tags(lines, index_start, app_name, dist_dir):
    indent = g_indent_unit * 3
    path = Path(dist_dir)
    idx = 1
    for file_path in path.glob("**/*"):
        if file_path.is_file():
            if file_path.name.lower() == f"{app_name}.exe".lower():
                continue

            subdir = str(file_path.parent.relative_to(path))
            dir_attr = ""
            if subdir != ".":
                dir_attr = f'Subdirectory="{subdir}"'

            # Don't generate Component Id and File Id like 'Component_{idx}' and 'File_{idx}'
            # because it will cause error
            # "Error WIX0130	The primary key 'xxxx' is duplicated in table 'Directory'"
            to_insert_lines = f"""
{indent}<Component Guid="{uuid.uuid4()}" {dir_attr}>
{indent}{g_indent_unit}<File Source="{file_path.as_posix()}" KeyPath="yes" Checksum="yes" />
{indent}</Component>
"""
            lines.insert(index_start + 1, to_insert_lines[1:])
            index_start += 1
            idx += 1
    return True


def gen_auto_component(app_name, dist_dir):
    return gen_content_between_tags(
        "Package/Components/RustDesk.wxs",
        "<!--$AutoComonentStart$-->",
        "<!--$AutoComponentEnd$-->",
        lambda lines, index_start: insert_components_between_tags(
            lines, index_start, app_name, dist_dir
        ),
    )


def gen_pre_vars(args, dist_dir):
    def func(lines, index_start):
        upgrade_code = uuid.uuid5(uuid.NAMESPACE_OID, app_name + ".exe")

        indent = g_indent_unit * 1
        to_insert_lines = [
            f'{indent}<?define Version="{g_version}" ?>\n',
            f'{indent}<?define Manufacturer="{args.manufacturer}" ?>\n',
            f'{indent}<?define Product="{args.app_name}" ?>\n',
            f'{indent}<?define Description="{args.app_name} Installer" ?>\n',
            f'{indent}<?define ProductLower="{args.app_name.lower()}" ?>\n',
            f'{indent}<?define RegKeyRoot=".$(var.ProductLower)" ?>\n',
            f'{indent}<?define RegKeyInstall="$(var.RegKeyRoot)\Install" ?>\n',
            f'{indent}<?define BuildDir="{dist_dir}" ?>\n',
            f'{indent}<?define BuildDate="{g_build_date}" ?>\n',
            "\n",
            f"{indent}<!-- The UpgradeCode must be consistent for each product. ! -->\n"
            f'{indent}<?define UpgradeCode = "{upgrade_code}" ?>\n',
        ]

        for i, line in enumerate(to_insert_lines):
            lines.insert(index_start + i + 1, line)
        return lines

    return gen_content_between_tags(
        "Package/Includes.wxi", "<!--$PreVarsStart$-->", "<!--$PreVarsEnd$-->", func
    )


def replace_app_name_in_langs(app_name):
    langs_dir = Path(sys.argv[0]).parent.joinpath("Package/Language")
    for file_path in langs_dir.glob("*.wxl"):
        with open(file_path, "r") as f:
            lines = f.readlines()
        for i, line in enumerate(lines):
            lines[i] = line.replace("RustDesk", app_name)
        with open(file_path, "w") as f:
            f.writelines(lines)


def gen_upgrade_info():
    def func(lines, index_start):
        indent = g_indent_unit * 3

        major, _, _ = g_version.split(".")
        upgrade_id = uuid.uuid4()
        to_insert_lines = [
            f'{indent}<Upgrade Id="{upgrade_id}">\n',
            f'{indent}{g_indent_unit}<UpgradeVersion Property="OLD_VERSION_FOUND" Minimum="{major}.0.0" Maximum="{major}.99.99" IncludeMinimum="yes" IncludeMaximum="yes" OnlyDetect="no" IgnoreRemoveFailure="yes" MigrateFeatures="yes" />\n',
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


def gen_custom_dialog_bitmaps():
    def func(lines, index_start):
        indent = g_indent_unit * 2

        # https://wixtoolset.org/docs/tools/wixext/wixui/#customizing-a-dialog-set
        vars = [
            "WixUIBannerBmp",
            "WixUIDialogBmp",
            "WixUIExclamationIco",
            "WixUIInfoIco",
            "WixUINewIco",
            "WixUIUpIco",
        ]
        to_insert_lines = []
        for var in vars:
            if Path(f"Package/Resources/{var}.bmp").exists():
                to_insert_lines.append(
                    f'{indent}<WixVariable Id="{var}" Value="Resources\\{var}.bmp" />\n'
                )

        for i, line in enumerate(to_insert_lines):
            lines.insert(index_start + i + 1, line)
        return lines

    return gen_content_between_tags(
        "Package/Package.wxs",
        "<!--$CustomBitmapsStart$-->",
        "<!--$CustomBitmapsEnd$-->",
        func,
    )


def gen_custom_ARPSYSTEMCOMPONENT_False(args):
    def func(lines, index_start):
        indent = g_indent_unit * 2

        lines_new = []
        lines_new.append(
            f"{indent}<!--https://learn.microsoft.com/en-us/windows/win32/msi/arpsystemcomponent?redirectedfrom=MSDN-->\n"
        )
        lines_new.append(
            f'{indent}<!--<Property Id="ARPSYSTEMCOMPONENT" Value="1" />-->\n\n'
        )

        lines_new.append(
            f"{indent}<!--https://learn.microsoft.com/en-us/windows/win32/msi/property-reference-->\n"
        )
        for _, v in g_arpsystemcomponent.items():
            if "msi" in v and "v" in v:
                lines_new.append(
                    f'{indent}<Property Id="{v["msi"]}" Value="{v["v"]}" />\n'
                )

        for i, line in enumerate(lines_new):
            lines.insert(index_start + i + 1, line)
        return lines

    return gen_content_between_tags(
        "Package/Fragments/AddRemoveProperties.wxs",
        "<!--$ArpStart$-->",
        "<!--$ArpEnd$-->",
        func,
    )


def get_folder_size(folder_path):
    total_size = 0

    folder = Path(folder_path)
    for file in folder.glob("**/*"):
        if file.is_file():
            total_size += file.stat().st_size

    return total_size


def gen_custom_ARPSYSTEMCOMPONENT_True(args, dist_dir):
    def func(lines, index_start):
        indent = g_indent_unit * 5

        lines_new = []
        lines_new.append(
            f"{indent}<!--https://learn.microsoft.com/en-us/windows/win32/msi/property-reference-->\n"
        )
        lines_new.append(
            f'{indent}<RegistryValue Type="string" Name="DisplayName" Value="{args.app_name}" />\n'
        )
        lines_new.append(
            f'{indent}<RegistryValue Type="string" Name="DisplayIcon" Value="[INSTALLFOLDER]{args.app_name}.exe" />\n'
        )
        lines_new.append(
            f'{indent}<RegistryValue Type="string" Name="DisplayVersion" Value="{g_version}" />\n'
        )
        lines_new.append(
            f'{indent}<RegistryValue Type="string" Name="Publisher" Value="{args.manufacturer}" />\n'
        )
        installDate = datetime.datetime.now().strftime("%Y%m%d")
        lines_new.append(
            f'{indent}<RegistryValue Type="string" Name="InstallDate" Value="{installDate}" />\n'
        )
        lines_new.append(
            f'{indent}<RegistryValue Type="string" Name="InstallLocation" Value="[INSTALLFOLDER]" />\n'
        )
        lines_new.append(
            f'{indent}<RegistryValue Type="string" Name="InstallSource" Value="[InstallSource]" />\n'
        )
        lines_new.append(
            f'{indent}<RegistryValue Type="integer" Name="Language" Value="[ProductLanguage]" />\n'
        )

        estimated_size = get_folder_size(dist_dir)
        lines_new.append(
            f'{indent}<RegistryValue Type="integer" Name="EstimatedSize" Value="{estimated_size}" />\n'
        )

        lines_new.append(
            f'{indent}<RegistryValue Type="expandable" Name="ModifyPath" Value="MsiExec.exe /X [ProductCode]" />\n'
        )
        lines_new.append(
            f'{indent}<RegistryValue Type="integer" Id="NoModify" Value="1" />\n'
        )
        lines_new.append(
            f'{indent}<RegistryValue Type="expandable" Name="UninstallString" Value="MsiExec.exe /X [ProductCode]" />\n'
        )

        major, minor, build = g_version.split(".")
        lines_new.append(
            f'{indent}<RegistryValue Type="string" Name="Version" Value="{g_version}" />\n'
        )
        lines_new.append(
            f'{indent}<RegistryValue Type="integer" Name="VersionMajor" Value="{major}" />\n'
        )
        lines_new.append(
            f'{indent}<RegistryValue Type="integer" Name="VersionMinor" Value="{minor}" />\n'
        )
        lines_new.append(
            f'{indent}<RegistryValue Type="integer" Name="VersionBuild" Value="{build}" />\n'
        )

        lines_new.append(
            f'{indent}<RegistryValue Type="integer" Name="WindowsInstaller" Value="1" />\n'
        )
        for k, v in g_arpsystemcomponent.items():
            if "v" in v:
                t = v["t"] if "t" in v is None else "string"
                lines_new.append(
                    f'{indent}<RegistryValue Type="{t}" Name="{k}" Value="{v["v"]}" />\n'
                )

        for i, line in enumerate(lines_new):
            lines.insert(index_start + i + 1, line)
        return lines

    return gen_content_between_tags(
        "Package/Components/Regs.wxs",
        "<!--$ArpStart$-->",
        "<!--$ArpEnd$-->",
        func,
    )


def gen_custom_ARPSYSTEMCOMPONENT(args, dist_dir):
    try:
        custom_arp = json.loads(args.custom_arp)
        g_arpsystemcomponent.update(custom_arp)
    except json.JSONDecodeError as e:
        print(f"Failed to decode custom arp: {e}")
        return False

    if args.arp:
        return gen_custom_ARPSYSTEMCOMPONENT_True(args, dist_dir)
    else:
        return gen_custom_ARPSYSTEMCOMPONENT_False(args)


def gen_content_between_tags(filename, tag_start, tag_end, func):
    target_file = Path(sys.argv[0]).parent.joinpath(filename)
    lines, index_start = read_lines_and_start_index(target_file, tag_start, tag_end)
    if lines is None:
        return False

    func(lines, index_start)

    with open(target_file, "w") as f:
        f.writelines(lines)

    return True


def init_global_vars(args):
    var_file = "../../src/version.rs"
    if not Path(var_file).exists():
        print(f"Error: {var_file} not found")
        return False

    with open(var_file, "r") as f:
        content = f.readlines()

    global g_version
    global g_build_date
    g_version = args.version.replace("-", ".")
    if g_version == "":
        # pub const VERSION: &str = "1.2.4";
        version_pattern = re.compile(r'.*VERSION: &str = "(.*)";.*')
        for line in content:
            match = version_pattern.match(line)
            if match:
                g_version = match.group(1)
                break
    if g_version == "":
        print(f"Error: version not found in {var_file}")
        return False

    # pub const BUILD_DATE: &str = "2024-04-08 23:11";
    build_date_pattern = re.compile(r'BUILD_DATE: &str = "(.*)";')
    for line in content:
        match = build_date_pattern.match(line)
        if match:
            g_build_date = match.group(1)
            break

    return True


def replace_component_guids_in_wxs():
    langs_dir = Path(sys.argv[0]).parent.joinpath("Package")
    for file_path in langs_dir.glob("**/*.wxs"):
        with open(file_path, "r") as f:
            lines = f.readlines()

        # <Component Id="Product.Registry.DefaultIcon" Guid="6DBF2690-0955-4C6A-940F-634DDA503F49">
        for i, line in enumerate(lines):
            match = re.search(r'Component.+Guid="([^"]+)"', line)
            if match:
                lines[i] = re.sub(r'Guid="[^"]+"', f'Guid="{uuid.uuid4()}"', line)

        with open(file_path, "w") as f:
            f.writelines(lines)


if __name__ == "__main__":
    parser = make_parser()
    args = parser.parse_args()

    app_name = args.app_name
    dist_dir = Path(sys.argv[0]).parent.joinpath(args.dist_dir).resolve()

    if not init_global_vars(args):
        sys.exit(-1)

    if not gen_pre_vars(args, dist_dir):
        sys.exit(-1)

    if app_name != "RustDesk":
        replace_component_guids_in_wxs()

    if not gen_upgrade_info():
        sys.exit(-1)

    if not gen_custom_ARPSYSTEMCOMPONENT(args, dist_dir):
        sys.exit(-1)

    if not gen_auto_component(app_name, dist_dir):
        sys.exit(-1)

    if not gen_custom_dialog_bitmaps():
        sys.exit(-1)

    replace_app_name_in_langs(args.app_name)
