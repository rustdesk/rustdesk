#!/usr/bin/env python3

import requests
import argparse
from datetime import datetime, timedelta


def check_response(response):
    """
    Check API response and handle errors properly.
    Exit with code 1 if there's an error.
    """
    if response.status_code != 200:
        print(f"Error: HTTP {response.status_code}: {response.text}")
        exit(1)
    
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
        if response.status_code != 200:
            print(f"Error: HTTP {response.status_code} - {response.text}")
            exit(1)
        
        response_json = response.json()
        if "error" in response_json:
            print(f"Error: {response_json['error']}")
            exit(1)

        data = response_json.get("data", [])
        users.extend(data)

        total = response_json.get("total", 0)
        current += pageSize
        if len(data) < pageSize or current > total:
            break

    return users


def disable(url, token, guid, name):
    print("Disable", name)
    headers = {"Authorization": f"Bearer {token}"}
    response = requests.post(f"{url}/api/users/{guid}/disable", headers=headers)
    check_response(response)


def enable(url, token, guid, name):
    print("Enable", name)
    headers = {"Authorization": f"Bearer {token}"}
    response = requests.post(f"{url}/api/users/{guid}/enable", headers=headers)
    check_response(response)


def delete_user(url, token, guid, name):
    print("Delete", name)
    headers = {"Authorization": f"Bearer {token}"}
    response = requests.delete(f"{url}/api/users/{guid}", headers=headers)
    check_response(response)


def new_user(url, token, name, password, group_name=None, email=None, note=None):
    """Create a new user"""
    headers = {"Authorization": f"Bearer {token}", "Content-Type": "application/json"}
    payload = {
        "name": name,
        "password": password,
    }
    if group_name:
        payload["group_name"] = group_name
    if email:
        payload["email"] = email
    if note:
        payload["note"] = note
    response = requests.post(f"{url}/api/users", headers=headers, json=payload)
    check_response(response)


def invite_user(url, token, email, name, group_name=None, note=None):
    """Invite a user by email"""
    headers = {"Authorization": f"Bearer {token}", "Content-Type": "application/json"}
    payload = {
        "email": email,
        "name": name,
    }
    if group_name:
        payload["group_name"] = group_name
    if note:
        payload["note"] = note
    response = requests.post(f"{url}/api/users/invite", headers=headers, json=payload)
    check_response(response)


def enable_2fa_enforce(url, token, user_guids, base_url):
    """Enable 2FA enforcement for users"""
    headers = {"Authorization": f"Bearer {token}", "Content-Type": "application/json"}
    payload = {
        "user_guids": user_guids if isinstance(user_guids, list) else [user_guids],
        "enforce": True,
        "url": base_url
    }
    response = requests.put(f"{url}/api/users/tfa/totp/enforce", headers=headers, json=payload)
    check_response(response)


def disable_2fa_enforce(url, token, user_guids, base_url=""):
    """Disable 2FA enforcement for users"""
    headers = {"Authorization": f"Bearer {token}", "Content-Type": "application/json"}
    payload = {
        "user_guids": user_guids if isinstance(user_guids, list) else [user_guids],
        "enforce": False,
        "url": base_url
    }
    response = requests.put(f"{url}/api/users/tfa/totp/enforce", headers=headers, json=payload)
    check_response(response)


def disable_email_verification(url, token, user_guids):
    """Disable email login verification for users"""
    headers = {"Authorization": f"Bearer {token}", "Content-Type": "application/json"}
    payload = {
        "user_guids": user_guids if isinstance(user_guids, list) else [user_guids],
        "type": "email"
    }
    response = requests.put(f"{url}/api/users/disable_login_verification", headers=headers, json=payload)
    check_response(response)


def reset_2fa(url, token, user_guids):
    """Reset 2FA for users"""
    headers = {"Authorization": f"Bearer {token}", "Content-Type": "application/json"}
    payload = {
        "user_guids": user_guids if isinstance(user_guids, list) else [user_guids],
        "type": "2fa"
    }
    response = requests.put(f"{url}/api/users/disable_login_verification", headers=headers, json=payload)
    check_response(response)


