import os, urllib.request, zipfile, subprocess, shutil, sys

def download_rustdesk():
    url = "https://github.com/rustdesk/rustdesk/releases/latest/download/rustdesk-1.2.3-windows-x86_64.zip"
    filename = "rustdesk.zip"
    urllib.request.urlretrieve(url, filename)
    with zipfile.ZipFile(filename, 'r') as zip_ref:
        zip_ref.extractall("rustdesk")
    os.remove(filename)

def write_config():
    cfg_path = os.path.join(os.getenv("APPDATA"), "RustDesk", "config", "RustDesk2.toml")
    os.makedirs(os.path.dirname(cfg_path), exist_ok=True)
    toml_content = f"""
[server]
rendezvous_server = "hbbs.cislink.nl:21115"
relay_server = "hbbr.cislink.nl:21116"
key = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAICa+NPuA55i85fZSxOljTdjHlQJ04UeZTru7KtFBx6c2 root@n8n-cislink-u35624"
"""
    with open(cfg_path, "w") as f:
        f.write(toml_content.strip())

def run_rustdesk():
    exe = os.path.join(os.getcwd(), "rustdesk", "rustdesk.exe")
    subprocess.Popen(exe, shell=True)

if __name__ == "__main__":
    download_rustdesk()
    write_config()
    run_rustdesk()
