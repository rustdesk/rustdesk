#!/usr/bin/env python3

import argparse
import json

import requests


ROLE_TYPES = {
    "global": 1,
    "individual": 2,
    "group": 3,
}

PERMISSION_IDS = {
    "users.view": 0x0101,
    "users.create": 0x0103,
    "users.invite": 0x0104,
    "users.delete": 0x0105,
    "users.enable_disable": 0x0106,
    "users.edit_email": 0x0107,
    "users.edit_password": 0x0108,
    "users.edit_note": 0x0109,
    "users.manage_2fa": 0x010A,
    "users.force_logout": 0x010B,
    "users.change_group": 0x010C,
    "users.change_strategy": 0x010D,
    "users.change_control_role": 0x010E,
    "users.edit_display_name": 0x010F,
    "devices.view": 0x0201,
    "devices.enable_disable": 0x0203,
    "devices.delete": 0x0204,
    "devices.edit_info": 0x0205,
    "devices.assign_to_user": 0x0206,
    "devices.change_group": 0x0207,
    "devices.change_strategy": 0x0208,
    "user_groups.view": 0x0301,
    "user_groups.edit": 0x0302,
    "device_groups.view": 0x0401,
    "device_groups.edit": 0x0402,
    "device_groups.change_strategy": 0x0403,
    "audits.view": 0x0501,
    "audits.edit": 0x0502,
    "strategies.view": 0x0601,
    "strategies.edit": 0x0602,
    "custom_clients.view": 0x0701,
    "custom_clients.edit": 0x0702,
    "control_roles.view": 0x0801,
    "control_roles.edit": 0x0802,
}

PERMISSION_NAMES = {permission_id: name for name, permission_id in PERMISSION_IDS.items()}


def check_response(response):
    if response.status_code != 200:
        print(f"Error: HTTP {response.status_code}: {response.text}")
        exit(1)

    if response.text and response.text.strip():
        try:
            data = response.json()
        except ValueError:
            return response.text
        if isinstance(data, dict) and "error" in data:
            print(f"Error: {data['error']}")
            exit(1)
        return data
    return None


def headers_with(token):
    return {"Authorization": f"Bearer {token}", "Content-Type": "application/json"}


def split_csv(value):
    if value is None:
        return None
    return [item.strip() for item in value.split(",") if item.strip()]


def parse_permissions(value):
    permissions = []
    for item in split_csv(value) or []:
        permission = PERMISSION_IDS.get(item.lower())
        if permission is None:
            try:
                permission = int(item, 0)
            except ValueError:
                print(f"Error: Invalid permission name or ID '{item}'")
                exit(1)
        if permission < 0 or permission > 65535:
            print(f"Error: Permission ID '{item}' is outside the 0-65535 range")
            exit(1)
        permissions.append(permission)
    return permissions


def format_role_permissions(role):
    permissions = role.get("permissions")
    if isinstance(permissions, list):
        role["permissions"] = [
            PERMISSION_NAMES.get(permission, permission) for permission in permissions
        ]
    return role


def list_roles(url, token, name=None, role_type=None, page_size=50):
    params = {"pageSize": page_size}
    if name is not None:
        params["name"] = name
    if role_type is not None:
        params["type"] = ROLE_TYPES[role_type]

    roles = []
    current = 0
    while True:
        current += 1
        params["current"] = current
        response = requests.get(
            f"{url}/api/admin-roles", headers=headers_with(token), params=params
        )
        data = check_response(response)
        if not isinstance(data, dict):
            print("Error: Unexpected response while listing admin roles")
            exit(1)
        rows = data.get("data", [])
        roles.extend(format_role_permissions(role) for role in rows)
        total = data.get("total", 0)
        if len(rows) < page_size or current * page_size >= total:
            break
    return roles


def get_role(url, token, name=None, guid=None):
    if guid:
        response = requests.get(
            f"{url}/api/admin-roles/{guid}", headers=headers_with(token)
        )
        role = check_response(response)
        if isinstance(role, dict):
            return format_role_permissions(role)
        return role

    roles = list_roles(url, token, name=name)
    for role in roles:
        if role.get("name") == name:
            return role
    return None


def resolve_role(url, token, name=None, guid=None):
    role = get_role(url, token, name=name, guid=guid)
    if role:
        return role
    target = guid if guid else name
    print(f"Error: Admin role '{target}' not found")
    exit(1)


def get_user_guid(url, token, name):
    response = requests.get(
        f"{url}/api/users",
        headers=headers_with(token),
        params={"name": name, "pageSize": 50, "current": 1},
    )
    data = check_response(response)
    users = data.get("data", []) if isinstance(data, dict) else []
    for user in users:
        if user.get("name") == name:
            return user.get("guid")
    return None


def resolve_users(url, token, users):
    guids = []
    for user in users:
        if len(user) == 36 and user.count("-") == 4:
            guids.append(user)
            continue
        guid = get_user_guid(url, token, user)
        if not guid:
            print(f"Error: User '{user}' not found")
            exit(1)
        guids.append(guid)
    return guids


def create_role(
    url,
    token,
    name,
    role_type,
    permissions,
    note=None,
    user_groups=None,
    device_groups=None,
    unassigned=None,
):
    payload = {
        "name": name,
        "type": ROLE_TYPES[role_type],
        "permissions": permissions,
    }
    if note is not None:
        payload["note"] = note
    if user_groups:
        payload["user_groups"] = user_groups
    if device_groups:
        payload["device_groups"] = device_groups
    if unassigned is not None:
        payload["unassigned"] = unassigned
    response = requests.post(
        f"{url}/api/admin-roles", headers=headers_with(token), json=payload
    )
    check_response(response)


