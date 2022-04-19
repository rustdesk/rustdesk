## How to build and run with Snap

Begin by cloning the repository and make sure snapcraft is installed in your Linux.

```sh
git clone https://github.com/rustdesk/rustdesk
# if snapcraft is installed, please skip this
sudo snap install snapcraft --classic
# build rustdesk snap package
snapcraft --use-lxd
# install rustdesk snap package, `--dangerous` flag must exists if u manually build and install rustdesk
sudo snap install rustdesk_xxx.snap --dangerous
```

Note: Some of interfaces needed by RustDesk cannot automatically connected by Snap. Please **manually** connect them by executing:
```sh
# record system audio
snap connect rustdesk:audio-record
snap connect rustdesk:pulseaudio
# observe loginctl session
snap connect rustdesk:login-session-observe
```

After steps above, RustDesk can be found in System App Menu.

