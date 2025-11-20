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


# ---------- Strategies APIs ----------

def list_strategies(url, token):
    """List all strategies"""
    headers = headers_with(token)
    r = requests.get(f"{url}/api/strategies", headers=headers)
    return check_response(r)


def get_strategy_by_guid(url, token, guid):
    """Get strategy by GUID"""
    headers = headers_with(token)
    r = requests.get(f"{url}/api/strategies/{guid}", headers=headers)
    return check_response(r)


def get_strategy_by_name(url, token, name):
    """Get strategy by name"""
    strategies = list_strategies(url, token)
    if not strategies:
        return None
    for s in strategies:
        if str(s.get("name")) == name:
            return s
    return None


def enable_strategy(url, token, name):
    """Enable a strategy"""
    headers = headers_with(token)
    strategy = get_strategy_by_name(url, token, name)
    if not strategy:
        print(f"Error: Strategy '{name}' not found")
        exit(1)
    guid = strategy.get("guid")
    r = requests.put(f"{url}/api/strategies/{guid}/status", headers=headers, json=True)
    check_response(r)
    return "Success"


def disable_strategy(url, token, name):
    """Disable a strategy"""
    headers = headers_with(token)
    strategy = get_strategy_by_name(url, token, name)
    if not strategy:
        print(f"Error: Strategy '{name}' not found")
        exit(1)
    guid = strategy.get("guid")
    r = requests.put(f"{url}/api/strategies/{guid}/status", headers=headers, json=False)
    check_response(r)
    return "Success"


def get_device_guid_by_id(url, token, device_id):
    """Get device GUID by device ID (exact match)"""
    headers = headers_with(token)
    params = {"id": device_id, "pageSize": 50}
    r = requests.get(f"{url}/api/devices", headers=headers, params=params)
    res = check_response(r)
    if not res:
        return None
    
    devices_data = res.get("data", []) if isinstance(res, dict) else res
    for d in devices_data:
        if d.get("id") == device_id:
            return d.get("guid")
    return None


def get_user_guid_by_name(url, token, name):
    """Get user GUID by exact name match"""
    headers = headers_with(token)
    params = {"name": name, "pageSize": 50}
    r = requests.get(f"{url}/api/users", headers=headers, params=params)
    res = check_response(r)
    if not res:
        return None
    
    users_data = res.get("data", []) if isinstance(res, dict) else res
    for u in users_data:
        if u.get("name") == name:
            return u.get("guid")
    return None


def get_device_group_guid_by_name(url, token, name):
    """Get device group GUID by exact name match"""
    headers = headers_with(token)
    params = {"pageSize": 50, "name": name}
    r = requests.get(f"{url}/api/device-groups", headers=headers, params=params)
    res = check_response(r)
    if not res:
        return None
    
    groups_data = res.get("data", []) if isinstance(res, dict) else res
    for g in groups_data:
        if g.get("name") == name:
            return g.get("guid")
    return None


def assign_strategy(url, token, strategy_name, peers=None, users=None, device_groups=None):
    """
    Assign strategy to peers, users, or device groups
    
    Args:
        strategy_name: Name of the strategy (or None to unassign)
        peers: List of device IDs or GUIDs
        users: List of user names or GUIDs
        device_groups: List of device group names or GUIDs
    """
    headers = headers_with(token)
    
    # Get strategy GUID if strategy_name is provided
    strategy_guid = None
    if strategy_name:
        strategy = get_strategy_by_name(url, token, strategy_name)
        if not strategy:
            print(f"Error: Strategy '{strategy_name}' not found")
            exit(1)
        strategy_guid = strategy.get("guid")
    
    # Convert device IDs to GUIDs
    peer_guids = []
    if peers:
        for peer in peers:
            # Check if it's already a GUID format
            if len(peer) == 36 and peer.count('-') == 4:
                peer_guids.append(peer)
            else:
                # Treat as device ID, look it up
                guid = get_device_guid_by_id(url, token, peer)
                if not guid:
                    print(f"Error: Device '{peer}' not found")
                    exit(1)
                peer_guids.append(guid)
    
    # Convert user names to GUIDs
    user_guids = []
    if users:
        for user in users:
            # Check if it's already a GUID format
            if len(user) == 36 and user.count('-') == 4:
                user_guids.append(user)
            else:
                # Treat as username, look it up
                guid = get_user_guid_by_name(url, token, user)
                if not guid:
                    print(f"Error: User '{user}' not found")
                    exit(1)
                user_guids.append(guid)
    
    # Convert device group names to GUIDs
    device_group_guids = []
    if device_groups:
        for dg in device_groups:
            # Check if it's already a GUID format
            if len(dg) == 36 and dg.count('-') == 4:
                device_group_guids.append(dg)
            else:
                # Treat as device group name, look it up
                guid = get_device_group_guid_by_name(url, token, dg)
                if not guid:
                    print(f"Error: Device group '{dg}' not found")
                    exit(1)
                device_group_guids.append(guid)
    
    # Build payload
    payload = {}
    if strategy_guid:
        payload["strategy"] = strategy_guid
    
    payload["peers"] = peer_guids
    payload["users"] = user_guids
    payload["groups"] = device_group_guids
    
    r = requests.post(f"{url}/api/strategies/assign", headers=headers, json=payload)
    check_response(r)


