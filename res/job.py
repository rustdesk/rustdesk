#!/usr/bin/env python3

import requests
import os
import time
import argparse
import logging
import shutil
import zipfile

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(message)s [%(filename)s:%(lineno)d]",
    handlers=[logging.StreamHandler()],
)

# The URL of your Flask server
BASE_URL = os.getenv("BASE_URL") or "http://localhost:5000"

# The secret key for API authentication
SECRET_KEY = os.getenv("SECRET_KEY") or "worldpeace2024"

# The headers for API requests
HEADERS = {"Authorization": f"Bearer {SECRET_KEY}"}

SIGN_TIMEOUT = int(os.getenv("SIGN_TIMEOUT") or "30")
TIMEOUT = float(os.getenv("TIMEOUT") or "900")


def create(task_name, file_path=None):
    if file_path is None:
        response = requests.post(
            f"{BASE_URL}/tasks/{task_name}", timeout=TIMEOUT, headers=HEADERS
        )
    else:
        with open(file_path, "rb") as f:
            files = {"file": f}
            response = requests.post(
                f"{BASE_URL}/tasks/{task_name}",
                timeout=TIMEOUT,
                headers=HEADERS,
                files=files,
            )
    return get_json(response)


def upload_file(task_id, file_path):
    with open(file_path, "rb") as f:
        files = {"file": f}
        response = requests.post(
            f"{BASE_URL}/tasks/{task_id}/files",
            timeout=TIMEOUT,
            headers=HEADERS,
            files=files,
        )
    return get_json(response)


def get_status(task_id):
    response = requests.get(
        f"{BASE_URL}/tasks/{task_id}/status", timeout=TIMEOUT, headers=HEADERS
    )
    return get_json(response)


def download_files(task_id, output_dir, fn=None):
    response = requests.get(
        f"{BASE_URL}/tasks/{task_id}/files",
        timeout=TIMEOUT,
        headers=HEADERS,
        stream=True,
    )

    # Check if the request was successful
    if fn is None:
        fn = f"task_{task_id}_files.zip"
    if response.status_code == 200:
        # Save the file to the output directory
        with open(os.path.join(output_dir, fn), "wb") as f:
            for chunk in response.iter_content(chunk_size=1024):
                if chunk:
                    f.write(chunk)
    return response.ok


def download_one_file(task_id, file_id, output_dir):
    response = requests.get(
        f"{BASE_URL}/tasks/{task_id}/files/{file_id}",
        timeout=TIMEOUT,
        headers=HEADERS,
        stream=True,
    )

    # Check if the request was successful
    if response.status_code == 200:
        # Save the file to the output directory
        with open(os.path.join(output_dir, file_id), "wb") as f:
            for chunk in response.iter_content(chunk_size=1024):
                if chunk:
                    f.write(chunk)
    return response.ok


def fetch(tag=None):
    response = requests.get(
        f"{BASE_URL}/tasks/fetch_task" + ("?tag=%s" % tag if tag else ""),
        timeout=TIMEOUT,
        headers=HEADERS,
    )
    return get_json(response)


def update_status(task_id, status):
    response = requests.patch(
        f"{BASE_URL}/tasks/{task_id}/status",
        timeout=TIMEOUT,
        headers=HEADERS,
        json=status,
    )
    return get_json(response)


def delete_task(task_id):
    response = requests.delete(
        f"{BASE_URL}/tasks/{task_id}",
        timeout=TIMEOUT,
        headers=HEADERS,
    )
    return get_json(response)


def sign(file_path):
    res = create("sign", file_path)
    if res.ok:
        task_id = res.task_id

        # Poll the status every second
        while True:
            status = get_status(task_id)
            if status["status"] == "done":
                # Download the files
                download_files(task_id, "output")

                # Delete the task
                delete_task(task_id)

                break

            time.sleep(1)


def sign_one_file(file_path):
    logging.info(f"Signing {file_path}")
    res = create("sign", file_path)
    logging.info(f"Uploaded {file_path}")
    task_id = res["id"]
    n = 0
    while True:
        if n >= SIGN_TIMEOUT:
            delete_task(task_id)
            logging.error(f"Failed to sign {file_path}")
            break
        time.sleep(6)
        n += 1
        status = get_status(task_id)
        if status and status.get("state") == "done":
            download_one_file(
                task_id, os.path.basename(file_path), os.path.dirname(file_path)
            )
            delete_task(task_id)
            logging.info(f"Signed {file_path}")
            return True
    return False


