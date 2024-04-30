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
    offline_days=None,
):
    headers = {"Authorization": f"Bearer {token}"}
    pageSize = 30
    params = {
        "id": id,
        "device_name": device_name,
        "user_name": user_name,
        "group_name": group_name,
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
                device["last_online"], "%Y-%m-%dT%H:%M:%S"
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


def main():
    parser = argparse.ArgumentParser(description="Device manager")
    parser.add_argument(
        "command",
        choices=["view", "disable", "enable", "delete"],
        help="Command to execute",
    )
    parser.add_argument("--url", required=True, help="URL of the API")
    parser.add_argument(
        "--token", required=True, help="Bearer token for authentication"
    )
    parser.add_argument("--id", help="Device ID")
    parser.add_argument("--device_name", help="Device name")
    parser.add_argument("--user_name", help="User name")
    parser.add_argument("--group_name", help="Group name")
    parser.add_argument(
        "--offline_days", type=int, help="Offline duration in days, e.g., 7"
    )

    args = parser.parse_args()

    devices = view(
        args.url,
        args.token,
        args.id,
        args.device_name,
        args.user_name,
        args.group_name,
        args.offline_days,
    )

    if args.command == "view":
        for device in devices:
            print(device)
    elif args.command == "disable":
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


if __name__ == "__main__":
    main()
