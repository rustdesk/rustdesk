# RustDesk Web Client

A browser remote-desktop client for an **unmodified** RustDesk host. The host
keeps doing its own capture and encoding. A small Rust gateway forwards those
frames and turns browser input into RustDesk protobuf messages.

```text
browser (React + WebCodecs)
        |  JSON control + binary video  (WebSocket)
        v
web-client/gateway          Rust, axum
        |  framed protobuf, NaCl when the host asks for it
        v
RustDesk host               unchanged  (direct IP :21118, or ID/relay)
```

There is no server-side video transcode. `VideoFrame` payloads (`vp8s`,
`vp9s`, `h264s`, `av1s`, `h265s`) are copied into a 32-byte header and sent
as a binary WebSocket frame. The page decodes them with `VideoDecoder`.

## Layout

| Path | Role |
| --- | --- |
| `gateway/` | Headless WebSocket gateway. Depends on `libs/hbb_common` and `libs/base` for the wire protocol, protobuf, and NaCl primitives. It does not link `scrap`, ffmpeg, or a GUI. |
| `web/` | React + TypeScript + Vite UI. |
| `scripts/e2e_desktop.py` | Full-screen Tk target used for the loopback check. |
| `docs/screenshots/` | Captured desktop and mobile sessions. |

## Gateway

Direct IP is the primary path. The host's direct server (`direct-server = 'Y'`,
port `21118`) accepts the TCP connection in the clear, sends a password
`Hash`, and then the usual `LoginRequest` / `LoginResponse` / `PeerInfo` /
`SwitchDisplay` exchange. The password is the normal double SHA-256:

```text
h1       = SHA256(password || salt)
response = SHA256(h1 || challenge)
```

Punch and relay sessions speak first with `SignedId`. The gateway answers
with `PublicKey`, installs the negotiated secretbox key (including key
exchange version 1), and only then reads the password challenge. ID mode
uses the same rendezvous messages as the desktop client: `PunchHoleRequest`,
then either a direct TCP punch or `RequestRelay` on port `21117`.

```bash
# from the repository root, after `git submodule update --init libs/hbb_common`
cargo test --manifest-path web-client/gateway/Cargo.toml
cargo run --manifest-path web-client/gateway/Cargo.toml -- \
  --listen 127.0.0.1:21119 \
  --static web-client/web/dist
```

| Variable | Flag | Default |
| --- | --- | --- |
| `WEB_GATEWAY_LISTEN` | `--listen` | `127.0.0.1:21119` |
| `WEB_GATEWAY_STATIC` | `--static` | `web-client/web/dist` or `/app/web` when that directory exists |
| `GATEWAY_TOKEN` | `--token` | empty. When set, the page asks for this token before dialing a host |

`GET /api/health` returns `{"ok":true,"service":"rustdesk-web-gateway","authRequired":false}`.
The same process serves the built page and `/ws`, so a browser on that origin does not need a second server or CORS setup. `npm run dev` still proxies `/api` and `/ws` to `127.0.0.1:21119` for frontend work.

After a video message the gateway echoes `TestDelay` unchanged and sends `Misc.video_received`, which is what keeps the host's 1.5 QoS from treating the link as stalled. The page probes `VideoDecoder.isConfigSupported` and only advertises those codecs, with `prefer_chroma = I420`.

### Binary video frame

```text
 0  b"RDV1"
 4  codec u8     1=vp8 2=vp9 3=h264 4=h265 5=av1
 5  flags u8     bit 0 = keyframe
 6  display u16  little-endian
 8  width u32
12  height u32
16  pts i64
24  gateway unix ms u64
32  encoded payload
```

Browser control messages are JSON (`connect`, `mouse`, `key`, `text`,
`clipboard`, `switchDisplay`, `refresh`, `quality`, `disconnect`).

## Web UI

```bash
cd web-client/web
npm install
npm test
npm run lint
npm run build
npm run dev    # proxies /api and /ws to 127.0.0.1:21119
```

The canvas keeps the remote aspect ratio. Pointer and touch events are
mapped through that letterbox into remote pixels. Keyboard capture includes
modifiers, the toolbar chords (`Esc`, `Tab`, `Ctrl+C`, `Ctrl+V`,
`Ctrl+Alt+Del`), and an IME / soft-keyboard field for phones.

## LAN self-host

1. On the computer you want to control, enable direct IP access and set a
   permanent password. In RustDesk: Settings → Security → password, and
   Settings → Network → enable direct IP access (port `21118`). Or write
   `RustDesk2.toml`:

   ```toml
   [options]
   direct-server = "Y"
   direct-access-port = "21118"
   approve-mode = "password"
   verification-method = "use-permanent-password"
   ```

2. Build the web app (`npm run build`) and run the gateway. If `web/dist`
   is present it is served on the same port as `/ws`.
3. Open `http://127.0.0.1:21119` (or `npm run dev` during UI work). Connect
   to the host address and port `21118`.

One container does the same thing:

```bash
docker compose -f web-client/docker-compose.yml up --build
```

Set `GATEWAY_TOKEN` in that compose file before publishing the port. Leave it
empty on a trusted loopback. A wrong host password, an unreachable address,
or a rejected token is returned as a sentence in the connection panel, and
the button becomes Reconnect.

### HTTPS in front of the gateway

`deploy/Caddyfile` and `deploy/nginx.conf` terminate TLS and forward the
page and the WebSocket on one port. Caddy's public config asks for a
certificate automatically. The LAN config uses Caddy's internal CA; import
that CA, or accept the browser warning, so the origin stays a secure context.

### HTTPS and WebCodecs

`VideoDecoder` only exists in a secure context. `http://localhost` and
`http://127.0.0.1` count. A page served from another host name, or from a
bare LAN IP, does not: the browser will show the decoder banner and refuse
to paint. Put TLS in front of the gateway (Caddy, nginx, or a tunnel) and
open the page as `https://...`. The page then uses `wss://` for `/ws`.

The gateway itself speaks plain WebSocket. Terminate TLS at the reverse
proxy and proxy `/ws` with upgrade headers.

## What this build does not do

- It does not transcode, record, or touch the host's screen capturer.
- ID mode needs a rendezvous server you already run. The gateway does not
  ship a public-server key or any account credential.
- Audio, file transfer, and terminal sessions are out of scope.
