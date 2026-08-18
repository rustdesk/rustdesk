# Cyberdriver

Cyberdriver is the Cyberdesk desktop agent. Install it on a Windows machine and that machine shows up in your Cyberdesk dashboard, ready to be screenshotted, clicked, typed into, and automated by Cyberdesk workflows and agents.

[Documentation](https://docs.cyberdesk.io) · [Quickstart](https://docs.cyberdesk.io/cyberdriver/quickstart) · [Dashboard](https://cyberdesk.io/dashboard)

## How it works

Cyberdriver runs as a Windows service with a desktop app for configuration. On startup it opens an outbound WebSocket tunnel to Cyberdesk and waits for work. Everything travels over that one connection:

- **No inbound ports.** Cyberdriver dials out to Cyberdesk, so it works behind NAT and corporate firewalls without port forwarding.
- **No local HTTP server.** The control routes are only reachable through the authenticated tunnel, never from localhost.
- **Service-first.** Because it runs as a service, Cyberdriver starts at boot and can control the machine before anyone logs in, including the Windows login screen.

Interactive remote desktop sessions (**Desktop Tools** in the dashboard) run over Cyberdesk-hosted rendezvous and relay servers, so you get a live screen alongside the automation API.

## Install

Windows x64 is the supported platform. Run the PowerShell installer from the [Quickstart](https://docs.cyberdesk.io/cyberdriver/quickstart), or grab the MSI directly from [Releases](https://github.com/cyberdesk-hq/cyberdriver-new/releases/latest) and install it as Administrator.

Then connect the machine with an organization API key from the dashboard, either by pasting it into the **Cyberdesk tunnel** card in the Cyberdriver app, or from a terminal:

```powershell
cyberdriver join --secret ak_your_api_key
```

The machine appears under **Desktops** in the dashboard once the tunnel is up.

Administrator is required at install time because the MSI registers a system service and writes to `Program Files`. Day-to-day use of the app does not need elevation. See [why Administrator is required](https://docs.cyberdesk.io/cyberdriver/quickstart#why-administrator-is-required-for-the-beta-msi).

## CLI

```
cyberdriver join --secret <ak_*> [options]
cyberdriver status
cyberdriver config-print
cyberdriver logs [--tail <bytes>] [--follow] [--path <file>]
cyberdriver new-identity
cyberdriver stop
cyberdriver --version
```

Useful `join` options:

| Flag | Purpose |
| --- | --- |
| `--secret <ak_*>` | Cyberdesk organization API key. Stored encrypted, then dropped from argv. |
| `--name <name>` | Display name in the dashboard. Printable ASCII, max 128 chars. |
| `--new-identity` | Reset the machine fingerprint and peer identity. Use this on every clone booted from a golden image or AMI. |
| `--no-keepalive` | Disable keepalive. It is on by default. |
| `--register-as-keepalive-for <machine-id>` | Run this host as the remote keepalive machine for another machine. |
| `--api-base <base>` | Point the tunnel at a custom Cyberdesk host. |
| `--env dev` | Use the Cyberdesk development environment. |

`cyberdriver status` and `config-print` are the fastest way to see whether the API key is configured, which environment you are on, and what the machine fingerprint is. Full reference: [CLI and Machine Images](https://docs.cyberdesk.io/cyberdriver/cli).

## What Cyberdesk can do over the tunnel

| Area | Capabilities |
| --- | --- |
| Display | Screen dimensions, screenshots |
| Mouse | Position, move, click, drag, scroll |
| Keyboard | Type text, key sequences and chords |
| Clipboard | Copy text to the clipboard |
| Files | List, read, write |
| Shell | PowerShell commands and persistent sessions |
| Management | Diagnostics, remote update, remote shutdown, keepalive control |

These are consumed through the Cyberdesk API and workflows rather than called directly. See [Desktop Tools](https://docs.cyberdesk.io/cyberdriver/desktop-tools).

## Configuration

Settings live in the Cyberdriver config directory and are managed through the app, the CLI, or MSI install properties. Run `cyberdriver config-print` to see the resolved state.

Environment variables, useful for scripted and image-based deployments:

| Variable | Purpose |
| --- | --- |
| `CYBERDESK_AGENT_KEY` | API key, instead of `--secret`. |
| `CYBERDRIVER_MACHINE_NAME` | Machine display name, instead of `--name`. |
| `CYBERDESK_API_BASE` | Tunnel base URL, instead of `--api-base`. |

For unattended and fleet installs, the MSI accepts `APIKEY`, `INSTALL_AS_SERVICE`, `CYBERDESK_API_BASE`, and `REGISTER_NOW`. See [`docs/headless-install.md`](docs/headless-install.md) for the golden image workflow, including how to avoid every clone sharing one identity.

## Troubleshooting

Start with `cyberdriver logs --tail 65536` and `cyberdriver status`. The docs cover the common cases:

- [Diagnostics and logs](https://docs.cyberdesk.io/cyberdriver/diagnostics)
- [Corporate TLS inspection and firewalls](https://docs.cyberdesk.io/cyberdriver/quickstart#corporate-tls-inspection-and-firewalls)
- [Clear all Cyberdriver traces and reinstall](https://docs.cyberdesk.io/cyberdriver/clear-cyberdriver-traces)
- [Display reliability](https://docs.cyberdesk.io/cyberdriver/display-reliability)

## Legacy Cyberdriver

The previous Python-based agent lives in [cyberdesk-hq/cyberdriver](https://github.com/cyberdesk-hq/cyberdriver) and is still supported. It does not require Administrator or a service install, but it cannot start at boot or reach the Windows login screen. See [Legacy Cyberdriver](https://docs.cyberdesk.io/cyberdriver/legacy-cyberdriver) to decide which one you want.

## Development

See [AGENTS.md](AGENTS.md) for the repository layout and build setup, and [`branding/`](branding/) for how Cyberdesk branding and release packaging are applied.

## License

Cyberdriver is a fork of [RustDesk](https://github.com/rustdesk/rustdesk) and is licensed under the GNU AGPL v3. See [LICENCE](LICENCE).

**Misuse disclaimer:** Cyberdriver is built for automating machines you own or are authorized to control. Unauthorized access, control, or invasion of privacy is against our guidelines, and the authors are not responsible for misuse.