def main():
    parser = argparse.ArgumentParser(description="Strategy manager")
    parser.add_argument("command", choices=[
        "list", "view", "enable", "disable", "assign", "unassign"
    ])
    parser.add_argument("--url", required=True, help="Server URL")
    parser.add_argument("--token", required=True, help="API token")

    parser.add_argument("--name", help="Strategy name (for view/enable/disable/assign commands)")
    parser.add_argument("--guid", help="Strategy GUID (for view command, alternative to --name)")
    
    # For assign/unassign commands
    parser.add_argument("--peers", help="Comma separated device IDs or GUIDs (requires Device Permission:r)")
    parser.add_argument("--users", help="Comma separated user names or GUIDs (requires User Permission:r)")
    parser.add_argument("--device-groups", help="Comma separated device group names or GUIDs (requires Device Group Permission:r)")

    args = parser.parse_args()
    while args.url.endswith("/"): args.url = args.url[:-1]

    if args.command == "list":
        res = list_strategies(args.url, args.token)
        print(json.dumps(res, indent=2))
    
    elif args.command == "view":
        if args.guid:
            res = get_strategy_by_guid(args.url, args.token, args.guid)
            print(json.dumps(res, indent=2))
        elif args.name:
            strategy = get_strategy_by_name(args.url, args.token, args.name)
            if not strategy:
                print(f"Error: Strategy '{args.name}' not found")
                exit(1)
            # Get full details by GUID
            guid = strategy.get("guid")
            res = get_strategy_by_guid(args.url, args.token, guid)
            print(json.dumps(res, indent=2))
        else:
            print("Error: --name or --guid is required for view command")
            exit(1)
    
    elif args.command == "enable":
        if not args.name:
            print("Error: --name is required")
            exit(1)
        print(enable_strategy(args.url, args.token, args.name))
    
    elif args.command == "disable":
        if not args.name:
            print("Error: --name is required")
            exit(1)
        print(disable_strategy(args.url, args.token, args.name))
    
    elif args.command == "assign":
        if not args.name:
            print("Error: --name is required")
            exit(1)
        if not args.peers and not args.users and not args.device_groups:
            print("Error: at least one of --peers, --users, or --device-groups is required")
            exit(1)
        
        peers = [x.strip() for x in args.peers.split(",") if x.strip()] if args.peers else None
        users = [x.strip() for x in args.users.split(",") if x.strip()] if args.users else None
        device_groups = [x.strip() for x in args.device_groups.split(",") if x.strip()] if args.device_groups else None
        
        assign_strategy(args.url, args.token, args.name, peers=peers, users=users, device_groups=device_groups)
        count = (len(peers) if peers else 0) + (len(users) if users else 0) + (len(device_groups) if device_groups else 0)
        print(f"Success: Assigned strategy '{args.name}' to {count} target(s)")
    
    elif args.command == "unassign":
        if not args.peers and not args.users and not args.device_groups:
            print("Error: at least one of --peers, --users, or --device-groups is required")
            exit(1)
        
        peers = [x.strip() for x in args.peers.split(",") if x.strip()] if args.peers else None
        users = [x.strip() for x in args.users.split(",") if x.strip()] if args.users else None
        device_groups = [x.strip() for x in args.device_groups.split(",") if x.strip()] if args.device_groups else None
        
        assign_strategy(args.url, args.token, None, peers=peers, users=users, device_groups=device_groups)
        count = (len(peers) if peers else 0) + (len(users) if users else 0) + (len(device_groups) if device_groups else 0)
        print(f"Success: Unassigned strategy from {count} target(s)")


if __name__ == "__main__":
    main()
