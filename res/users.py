#!/usr/bin/env python3

import requests
import argparse
from datetime import datetime, timedelta


def view(
    url,
    token,
    name=None,
    group_name=None,
):
    headers = {"Authorization": f"Bearer {token}"}
    pageSize = 30
    params = {
        "name": name,
        "group_name": group_name,
    }

    params = {
        k: "%" + v + "%" if (v != "-" and "%" not in v) else v
        for k, v in params.items()
        if v is not None
    }
    params["pageSize"] = pageSize

    users = []

    current = 1

    while True:
        params["current"] = current
        response = requests.get(f"{url}/api/users", headers=headers, params=params)
        response_json = response.json()

        data = response_json.get("data", [])
        users.extend(data)

        total = response_json.get("total", 0)
        current += pageSize
        if len(data) < pageSize or current > total:
            break

    return users


def check(response):
    if response.status_code == 200:
        try:
            response_json = response.json()
            return response_json
        except ValueError:
            return response.text or "Success"
    else:
        return "Failed", response.status_code, response.text


def disable(url, token, guid, name):
    print("Disable", name)
    headers = {"Authorization": f"Bearer {token}"}
    response = requests.post(f"{url}/api/users/{guid}/disable", headers=headers)
    return check(response)


def enable(url, token, guid, name):
    print("Enable", name)
    headers = {"Authorization": f"Bearer {token}"}
    response = requests.post(f"{url}/api/users/{guid}/enable", headers=headers)
    return check(response)


def delete(url, token, guid, name):
    print("Delete", name)
    headers = {"Authorization": f"Bearer {token}"}
    response = requests.delete(f"{url}/api/users/{guid}", headers=headers)
    return check(response)


def main():
    parser = argparse.ArgumentParser(description="User manager")
    parser.add_argument(
        "command",
        choices=["view", "disable", "enable", "delete"],
        help="Command to execute",
    )
    parser.add_argument("--url", required=True, help="URL of the API")
    parser.add_argument(
        "--token", required=True, help="Bearer token for authentication"
    )
    parser.add_argument("--name", help="User name")
    parser.add_argument("--group_name", help="Group name")

    args = parser.parse_args()

    while args.url.endswith("/"): args.url = args.url[:-1]

    users = view(
        args.url,
        args.token,
        args.name,
        args.group_name,
    )

    if args.command == "view":
        for user in users:
            print(user)
    elif args.command == "disable":
        for user in users:
            response = disable(args.url, args.token, user["guid"], user["name"])
            print(response)
    elif args.command == "enable":
        for user in users:
            response = enable(args.url, args.token, user["guid"], user["name"])
            print(response)
    elif args.command == "delete":
        for user in users:
            response = delete(args.url, args.token, user["guid"], user["name"])
            print(response)


if __name__ == "__main__":
    main()
