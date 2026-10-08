#!/usr/bin/env python3

import argparse
import json

import requests


STATUSES = {
    "disabled": 0,
    "enabled": 1,
}


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


def list_roles(url, token, name=None, status=None, page_size=50):
    params = {"pageSize": page_size}
    if name is not None:
        params["name"] = name
    if status is not None:
        params["status"] = STATUSES[status]

    roles = []
    current = 0
    while True:
        current += 1
        params["current"] = current
        response = requests.get(
            f"{url}/api/control-roles", headers=headers_with(token), params=params
        )
        data = check_response(response)
        if not isinstance(data, dict):
            print("Error: Unexpected response while listing control roles")
            exit(1)
        rows = data.get("data", [])
        for role in rows:
            role.pop("info", None)
        roles.extend(rows)
        total = data.get("total", 0)
        if len(rows) < page_size or current * page_size >= total:
            break
    return roles


def get_role(url, token, name=None, guid=None):
    if guid:
        response = requests.get(
            f"{url}/api/control-roles/{guid}", headers=headers_with(token)
        )
        role = check_response(response)
        if isinstance(role, dict):
            role.pop("info", None)
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
    print(f"Error: Control role '{target}' not found")
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


def create_role(url, token, name, note=None):
    payload = {"name": name}
    if note is not None:
        payload["note"] = note
    response = requests.post(
        f"{url}/api/control-roles", headers=headers_with(token), json=payload
    )
    check_response(response)


def update_role(url, token, guid, new_name=None, note=None):
    payload = {}
    if new_name is not None:
        payload["name"] = new_name
    if note is not None:
        payload["note"] = note
    response = requests.put(
        f"{url}/api/control-roles/{guid}", headers=headers_with(token), json=payload
    )
    check_response(response)


def delete_roles(url, token, guids):
    response = requests.delete(
        f"{url}/api/control-roles",
        headers=headers_with(token),
        json={"guids": guids},
    )
    check_response(response)


def set_status(url, token, guids, disable):
    response = requests.put(
        f"{url}/api/control-roles/enable",
        headers=headers_with(token),
        json={"guids": guids, "disable": disable},
    )
    check_response(response)


def change_users(url, token, guid, users, remove=False):
    if remove:
        endpoint = f"{url}/api/control-roles/users"
        response = requests.delete(
            endpoint,
            headers=headers_with(token),
            json={"user_guids": users},
        )
    else:
        endpoint = f"{url}/api/control-roles/{guid}/users"
        response = requests.post(
            endpoint,
            headers=headers_with(token),
            json={"user_guids": users},
        )
    check_response(response)


def view_users(url, token, role_guid, page_size=50):
    params = {"control_role_guid": role_guid, "pageSize": page_size}
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
    parser = argparse.ArgumentParser(
        description="Control role manager (configure control permissions in the web console)"
    )
    parser.add_argument(
        "command",
        choices=[
            "view",
            "add",
            "update",
            "delete",
            "enable",
            "disable",
            "view-users",
            "assign-users",
            "remove-users",
        ],
    )
    parser.add_argument("--url", required=True, help="Server URL")
    parser.add_argument("--token", required=True, help="API token")
    parser.add_argument("--name", help="Control role name")
    parser.add_argument("--guid", help="Control role GUID")
    parser.add_argument("--new-name", help="New control role name")
    parser.add_argument("--note", help="Role note; use an empty value to clear it")
    parser.add_argument("--status", choices=STATUSES, help="Status filter for view")
    parser.add_argument("--users", help="Comma-separated user names or GUIDs")
    args = parser.parse_args()
    args.url = args.url.rstrip("/")

    if args.command == "view":
        if args.guid:
            result = resolve_role(args.url, args.token, guid=args.guid)
        else:
            result = list_roles(args.url, args.token, args.name, args.status)
        print(json.dumps(result, indent=2))
        return

    if args.command == "add":
        if not args.name:
            parser.error("--name is required for add")
        create_role(args.url, args.token, args.name, args.note)
        print(f"Success: Created control role '{args.name}'")
        return

    if args.command == "remove-users":
        users = split_csv(args.users)
        if not users:
            parser.error("--users is required for remove-users")
        user_guids = resolve_users(args.url, args.token, users)
        change_users(args.url, args.token, None, user_guids, remove=True)
        print("Success: Removed users from their control roles")
        return

    require_role_target(parser, args)
    role = resolve_role(args.url, args.token, args.name, args.guid)
    role_guid = role.get("guid")
    role_name = role.get("name")

    if args.command == "update":
        if args.new_name is None and args.note is None:
            parser.error("--new-name or --note is required for update")
        update_role(args.url, args.token, role_guid, args.new_name, args.note)
        print(f"Success: Updated control role '{role_name}'")
    elif args.command == "delete":
        delete_roles(args.url, args.token, [role_guid])
        print(f"Success: Deleted control role '{role_name}'")
    elif args.command in ("enable", "disable"):
        disable = args.command == "disable"
        set_status(args.url, args.token, [role_guid], disable)
        print(f"Success: {args.command.title()}d control role '{role_name}'")
    elif args.command == "view-users":
        print(json.dumps(view_users(args.url, args.token, role_guid), indent=2))
    elif args.command == "assign-users":
        users = split_csv(args.users)
        if not users:
            parser.error("--users is required for assign-users")
        user_guids = resolve_users(args.url, args.token, users)
        change_users(args.url, args.token, role_guid, user_guids)
        print(f"Success: Assigned users to control role '{role_name}'")


if __name__ == "__main__":
    main()
