# Build from the repository root:
#   docker build -f web-client/Dockerfile -t rustdesk-web-client .
FROM node:22-bookworm AS web
WORKDIR /src/web-client/web
COPY web-client/web/package.json web-client/web/package-lock.json ./
RUN npm ci
COPY web-client/web ./
RUN npm run build

FROM rust:1.83-bookworm AS gateway
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev libwayland-dev libxkbcommon-dev clang \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY Cargo.toml Cargo.lock build.rs ./
COPY libs ./libs
COPY src ./src
COPY web-client/gateway ./web-client/gateway
RUN cargo build --release -p rustdesk-web-gateway

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libssl3 libwayland-client0 \
    && rm -rf /var/lib/apt/lists/*
COPY --from=gateway /src/target/release/rustdesk-web-gateway /usr/local/bin/rustdesk-web-gateway
COPY --from=web /src/web-client/web/dist /app/web
ENV WEB_GATEWAY_LISTEN=0.0.0.0:21119
ENV WEB_GATEWAY_STATIC=/app/web
EXPOSE 21119
USER nobody
ENTRYPOINT ["rustdesk-web-gateway"]