def update_role(
    url,
    token,
    guid,
    new_name=None,
    note=None,
    permissions=None,
    user_groups=None,
    device_groups=None,
    unassigned=None,
):
    payload = {}
    if new_name is not None:
        payload["name"] = new_name
    if note is not None:
        payload["note"] = note
    if permissions is not None:
        payload["permissions"] = permissions
    if user_groups is not None:
        payload["user_groups"] = user_groups
    if device_groups is not None:
        payload["device_groups"] = device_groups
    if unassigned is not None:
        payload["unassigned"] = unassigned
    response = requests.put(
        f"{url}/api/admin-roles/{guid}", headers=headers_with(token), json=payload
    )
    check_response(response)


def delete_roles(url, token, guids):
    response = requests.delete(
        f"{url}/api/admin-roles",
        headers=headers_with(token),
        json={"guids": guids},
    )
    check_response(response)


def change_users(url, token, guid, users, remove=False):
    method = requests.delete if remove else requests.post
    response = method(
        f"{url}/api/admin-roles/{guid}/users",
        headers=headers_with(token),
        json={"users": users},
    )
    check_response(response)


def view_users(url, token, role_guid, page_size=50):
    params = {"admin_role_guid": role_guid, "pageSize": page_size}
    users = []
    current = 0
    while True:
        current += 1
        params["current"] = current
        response = requests.get(
            f"{url}/api/users", headers=headers_with(token), params=params
        )
        data = check_response(response)
        if not isinstance(data, dict):
            print("Error: Unexpected response while listing users")
            exit(1)
        rows = data.get("data", [])
        users.extend(rows)
        total = data.get("total", 0)
        if len(rows) < page_size or current * page_size >= total:
            break
    return users


def require_role_target(parser, args):
    if not args.name and not args.guid:
        parser.error("one of --name or --guid is required")


def main():
    parser = argparse.ArgumentParser(description="Admin role manager")
    parser.add_argument(
        "command",
        choices=["view", "add", "update", "delete", "view-users", "add-users", "remove-users"],
    )
    parser.add_argument("--url", required=True, help="Server URL")
    parser.add_argument("--token", required=True, help="API token")
    parser.add_argument("--name", help="Admin role name")
    parser.add_argument("--guid", help="Admin role GUID")
    parser.add_argument("--new-name", help="New admin role name")
    parser.add_argument("--note", help="Role note; use an empty value to clear it")
    parser.add_argument("--type", choices=ROLE_TYPES, help="Role type")
    parser.add_argument(
        "--permissions",
        help="Comma-separated permission names or numeric IDs; use an empty value to clear",
    )
    parser.add_argument(
        "--user-groups",
        help="Comma-separated user group names; use an empty value to clear",
    )
    parser.add_argument(
        "--device-groups",
        help="Comma-separated device group names; use an empty value to clear",
    )
    parser.add_argument("--users", help="Comma-separated user names or GUIDs")
    unassigned = parser.add_mutually_exclusive_group()
    unassigned.add_argument(
        "--unassigned", dest="unassigned", action="store_true", help="Include unassigned devices"
    )
    unassigned.add_argument(
        "--no-unassigned",
        dest="unassigned",
        action="store_false",
        help="Exclude unassigned devices",
    )
    parser.set_defaults(unassigned=None)
    args = parser.parse_args()
    args.url = args.url.rstrip("/")

    if args.command == "view":
        if args.guid:
            result = resolve_role(args.url, args.token, guid=args.guid)
        else:
            result = list_roles(args.url, args.token, args.name, args.type)
        print(json.dumps(result, indent=2))
        return

    if args.command == "add":
        if not args.name or not args.type or args.permissions is None:
            parser.error("--name, --type, and --permissions are required for add")
        if args.type != "group" and (
            args.user_groups is not None
            or args.device_groups is not None
            or args.unassigned is not None
        ):
            parser.error("group scope options can only be used with --type group")
        create_role(
            args.url,
            args.token,
            args.name,
            args.type,
            parse_permissions(args.permissions),
            args.note,
            split_csv(args.user_groups),
            split_csv(args.device_groups),
            args.unassigned,
        )
        print(f"Success: Created admin role '{args.name}'")
        return

    require_role_target(parser, args)
    role = resolve_role(args.url, args.token, args.name, args.guid)
    role_guid = role.get("guid")
    role_name = role.get("name")

    if args.command == "update":
        updates = [
            args.new_name,
            args.note,
            args.permissions,
            args.user_groups,
            args.device_groups,
            args.unassigned,
        ]
        if all(value is None for value in updates):
            parser.error("at least one update option is required")
        if role.get("type") != ROLE_TYPES["group"] and (
            args.user_groups is not None
            or args.device_groups is not None
            or args.unassigned is not None
        ):
            parser.error("group scope options can only be used with a group role")
        update_role(
            args.url,
            args.token,
            role_guid,
            args.new_name,
            args.note,
            parse_permissions(args.permissions) if args.permissions is not None else None,
            split_csv(args.user_groups),
            split_csv(args.device_groups),
            args.unassigned,
        )
        print(f"Success: Updated admin role '{role_name}'")
    elif args.command == "delete":
        delete_roles(args.url, args.token, [role_guid])
        print(f"Success: Deleted admin role '{role_name}'")
    elif args.command == "view-users":
        print(json.dumps(view_users(args.url, args.token, role_guid), indent=2))
    elif args.command in ("add-users", "remove-users"):
        users = split_csv(args.users)
        if not users:
            parser.error("--users is required for add-users and remove-users")
        user_guids = resolve_users(args.url, args.token, users)
        remove = args.command == "remove-users"
        change_users(args.url, args.token, role_guid, user_guids, remove=remove)
        action = "Removed users from" if remove else "Added users to"
        print(f"Success: {action} admin role '{role_name}'")


if __name__ == "__main__":
    main()
