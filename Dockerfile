FROM rust:1.77 as builder

RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    build-essential \
    cmake \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY . .

RUN cargo build --release --bin hbbs
RUN cargo build --release --bin hbbr

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    libssl3 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /root

COPY --from=builder /app/target/release/hbbs /usr/local/bin/hbbs
COPY --from=builder /app/target/release/hbbr /usr/local/bin/hbbr

CMD ["hbbs"]

