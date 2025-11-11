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


# ---------- User Group APIs ----------

def list_groups(url, token, name=None, page_size=50):
    headers = headers_with(token)
    params = {"pageSize": page_size}
    if name:
        params["name"] = name
    data, current = [], 1
    while True:
        params["current"] = current
        r = requests.get(f"{url}/api/user-groups", headers=headers, params=params)
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


def create_group(url, token, name, note=None, accessed_from=None, access_to=None):
    headers = headers_with(token)
    payload = {"name": name}
    if note:
        payload["note"] = note
    if accessed_from:
        payload["allowed_incomings"] = accessed_from
    if access_to:
        payload["allowed_outgoings"] = access_to
    r = requests.post(f"{url}/api/user-groups", headers=headers, json=payload)
    return check_response(r)


def update_group(url, token, name, new_name=None, note=None, accessed_from=None, access_to=None):
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
    if access_to is not None:
        payload["allowed_outgoings"] = access_to
    r = requests.patch(f"{url}/api/user-groups/{guid}", headers=headers, json=payload)
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
        r = requests.delete(f"{url}/api/user-groups/{guid}", headers=headers)
        check_response(r)
    return "Success"


# ---------- User management in group ----------

def view_users(url, token, group_name=None, name=None, page_size=50):
    """View users in a user group with filters"""
    headers = headers_with(token)
    
    # Separate exact match and fuzzy match params
    params = {}
    fuzzy_params = {
        "name": name,
    }
    
    # Add group_name without wildcard (exact match)
    if group_name:
        params["group_name"] = group_name
    
    # Add wildcard for fuzzy search to other params
    for k, v in fuzzy_params.items():
        if v is not None:
            params[k] = "%" + v + "%" if (v != "-" and "%" not in v) else v
    
    params["pageSize"] = page_size
    
    data, current = [], 1
    while True:
        params["current"] = current
        r = requests.get(f"{url}/api/users", headers=headers, params=params)
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


def add_users(url, token, group_name, user_names):
    """Add users to a user group"""
    headers = headers_with(token)
    if isinstance(user_names, str):
        user_names = [user_names]
    
    # Get the user group guid
    g = get_group_by_name(url, token, group_name)
    if not g:
        print(f"Error: Group '{group_name}' not found")
        exit(1)
    guid = g.get("guid")
    
    # Get user GUIDs
    user_guids = []
    errors = []
    
    for user_name in user_names:
        # Get user by exact name match
        params = {"name": user_name, "pageSize": 50}
        r = requests.get(f"{url}/api/users", headers=headers, params=params)
        if r.status_code != 200:
            errors.append(f"{user_name}: HTTP {r.status_code}")
            continue
        
        users_data = r.json()
        users_list = users_data.get("data", [])
        user = None
        for u in users_list:
            if u.get("name") == user_name:
                user = u
                break
        
        if not user:
            errors.append(f"{user_name}: User not found")
            continue
        
        user_guids.append(user["guid"])
    
    if not user_guids:
        msg = "Error: No valid users found"
        if errors:
            msg += ". " + "; ".join(errors)
        print(msg)
        exit(1)
    
    # Add users to group using POST /api/user-groups/:guid
    r = requests.post(f"{url}/api/user-groups/{guid}", headers=headers, json=user_guids)
    check_response(r)
    
    success_msg = f"Success: Added {len(user_guids)} user(s) to group '{group_name}'"
    if errors:
        return success_msg + " (with errors: " + "; ".join(errors) + ")"
    return success_msg


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
    parser = argparse.ArgumentParser(description="User Group manager")
    parser.add_argument("command", choices=[
        "view", "add", "update", "delete",
        "view-users", "add-users"
    ], help=(
        "Command to execute. "
        "[view/add/update/delete/add-users: require User Group Permission] "
        "[view-users: require User Permission]"
    ))
    parser.add_argument("--url", required=True)
    parser.add_argument("--token", required=True)

    parser.add_argument("--name", help="User group name (exact match)")
    parser.add_argument("--new-name", help="New user group name (for update)")
    parser.add_argument("--note", help="Note")

    parser.add_argument("--accessed-from", help="JSON array: '[{\"type\":0|2,\"name\":\"...\"}]' (0=User Group, 2=User)")
    parser.add_argument("--access-to", help="JSON array: '[{\"type\":0|1,\"name\":\"...\"}]' (0=User Group, 1=Device Group)")

    parser.add_argument("--users", help="Comma separated usernames for add-users")
    
    # Filters for view-users command
    parser.add_argument("--user-name", help="User name filter (for view-users, supports fuzzy search)")

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
            parse_rules(args.accessed_from),
            parse_rules(args.access_to)
        ))
    elif args.command == "update":
        if not args.name:
            print("Error: --name is required")
            exit(1)
        print(update_group(
            args.url, args.token, args.name, args.new_name, args.note,
            parse_rules(args.accessed_from),
            parse_rules(args.access_to)
        ))
    elif args.command == "delete":
        if not args.name:
            print("Error: --name is required (supports comma separated)")
            exit(1)
        names = [x.strip() for x in args.name.split(",") if x.strip()]
        print(delete_groups(args.url, args.token, names))
    elif args.command == "view-users":
        res = view_users(
            args.url, 
            args.token, 
            group_name=args.name,
            name=args.user_name
        )
        print(json.dumps(res, indent=2))
    elif args.command == "add-users":
        if not args.name or not args.users:
            print("Error: --name and --users are required")
            exit(1)
        users = [x.strip() for x in args.users.split(",") if x.strip()]
        print(add_users(args.url, args.token, args.name, users))


if __name__ == "__main__":
    main()
