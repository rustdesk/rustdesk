# RustDesk Web Client

The web client uses WebCodecs to display RustDesk sessions. The Node.js gateway serves the frontend and handles the WebSocket connection at `/ws`.

## Local development

```sh
yarn install --frozen-lockfile
yarn dev
```

Open `http://localhost:3000`.

## Production

```sh
yarn build
NODE_ENV=production npm start
```

Use Node.js 24 for native cursor decoding. The gateway reads `PORT` and supports `RUSTDESK_GATEWAY_TOKEN` for gateway authentication. Use HTTPS for remote browser access.
