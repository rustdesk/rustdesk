# RustDesk msi project

Use Visual Studio 2022 to compile this project.

This project is mainly derived from <https://github.com/MediaPortal/MediaPortal-2.git> .

## Steps

1. `python preprocess.py`
2. Build the .sln solution.

Run `msiexec /i package.msi /l*v install.log` to record the log.

## TODOs

1. tray, uninstall shortcut
1. launch client after installation
1. github ci
1. options
1. Custom client.
    1. firewall and tcp allow. Outgoing
    1. Custom icon. Current `Resources/icon.ico`.
    1. Show license ?
    1. Do create service. Outgoing.
