#!/usr/bin/env python3
# -*- coding: utf-8 -*-

import json
import sys
import uuid
import argparse
import datetime
import subprocess
import re
import platform
from pathlib import Path
from itertools import chain
import shutil

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
        "v": "https://github.com/rustdesk/rustdesk",
    },
}

def default_revision_version():
    return int(datetime.datetime.now().timestamp() / 60)

def make_parser():
    parser = argparse.ArgumentParser(description="Msi preprocess script.")
    parser.add_argument(
        "-d",
        "--dist-dir",
        type=str,
        default="../../rustdesk",
        help="The dist directory to install.",
    )
    parser.add_argument(
        "--arp",
        action="store_true",
        help="Is ARPSYSTEMCOMPONENT",
        default=False,
    )
    parser.add_argument(
        "--custom-arp",
        type=str,
        default="{}",
        help='Custom arp properties, e.g. \'["Comments": {"msi": "ARPCOMMENTS", "v": "Remote control application."}]\'',
    )
    parser.add_argument(
        "-c", "--custom", action="store_true", help="Is custom client", default=False
    )
    parser.add_argument(
        "--conn-type",
        type=str,
        default="",
        help='Connection type, e.g. "incoming", "outgoing". Default is empty, means incoming-outgoing',
    )
    parser.add_argument(
        "--app-name", type=str, default="RustDesk", help="The app name."
    )
    parser.add_argument(
        "-v", "--version", type=str, default="", help="The app version."
    )
    parser.add_argument(
        "--revision-version", type=int, default=default_revision_version(), help="The revision version."
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
    with open(file_path, "r", encoding="utf-8") as f:
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
            f'{indent}<?define RegKeyInstall="$(var.RegKeyRoot)\\Install" ?>\n',
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
        with open(file_path, "r", encoding="utf-8") as f:
            lines = f.readlines()
        for i, line in enumerate(lines):
            lines[i] = line.replace("RustDesk", app_name)
        with open(file_path, "w", encoding="utf-8") as f:
            f.writelines(lines)

def replace_app_name_in_custom_actions(app_name):
    custion_actions_dir = Path(sys.argv[0]).parent.joinpath("CustomActions")
    for file_path in chain(custion_actions_dir.glob("*.cpp"), custion_actions_dir.glob("*.h")):
        with open(file_path, "r", encoding="utf-8") as f:
            lines = f.readlines()
        for i, line in enumerate(lines):
            line = re.sub(r"\bRustDesk\b", app_name, line)
            line = line.replace(f"{app_name} v4 Printer Driver", "RustDesk v4 Printer Driver")
            lines[i] = line
        with open(file_path, "w", encoding="utf-8") as f:
            f.writelines(lines)

def gen_upgrade_info():
    def func(lines, index_start):
        indent = g_indent_unit * 3

        vs = g_version.split(".")
        major = vs[0]
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
            f'{indent}<RegistryValue Type="string" Name="DisplayIcon" Value="[INSTALLFOLDER_INNER]{args.app_name}.exe" />\n'
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
            f'{indent}<RegistryValue Type="string" Name="InstallLocation" Value="[INSTALLFOLDER_INNER]" />\n'
        )
        lines_new.append(
            f'{indent}<RegistryValue Type="string" Name="InstallSource" Value="[InstallSource]" />\n'
        )
        lines_new.append(
            f'{indent}<RegistryValue Type="integer" Name="Language" Value="[ProductLanguage]" />\n'
        )

        # EstimatedSize in uninstall registry must be in KB.
        estimated_size_bytes = get_folder_size(dist_dir)
        estimated_size = max(1, (estimated_size_bytes + 1023) // 1024)
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
        lines_new.append(
            f'{indent}<RegistryValue Type="expandable" Name="QuietUninstallString" Value="MsiExec.exe /qn /X [ProductCode]" />\n'
        )

        vs = g_version.split(".")
        major, minor, build = vs[0], vs[1], vs[2]
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

def gen_conn_type(args):
    def func(lines, index_start):
        indent = g_indent_unit * 3

        lines_new = []
        if args.conn_type != "":
            lines_new.append(
                f"""{indent}<Property Id="CC_CONNECTION_TYPE" Value="{args.conn_type}" />\n"""
            )

        for i, line in enumerate(lines_new):
            lines.insert(index_start + i + 1, line)
        return lines

    return gen_content_between_tags(
        "Package/Fragments/AddRemoveProperties.wxs",
        "<!--$CustomClientPropsStart$-->",
        "<!--$CustomClientPropsEnd$-->",
        func,
    )

def gen_content_between_tags(filename, tag_start, tag_end, func):
    target_file = Path(sys.argv[0]).parent.joinpath(filename)
    lines, index_start = read_lines_and_start_index(target_file, tag_start, tag_end)
    if lines is None:
        return False

    func(lines, index_start)

    with open(target_file, "w", encoding="utf-8") as f:
        f.writelines(lines)

    return True


def prepare_resources():
    icon_src = Path(sys.argv[0]).parent.joinpath("../icon.ico")
    icon_dst = Path(sys.argv[0]).parent.joinpath("Package/Resources/icon.ico")
    if icon_src.exists():
        icon_dst.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy(icon_src, icon_dst)
        return True
    else:
        # unreachable
        print(f"Error: icon.ico not found in {icon_src}")
        return False


def init_global_vars(dist_dir, app_name, args):
    dist_app = dist_dir.joinpath(app_name + ".exe")

    def read_process_output(args):
        process = subprocess.Popen(
            f"{dist_app} {args}",
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            shell=True,
        )
        output, _ = process.communicate()
        return output.decode("utf-8").strip()

    global g_version
    global g_build_date
    g_version = args.version.replace("-", ".")
    if g_version == "":
        g_version = read_process_output("--version")
    version_pattern = re.compile(r"\d+\.\d+\.\d+.*")
    if not version_pattern.match(g_version):
        print(f"Error: version {g_version} not found in {dist_app}")
        return False
    if g_version.count(".") == 2:
        # https://github.com/dotnet/runtime/blob/5535e31a712343a63f5d7d796cd874e563e5ac14/src/libraries/System.Private.CoreLib/src/System/Version.cs
        if args.revision_version < 0 or args.revision_version > 2147483647:
            raise ValueError(f"Invalid revision version: {args.revision_version}")    
        g_version = f"{g_version}.{args.revision_version}"

    g_build_date = read_process_output("--build-date")
    build_date_pattern = re.compile(r"\d{4}-\d{2}-\d{2} \d{2}:\d{2}")
    if not build_date_pattern.match(g_build_date):
        print(f"Error: build date {g_build_date} not found in {dist_app}")
        return False

    return True


def update_license_file(app_name):
    if app_name == "RustDesk":
        return
    license_file = Path(sys.argv[0]).parent.joinpath("Package/License.rtf")
    with open(license_file, "r", encoding="utf-8") as f:
        license_content = f.read()
    license_content = license_content.replace("website rustdesk.com and other ", "")
    license_content = license_content.replace("RustDesk", app_name)
    license_content = re.sub("Purslane Ltd", app_name, license_content, flags=re.IGNORECASE)
    with open(license_file, "w", encoding="utf-8") as f:
        f.write(license_content)


def replace_component_guids_in_wxs():
    langs_dir = Path(sys.argv[0]).parent.joinpath("Package")
    for file_path in langs_dir.glob("**/*.wxs"):
        with open(file_path, "r", encoding="utf-8") as f:
            lines = f.readlines()

        # <Component Id="Product.Registry.DefaultIcon" Guid="6DBF2690-0955-4C6A-940F-634DDA503F49">
        for i, line in enumerate(lines):
            match = re.search(r'Component.+Guid="([^"]+)"', line)
            if match:
                lines[i] = re.sub(r'Guid="[^"]+"', f'Guid="{uuid.uuid4()}"', line)

        with open(file_path, "w", encoding="utf-8") as f:
            f.writelines(lines)


if __name__ == "__main__":
    parser = make_parser()
    args = parser.parse_args()

    app_name = args.app_name
    dist_dir = Path(sys.argv[0]).parent.joinpath(args.dist_dir).resolve()

    if not prepare_resources():
        sys.exit(-1)

    if not init_global_vars(dist_dir, app_name, args):
        sys.exit(-1)

    update_license_file(app_name)

    if not gen_pre_vars(args, dist_dir):
        sys.exit(-1)

    if app_name != "RustDesk":
        replace_component_guids_in_wxs()

    if not gen_upgrade_info():
        sys.exit(-1)

    if not gen_custom_ARPSYSTEMCOMPONENT(args, dist_dir):
        sys.exit(-1)

    if not gen_conn_type(args):
        sys.exit(-1)

    if not gen_auto_component(app_name, dist_dir):
        sys.exit(-1)

    if not gen_custom_dialog_bitmaps():
        sys.exit(-1)

    replace_app_name_in_langs(args.app_name)
    replace_app_name_in_custom_actions(args.app_name)
