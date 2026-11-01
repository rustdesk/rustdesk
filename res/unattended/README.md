# Unattended access

An installed RustDesk runs as a background service. It starts at boot, works at
the login screen, and keeps accepting connections whether or not anyone opens
the RustDesk window. What a client needs for you to reach them without them
doing anything is:

1. RustDesk **installed** (not the portable or quick-support exe they just run);
2. a **permanent password** set;
3. you knowing their **ID** (and your server, if you self-host).

The scripts here do all three in one step and print the connection info.

| Script | Platform |
| --- | --- |
| `install-windows.ps1` | Windows 10/11, elevated PowerShell |
| `install-linux.sh` | Linux with systemd, run as root |

Both scripts:

* install the package you point them at (path or `https://` URL);
* apply your server config, if you pass one;
* set a permanent password (a random 16-character one if you don't pass one);
* make sure password logins are accepted (switches "click to accept only" to
  password mode, and "one-time password only" to both passwords);
* print the computer name, ID, password and server;
* optionally write that info to a JSON file and/or POST it to a URL.

They are safe to run again on the same machine, e.g. to rotate the password.

## Windows

From an elevated PowerShell prompt:

```powershell
powershell -ExecutionPolicy Bypass -File .\install-windows.ps1 `
  -Installer https://example.com/rustdesk-x86_64.exe `
  -Config "host=rd.example.com,key=YOUR_PUBLIC_KEY" `
  -ReportUrl https://example.com/rustdesk-checkin
```

`-Installer` takes the `.exe` or `.msi`, as a local path or URL. Leave it out if
RustDesk is already installed. Other options: `-Password`, `-OutFile`,
`-AppName` (for a renamed build; default `RustDesk`).

A Windows machine with no monitor attached still works: on Windows 10 2004 and
later RustDesk adds a virtual display when it finds none.

## Linux

```bash
sudo ./install-linux.sh \
  --package https://example.com/rustdesk-x86_64.deb \
  --config "host=rd.example.com,key=YOUR_PUBLIC_KEY" \
  --report-url https://example.com/rustdesk-checkin
```

`--package` takes a `.deb`, `.rpm` or `.pkg.tar.zst`. Leave it out if RustDesk
is already installed. Other options: `--password`, `--out-file`.

RustDesk shares an existing graphical session (or the login screen); it does
not start a desktop on a machine that has none. X11 sessions are the most
reliable for unattended use.

## macOS

macOS cannot be fully scripted: Apple requires someone at the Mac to grant
Screen Recording and Accessibility once. After that:

1. Install the `.dmg` into `/Applications`, open RustDesk once, grant the
   permissions it asks for, and install its service when prompted.
2. In Terminal:

   ```bash
   sudo /Applications/RustDesk.app/Contents/MacOS/RustDesk --password 'YOUR_PASSWORD'
   ```

   ```bash
   /Applications/RustDesk.app/Contents/MacOS/RustDesk --get-id
   ```

From then on it runs at login without the window open.

## Getting the connection info back

* **Run it yourself** (remote session, RMM tool, etc.): the info is printed at
  the end, so read it from the output.
* **Have the client run it**: pass `-ReportUrl` / `--report-url` and the
  script POSTs this JSON to you:

  ```json
  {
    "computer": "FRONT-DESK-PC",
    "id": "123456789",
    "password": "...",
    "server": "rd.example.com",
    "text": "FRONT-DESK-PC: RustDesk ID 123456789, password ..."
  }
  ```

  The `text` field means a Slack-style incoming webhook works as-is. Use
  `https://`, since the body contains the password.
* `-OutFile` / `--out-file` writes the same JSON locally. It holds the
  password in plain text, so put it somewhere private and delete it once
  you've recorded the info.

With RustDesk Server Pro you can also file each device in your address book
or a device group when it's deployed: run `rustdesk --assign --token <token>`
with `--address_book_name` or `--device_group_name`.

## What the client sees

Nothing changes in what RustDesk shows: its tray icon stays visible, and while
you're connected a window on their screen shows who is connected and lets them
disconnect you. Get your clients' agreement before you set up unattended
access.
