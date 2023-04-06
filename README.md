# RustDesk - iSEKS fork

This is a fork of https://github.com/rustdesk in order to hardcode iseks server (iseks.de) as primary relay.


## File Structure

- **[libs/hbb_common](https://github.com/iseks/rustdesk/tree/master/libs/hbb_common)**: video codec, config, tcp/udp wrapper, protobuf, fs functions for file transfer, and some other utility functions
- **[libs/scrap](https://github.com/iseks/rustdesk/tree/master/libs/scrap)**: screen capture
- **[libs/enigo](https://github.com/iseks/rustdesk/tree/master/libs/enigo)**: platform specific keyboard/mouse control
- **[src/ui](https://github.com/iseks/rustdesk/tree/master/src/ui)**: GUI
- **[src/server](https://github.com/iseks/rustdesk/tree/master/src/server)**: audio/clipboard/input/video services, and network connections
- **[src/client.rs](https://github.com/iseks/rustdesk/tree/master/src/client.rs)**: start a peer connection
- **[src/rendezvous_mediator.rs](https://github.com/iseks/rustdesk/tree/master/src/rendezvous_mediator.rs)**: Communicate with [rustdesk-server](https://github.com/rustdesk/rustdesk-server), wait for remote direct (TCP hole punching) or relayed connection
- **[src/platform](https://github.com/iseks/rustdesk/tree/master/src/platform)**: platform specific code
- **[flutter](https://github.com/iseks/rustdesk/tree/master/flutter)**: Flutter code for mobile
- **[flutter/web/js](https://github.com/iseks/rustdesk/tree/master/flutter/web/js)**: JavaScript for Flutter web client

