#!/usr/bin/env python3

import requests
import argparse
from datetime import datetime, timedelta


def view(
    url,
    token,
    id=None,
    device_name=None,
    user_name=None,
    group_name=None,
    device_group_name=None,
    offline_days=None,
):
    headers = {"Authorization": f"Bearer {token}"}
    pageSize = 30
    params = {
        "id": id,
        "device_name": device_name,
        "user_name": user_name,
        "group_name": group_name,
        "device_group_name": device_group_name,
    }

    params = {
        k: "%" + v + "%" if (v != "-" and "%" not in v) else v
        for k, v in params.items()
        if v is not None
    }
    params["pageSize"] = pageSize

    devices = []

    current = 1

    while True:
        params["current"] = current
        response = requests.get(f"{url}/api/devices", headers=headers, params=params)
        response_json = response.json()

        data = response_json.get("data", [])

        for device in data:
            if offline_days is None:
                devices.append(device)
                continue
            last_online = datetime.strptime(
                device["last_online"].split(".")[0], "%Y-%m-%dT%H:%M:%S"
            )  # assuming date is in this format
            if (datetime.utcnow() - last_online).days >= offline_days:
                devices.append(device)

        total = response_json.get("total", 0)
        current += pageSize
        if len(data) < pageSize or current > total:
            break

    return devices


def check(response):
    if response.status_code == 200:
        try:
            response_json = response.json()
            return response_json
        except ValueError:
            return response.text or "Success"
    else:
        return "Failed", response.status_code, response.text


def disable(url, token, guid, id):
    print("Disable", id)
    headers = {"Authorization": f"Bearer {token}"}
    response = requests.post(f"{url}/api/devices/{guid}/disable", headers=headers)
    return check(response)


def enable(url, token, guid, id):
    print("Enable", id)
    headers = {"Authorization": f"Bearer {token}"}
    response = requests.post(f"{url}/api/devices/{guid}/enable", headers=headers)
    return check(response)


def delete(url, token, guid, id):
    print("Delete", id)
    headers = {"Authorization": f"Bearer {token}"}
    response = requests.delete(f"{url}/api/devices/{guid}", headers=headers)
    return check(response)


def assign(url, token, guid, id, type, value):
    print("assign", id, type, value)
    valid_types = [
        "ab",
        "strategy_name",
        "user_name",
        "device_group_name",
        "note",
        "device_username",
        "device_name",
    ]
    if type not in valid_types:
        print(f"Invalid type, it must be one of: {', '.join(valid_types)}")
        return
    data = {"type": type, "value": value}
    headers = {"Authorization": f"Bearer {token}"}
    response = requests.post(
        f"{url}/api/devices/{guid}/assign", headers=headers, json=data
    )
    return check(response)


def main():
    parser = argparse.ArgumentParser(description="Device manager")
    parser.add_argument(
        "command",
        choices=["view", "disable", "enable", "delete", "assign"],
        help="Command to execute",
    )
    parser.add_argument("--url", required=True, help="URL of the API")
    parser.add_argument(
        "--token", required=True, help="Bearer token for authentication"
    )
    parser.add_argument("--id", help="Device ID")
    parser.add_argument("--device_name", help="Device name")
    parser.add_argument("--user_name", help="User name")
    parser.add_argument("--group_name", help="User group name")
    parser.add_argument("--device_group_name", help="Device group name")
    parser.add_argument(
        "--assign_to",
        help="<type>=<value>, e.g. user_name=mike, strategy_name=test, device_group_name=group1, note=note1, device_username=username1, device_name=name1, ab=ab1, ab=ab1,tag1,alias1,password1,note1"
    )
    parser.add_argument(
        "--offline_days", type=int, help="Offline duration in days, e.g., 7"
    )

    args = parser.parse_args()
    
    while args.url.endswith("/"): args.url = args.url[:-1]

    devices = view(
        args.url,
        args.token,
        args.id,
        args.device_name,
        args.user_name,
        args.group_name,
        args.device_group_name,
        args.offline_days,
    )

    if args.command == "view":
        for device in devices:
            print(device)
    elif args.command in ["disable", "enable", "delete", "assign"]:
        # Check if we need user confirmation for multiple devices
        if len(devices) > 1:
            print(f"Found {len(devices)} devices. Do you want to proceed with {args.command} operation on the devices? (Y/N)")
            confirmation = input("Type 'Y' to confirm: ").strip()
            if confirmation.upper() != 'Y':
                print("Operation cancelled.")
                return
        
        if args.command == "disable":
            for device in devices:
                response = disable(args.url, args.token, device["guid"], device["id"])
                print(response)
        elif args.command == "enable":
            for device in devices:
                response = enable(args.url, args.token, device["guid"], device["id"])
                print(response)
        elif args.command == "delete":
            for device in devices:
                response = delete(args.url, args.token, device["guid"], device["id"])
                print(response)
        elif args.command == "assign":
            if "=" not in args.assign_to:
                print("Invalid assign_to format, it must be <type>=<value>")
                return
            type, value = args.assign_to.split("=", 1)
            for device in devices:
                response = assign(
                    args.url, args.token, device["guid"], device["id"], type, value
                )
                print(response)


if __name__ == "__main__":
    main()