def get_json(response):
    try:
        return response.json()
    except Exception as e:
        raise Exception(response.text)


SIGN_EXTENSIONS = [
    ".dll",
    ".exe",
    ".sys",
    ".vxd",
    ".msix",
    ".msixbundle",
    ".appx",
    ".appxbundle",
    ".msi",
    ".msp",
    ".msm",
    ".cab",
    ".ps1",
    ".psm1",
]


def sign_files(dir_path, only_ext=None):
    if only_ext:
        only_ext = only_ext.split(",")
        for i in range(len(only_ext)):
            if not only_ext[i].startswith("."):
                only_ext[i] = "." + only_ext[i]
    for root, dirs, files in os.walk(dir_path):
        for file in files:
            file_path = os.path.join(root, file)
            _, ext = os.path.splitext(file_path)
            if only_ext and ext not in only_ext:
                continue
            if ext in SIGN_EXTENSIONS:
                if not sign_one_file(file_path):
                    logging.error(f"Failed to sign {file_path}")
                    break


def main():
    parser = argparse.ArgumentParser(
        description="Command line interface for task operations."
    )
    subparsers = parser.add_subparsers(dest="command")

    # Create a parser for the "sign_one_file" command
    sign_one_file_parser = subparsers.add_parser(
        "sign_one_file", help="Sign a single file."
    )
    sign_one_file_parser.add_argument("file_path", help="The path of the file to sign.")

    # Create a parser for the "sign_files" command
    sign_files_parser = subparsers.add_parser(
        "sign_files", help="Sign all files in a directory."
    )
    sign_files_parser.add_argument(
        "dir_path", help="The path of the directory containing the files to sign."
    )
    sign_files_parser.add_argument(
        "only_ext", help="The file extension to sign.", default=None, nargs="?"
    )

    # Create a parser for the "fetch" command
    fetch_parser = subparsers.add_parser("fetch", help="Fetch a task.")

    # Create a parser for the "update_status" command
    update_status_parser = subparsers.add_parser(
        "update_status", help="Update the status of a task."
    )
    update_status_parser.add_argument("task_id", help="The ID of the task to update.")
    update_status_parser.add_argument("status", help="The new status of the task.")

    # Create a parser for the "delete_task" command
    delete_task_parser = subparsers.add_parser("delete_task", help="Delete a task.")
    delete_task_parser.add_argument("task_id", help="The ID of the task to delete.")

    # Create a parser for the "create" command
    create_parser = subparsers.add_parser("create", help="Create a task.")
    create_parser.add_argument("task_name", help="The name of the task to create.")
    create_parser.add_argument(
        "file_path",
        help="The path of the file for the task.",
        default=None,
        nargs="?",
    )

    # Create a parser for the "upload_file" command
    upload_file_parser = subparsers.add_parser(
        "upload_file", help="Upload a file to a task."
    )
    upload_file_parser.add_argument(
        "task_id", help="The ID of the task to upload the file to."
    )
    upload_file_parser.add_argument("file_path", help="The path of the file to upload.")

    # Create a parser for the "get_status" command
    get_status_parser = subparsers.add_parser(
        "get_status", help="Get the status of a task."
    )
    get_status_parser.add_argument(
        "task_id", help="The ID of the task to get the status of."
    )

    # Create a parser for the "download_files" command
    download_files_parser = subparsers.add_parser(
        "download_files", help="Download files from a task."
    )
    download_files_parser.add_argument(
        "task_id", help="The ID of the task to download files from."
    )
    download_files_parser.add_argument(
        "output_dir", help="The directory to save the downloaded files to."
    )

    args = parser.parse_args()

    if args.command == "sign_one_file":
        sign_one_file(args.file_path)
    elif args.command == "sign_files":
        sign_files(args.dir_path, args.only_ext)
    elif args.command == "fetch":
        print(fetch())
    elif args.command == "update_status":
        print(update_status(args.task_id, args.status))
    elif args.command == "delete_task":
        print(delete_task(args.task_id))
    elif args.command == "create":
        print(create(args.task_name, args.file_path))
    elif args.command == "upload_file":
        print(upload_file(args.task_id, args.file_path))
    elif args.command == "get_status":
        print(get_status(args.task_id))
    elif args.command == "download_files":
        print(download_files(args.task_id, args.output_dir))


if __name__ == "__main__":
    main()
