# Network Interface Binding

RustDesk can be configured to bind to a specific network interface by IP address.

## Configuration

To bind RustDesk to a specific network interface, set the `bind-interface` option in your configuration.

### Option Name
`bind-interface`

### Supported Values
- Empty string (default): Bind to all available interfaces (0.0.0.0 for IPv4, :: for IPv6)
- IPv4 address: e.g., `192.168.1.100`, `10.0.0.1`
- IPv6 address: e.g., `::1`, `fe80::1`, `2001:db8::1`

## Usage Examples

### Bind to a specific IPv4 address
To bind RustDesk to only listen on interface with IP address `192.168.1.100`:

```json
{
  "options": {
    "bind-interface": "192.168.1.100"
  }
}
```

### Bind to IPv6 localhost
```json
{
  "options": {
    "bind-interface": "::1"
  }
}
```

### Bind to all interfaces (default)
```json
{
  "options": {
    "bind-interface": ""
  }
}
```

Or simply omit the `bind-interface` option entirely.

## How It Works

When the `bind-interface` option is set:

1. RustDesk reads the configuration when starting the direct server
2. If `bind-interface` is empty or not set, it binds to all available network interfaces
3. If `bind-interface` contains a valid IP address:
   - RustDesk validates the IP address format (supports both IPv4 and IPv6)
   - Creates a TCP socket and binds it to the specified IP address
   - Starts listening for connections on that interface only

## Use Cases

### Multiple Network Interfaces
If your machine has multiple network interfaces (e.g., Ethernet, Wi-Fi, VPN), you can force RustDesk to use a specific one:

- **Example**: Force RustDesk to use the Ethernet interface at `192.168.1.100` instead of the Wi-Fi interface at `192.168.2.50`

### Security
Restrict RustDesk to listen only on internal network interfaces:

- **Example**: Bind to `10.0.0.5` (internal network) instead of listening on all interfaces including public-facing ones

### VPN/Tunneling
Force RustDesk to use a VPN or tunnel interface:

- **Example**: Bind to the VPN interface IP address to ensure all traffic goes through the VPN

## Troubleshooting

### Invalid bind address error
If you see an error like "Invalid bind interface address", check that:
- The IP address format is correct (no typos)
- The IP address is valid (e.g., not `999.999.999.999`)
- The IP address exists on one of your machine's network interfaces

### Failed to start direct server
If RustDesk fails to start with a bind error, it could be because:
- The specified IP address doesn't exist on your machine
- Another application is already using the port on that interface
- You don't have permission to bind to that address

### Finding your network interface IP addresses

**Windows:**
```cmd
ipconfig
```

**Linux/macOS:**
```bash
ip addr show    # Linux
ifconfig        # macOS/Linux
```

Look for the `inet` (IPv4) or `inet6` (IPv6) addresses associated with your network interfaces.

## Related Discussion

This feature was implemented to address: https://github.com/rustdesk/rustdesk/discussions/2286