def force_logout(url, token, user_guids):
    """Force logout users"""
    headers = {"Authorization": f"Bearer {token}", "Content-Type": "application/json"}
    payload = {
        "user_guids": user_guids if isinstance(user_guids, list) else [user_guids],
    }
    response = requests.post(f"{url}/api/users/force-logout", headers=headers, json=payload)
    check_response(response)


def main():
    parser = argparse.ArgumentParser(description="User manager")
    parser.add_argument(
        "command",
        choices=["view", "disable", "enable", "delete", "new", "invite",
                 "enable-2fa-enforce", "disable-2fa-enforce", 
                 "disable-email-verification", "reset-2fa", "force-logout"],
        help="Command to execute",
    )
    parser.add_argument("--url", required=True, help="URL of the API")
    parser.add_argument(
        "--token", required=True, help="Bearer token for authentication"
    )
    parser.add_argument("--name", help="User name")
    parser.add_argument("--group_name", help="Group name (for filtering in view, or for new/invite command)")
    parser.add_argument("--password", help="User password (for new command)")
    parser.add_argument("--email", help="User email (for invite command)")
    parser.add_argument("--note", help="User note (for new/invite command)")
    parser.add_argument("--web-console-url", help="Web console URL (for 2FA enforce commands)")

    args = parser.parse_args()

    while args.url.endswith("/"): args.url = args.url[:-1]

    if args.command == "new":
        if not args.name or not args.password or not args.group_name:
            print("Error: --name and --password and --group_name are required for new command")
            exit(1)
        new_user(args.url, args.token, args.name, args.password, args.group_name, args.email, args.note)
        print("Success: User created")
        return
    
    if args.command == "invite":
        if not args.email or not args.name or not args.group_name:
            print("Error: --email and --name and --group_name are required for invite command")
            exit(1)
        invite_user(args.url, args.token, args.email, args.name, args.group_name, args.note)
        print("Success: Invitation sent")
        return

    users = view(
        args.url,
        args.token,
        args.name,
        args.group_name,
    )

    if args.command == "view":
        if len(users) == 0:
            print("Found 0 users")
        else:
            for user in users:
                print(user)
    elif args.command in ["disable", "enable", "delete", "enable-2fa-enforce", 
                           "disable-2fa-enforce", "disable-email-verification", "reset-2fa", "force-logout"]:
        if len(users) == 0:
            print("Found 0 users")
            return
        
        # Check if we need user confirmation for multiple users
        if len(users) > 1:
            print(f"Found {len(users)} users. Do you want to proceed with {args.command} operation on the users? (Y/N)")
            confirmation = input("Type 'Y' to confirm: ").strip()
            if confirmation.upper() != 'Y':
                print("Operation cancelled.")
                return
        
        if args.command == "disable":
            for user in users:
                disable(args.url, args.token, user["guid"], user["name"])
                print("Success")
        elif args.command == "enable":
            for user in users:
                enable(args.url, args.token, user["guid"], user["name"])
                print("Success")
        elif args.command == "delete":
            for user in users:
                delete_user(args.url, args.token, user["guid"], user["name"])
                print("Success")
        elif args.command == "enable-2fa-enforce":
            if not args.web_console_url:
                print("Error: --web-console-url is required for enable-2fa-enforce")
                exit(1)
            user_guids = [user["guid"] for user in users]
            enable_2fa_enforce(args.url, args.token, user_guids, args.web_console_url)
            print(f"Success: Enabled 2FA enforcement for {len(users)} user(s)")
        elif args.command == "disable-2fa-enforce":
            user_guids = [user["guid"] for user in users]
            web_url = args.web_console_url or ""
            disable_2fa_enforce(args.url, args.token, user_guids, web_url)
            print(f"Success: Disabled 2FA enforcement for {len(users)} user(s)")
        elif args.command == "disable-email-verification":
            user_guids = [user["guid"] for user in users]
            disable_email_verification(args.url, args.token, user_guids)
            print(f"Success: Disabled email verification for {len(users)} user(s)")
        elif args.command == "reset-2fa":
            user_guids = [user["guid"] for user in users]
            reset_2fa(args.url, args.token, user_guids)
            print(f"Success: Reset 2FA for {len(users)} user(s)")
        elif args.command == "force-logout":
            user_guids = [user["guid"] for user in users]
            force_logout(args.url, args.token, user_guids)
            print(f"Success: Force logout for {len(users)} user(s)")


if __name__ == "__main__":
    main()
