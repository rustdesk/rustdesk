
MacOS cannot use `fuse` because of https://github.com/macfuse/macfuse/wiki/Getting-Started#enabling-support-for-third-party-kernel-extensions-apple-silicon-macs 

1. Use a temporary file `/tmp/rustdesk_<uuid>` as a placeholder in the pasteboard.
2. Uses `fsevent` to observe files paste operation. Then perform pasting files.
