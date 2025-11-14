#!/usr/bin/env python3

import requests
import argparse
import json


def check_response(response):
    """
    Check API response and handle errors.
    
    Two error cases:
    1. Status code is not 200 -> exit with error
    2. Response contains {"error": "xxx"} -> exit with error
    """
    if response.status_code != 200:
        print(f"Error: HTTP {response.status_code}: {response.text}")
        exit(1)
    
    # Check for {"error": "xxx"} in response
    if response.text and response.text.strip():
        try:
            json_data = response.json()
            if isinstance(json_data, dict) and "error" in json_data:
                print(f"Error: {json_data['error']}")
                exit(1)
            return json_data
        except ValueError:
            return response.text
    
    return None


def headers_with(token):
    return {"Authorization": f"Bearer {token}", "Content-Type": "application/json"}


# ---------- Device Group APIs ----------

def list_groups(url, token, name=None, page_size=50):
    headers = headers_with(token)
    params = {"pageSize": page_size}
    if name:
        params["name"] = name
    data, current = [], 1
    while True:
        params["current"] = current
        r = requests.get(f"{url}/api/device-groups", headers=headers, params=params)
        if r.status_code != 200:
            print(f"Error: HTTP {r.status_code} - {r.text}")
            exit(1)
        res = r.json()
        if "error" in res:
            print(f"Error: {res['error']}")
            exit(1)
        rows = res.get("data", [])
        data.extend(rows)
        total = res.get("total", 0)
        current += page_size
        if len(rows) < page_size or current > total:
            break
    return data


def get_group_by_name(url, token, name):
    groups = list_groups(url, token, name)
    for g in groups:
        if str(g.get("name")) == name:
            return g
    return None


def create_group(url, token, name, note=None, accessed_from=None):
    headers = headers_with(token)
    payload = {"name": name}
    if note:
        payload["note"] = note
    if accessed_from:
        payload["allowed_incomings"] = accessed_from
    r = requests.post(f"{url}/api/device-groups", headers=headers, json=payload)
    return check_response(r)


def update_group(url, token, name, new_name=None, note=None, accessed_from=None):
    headers = headers_with(token)
    g = get_group_by_name(url, token, name)
    if not g:
        print(f"Error: Group '{name}' not found")
        exit(1)
    guid = g.get("guid")
    payload = {}
    if new_name is not None:
        payload["name"] = new_name
    if note is not None:
        payload["note"] = note
    if accessed_from is not None:
        payload["allowed_incomings"] = accessed_from
    r = requests.patch(f"{url}/api/device-groups/{guid}", headers=headers, json=payload)
    check_response(r)
    return "Success"


def delete_groups(url, token, names):
    headers = headers_with(token)
    if isinstance(names, str):
        names = [names]
    for n in names:
        g = get_group_by_name(url, token, n)
        if not g:
            print(f"Error: Group '{n}' not found")
            exit(1)
        guid = g.get("guid")
        r = requests.delete(f"{url}/api/device-groups/{guid}", headers=headers)
        check_response(r)
    return "Success"


# ---------- Device group assign APIs (name -> guid) ----------

def view_devices(url, token, group_name=None, id=None, device_name=None, 
                 user_name=None, device_username=None, page_size=50):
    """View devices in a device group with filters"""
    headers = headers_with(token)
    
    # Separate exact match and fuzzy match params
    params = {}
    fuzzy_params = {
        "id": id,
        "device_name": device_name,
        "user_name": user_name,
        "device_username": device_username,
    }
    
    # Add device_group_name without wildcard (exact match)
    if group_name:
        params["device_group_name"] = group_name
    
    # Add wildcard for fuzzy search to other params
    for k, v in fuzzy_params.items():
        if v is not None:
            params[k] = "%" + v + "%" if (v != "-" and "%" not in v) else v
    
    params["pageSize"] = page_size
    
    data, current = [], 1
    while True:
        params["current"] = current
        r = requests.get(f"{url}/api/devices", headers=headers, params=params)
        if r.status_code != 200:
            return check_response(r)
        res = r.json()
        rows = res.get("data", [])
        data.extend(rows)
        total = res.get("total", 0)
        current += page_size
        if len(rows) < page_size or current > total:
            break
    return data


def add_devices(url, token, group_name, device_ids):
    headers = headers_with(token)
    g = get_group_by_name(url, token, group_name)
    if not g:
        return f"Group '{group_name}' not found"
    guid = g.get("guid")
    payload = device_ids if isinstance(device_ids, list) else [device_ids]
    r = requests.post(f"{url}/api/device-groups/{guid}", headers=headers, json=payload)
    return check_response(r)


def remove_devices(url, token, group_name, device_ids):
    headers = headers_with(token)
    g = get_group_by_name(url, token, group_name)
    if not g:
        return f"Group '{group_name}' not found"
    guid = g.get("guid")
    payload = device_ids if isinstance(device_ids, list) else [device_ids]
    r = requests.delete(f"{url}/api/device-groups/{guid}/devices", headers=headers, json=payload)
    return check_response(r)


def parse_rules(s):
    if not s:
        return None
    try:
        v = json.loads(s)
        if isinstance(v, list):
            # expect list of {"type": number, "name": string}
            return v
    except Exception:
        pass
    return None


def main():
    parser = argparse.ArgumentParser(description="Device Group manager")
    parser.add_argument("command", choices=[
        "view", "add", "update", "delete",
        "view-devices", "add-devices", "remove-devices"
    ], help=(
        "Command to execute. "
        "[view/add/update/delete/add-devices/remove-devices: require Device Group Permission] "
        "[view-devices: require Device Permission]"
    ))
    parser.add_argument("--url", required=True)
    parser.add_argument("--token", required=True)

    parser.add_argument("--name", help="Device group name (exact match)")
    parser.add_argument("--new-name", help="New device group name (for update)")
    parser.add_argument("--note", help="Note")

    parser.add_argument("--accessed-from", help="JSON array: '[{\"type\":0|2,\"name\":\"...\"}]' (0=User Group, 2=User)")

    parser.add_argument("--ids", help="Comma separated device IDs for add-devices/remove-devices")
    
    # Filters for view-devices command
    parser.add_argument("--id", help="Device ID filter (for view-devices)")
    parser.add_argument("--device-name", help="Device name filter (for view-devices)")
    parser.add_argument("--user-name", help="User name filter (owner of device, for view-devices)")
    parser.add_argument("--device-username", help="Device username filter (logged in user on device, for view-devices)")

    args = parser.parse_args()
    while args.url.endswith("/"): args.url = args.url[:-1]

    if args.command == "view":
        res = list_groups(args.url, args.token, args.name)
        print(json.dumps(res, indent=2))
    elif args.command == "add":
        if not args.name:
            print("Error: --name is required")
            exit(1)
        print(create_group(
            args.url, args.token, args.name, args.note,
            parse_rules(args.accessed_from)
        ))
    elif args.command == "update":
        if not args.name:
            print("Error: --name is required")
            exit(1)
        print(update_group(
            args.url, args.token, args.name, args.new_name, args.note,
            parse_rules(args.accessed_from)
        ))
    elif args.command == "delete":
        if not args.name:
            print("Error: --name is required (supports comma separated)")
            exit(1)
        names = [x.strip() for x in args.name.split(",") if x.strip()]
        print(delete_groups(args.url, args.token, names))
    elif args.command == "view-devices":
        res = view_devices(
            args.url, 
            args.token, 
            group_name=args.name,
            id=args.id,
            device_name=args.device_name,
            user_name=args.user_name,
            device_username=args.device_username
        )
        print(json.dumps(res, indent=2))
    elif args.command in ("add-devices", "remove-devices"):
        if not args.name or not args.ids:
            print("Error: --name and --ids are required for add/remove devices")
            exit(1)
        ids = [x.strip() for x in args.ids.split(",") if x.strip()]
        if args.command == "add-devices":
            print(add_devices(args.url, args.token, args.name, ids))
        else:
            print(remove_devices(args.url, args.token, args.name, ids))


if __name__ == "__main__":
    main()
