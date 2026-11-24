#!/usr/bin/env bash
set -euo pipefail

# RustDesk Web Client — Docker One-Click Deployment Script
# Usage:
#   ./deploy.sh
#   ./deploy.sh --port 21120 --token "your-secret-token"
#   HOST_PORT=28080 ./deploy.sh

DEFAULT_PORT="${HOST_PORT:-21120}"
HOST_BIND="${HOST_BIND:-0.0.0.0}"
GATEWAY_TOKEN="${RUSTDESK_GATEWAY_TOKEN:-}"
CONTAINER_NAME="${CONTAINER_NAME:-rustdesk-web-client}"
USER_PORT_EXPLICIT=0

print_usage() {
  cat <<'EOF'
Usage: ./deploy.sh [options]

Options:
  -p, --port <PORT>      Host port to bind (default: 21120; auto-increments if occupied)
  -b, --bind <IP>        Host interface IP to bind (default: 0.0.0.0)
  -t, --token <TOKEN>    Optional gateway access token (RUSTDESK_GATEWAY_TOKEN)
  -n, --name <NAME>      Docker container name (default: rustdesk-web-client)
  -h, --help             Show this help message
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    -p|--port)
      DEFAULT_PORT="$2"
      USER_PORT_EXPLICIT=1
      shift 2
      ;;
    -b|--bind)
      HOST_BIND="$2"
      shift 2
      ;;
    -t|--token)
      GATEWAY_TOKEN="$2"
      shift 2
      ;;
    -n|--name)
      CONTAINER_NAME="$2"
      shift 2
      ;;
    -h|--help)
      print_usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      print_usage
      exit 1
      ;;
  esac
done

if ! [[ "$DEFAULT_PORT" =~ ^[0-9]+$ ]] || (( DEFAULT_PORT < 1 || DEFAULT_PORT > 65535 )); then
  echo "[ERROR] Invalid port number: $DEFAULT_PORT" >&2
  exit 1
fi

# Check Docker & Compose availability
if ! command -v docker >/dev/null 2>&1; then
  echo "[ERROR] Docker is not installed. Please install Docker first: https://docs.docker.com/get-docker/" >&2
  exit 1
fi

if docker compose version >/dev/null 2>&1; then
  COMPOSE_CMD=(docker compose)
elif command -v docker-compose >/dev/null 2>&1; then
  COMPOSE_CMD=(docker-compose)
else
  echo "[ERROR] Docker Compose plugin is not available." >&2
  exit 1
fi

# Check if a port is currently used on the host (excluding our own existing container if restarting)
is_port_in_use() {
  local port="$1"

  # Skip RustDesk hbbs/hbbr reserved range (21114-21119) to avoid colliding with a co-located RustDesk server
  if (( port >= 21114 && port <= 21119 )); then
    return 0
  fi

  # If our own container already binds this exact host port, allow seamless redeploy on the same port
  if docker ps --filter "name=^/${CONTAINER_NAME}$" --format '{{.Ports}}' 2>/dev/null | grep -q ":${port}->3000/"; then
    return 1
  fi

  if command -v ss >/dev/null 2>&1; then
    if ss -H -ltn 2>/dev/null | awk '{print $4}' | grep -Eq "(:|\\])${port}$"; then
      return 0
    fi
  elif command -v lsof >/dev/null 2>&1; then
    if lsof -iTCP:"${port}" -sTCP:LISTEN -Pn >/dev/null 2>&1; then
      return 0
    fi
  elif command -v netstat >/dev/null 2>&1; then
    if netstat -ltn 2>/dev/null | awk '{print $4}' | grep -Eq "(:|\\])${port}$"; then
      return 0
    fi
  fi

  return 1
}

SELECTED_PORT="$DEFAULT_PORT"
if is_port_in_use "$SELECTED_PORT"; then
  if (( SELECTED_PORT >= 21114 && SELECTED_PORT <= 21119 )); then
    echo "[WARN] Port ${SELECTED_PORT} belongs to RustDesk hbbs/hbbr (21114-21119)."
  else
    echo "[WARN] Port ${SELECTED_PORT} is already in use on this host."
  fi

  if (( USER_PORT_EXPLICIT == 1 )); then
    echo "[INFO] Searching for the next available port automatically..."
  fi

  candidate=$(( SELECTED_PORT < 21120 ? 21120 : SELECTED_PORT + 1 ))
  found=0
  for (( i = 0; i < 100; i++ )); do
    if (( candidate > 65535 )); then
      break
    fi
    if ! is_port_in_use "$candidate"; then
      SELECTED_PORT="$candidate"
      found=1
      break
    fi
    candidate=$(( candidate + 1 ))
  done

  if (( found == 0 )); then
    echo "[ERROR] Could not find a free port automatically. Please specify one with --port <PORT>." >&2
    exit 1
  fi
  echo "[INFO] Automatically switched host port to ${SELECTED_PORT} to avoid conflict."
fi

# Persist chosen deployment settings cleanly in .env.docker (read by compose via --env-file)
ENV_FILE=".env.docker"
cat > "$ENV_FILE" <<EOF
HOST_BIND=${HOST_BIND}
HOST_PORT=${SELECTED_PORT}
CONTAINER_NAME=${CONTAINER_NAME}
RUSTDESK_GATEWAY_TOKEN=${GATEWAY_TOKEN}
EOF
chmod 600 "$ENV_FILE"

echo "[INFO] Building and starting ${CONTAINER_NAME} on ${HOST_BIND}:${SELECTED_PORT} -> container:3000 ..."
"${COMPOSE_CMD[@]}" --env-file "$ENV_FILE" up -d --build --remove-orphans

echo "[INFO] Waiting for gateway health check..."
HEALTH_URL="http://127.0.0.1:${SELECTED_PORT}/api/health"
for (( i = 1; i <= 30; i++ )); do
  if command -v curl >/dev/null 2>&1 && curl -fsS "$HEALTH_URL" >/dev/null 2>&1; then
    break
  fi
  sleep 1
done

echo ""
echo "============================================================"
echo " RustDesk Web Client deployed successfully!"
echo " Local URL : http://127.0.0.1:${SELECTED_PORT}"
echo " Container : ${CONTAINER_NAME} (internal port 3000)"
echo " Config    : ${ENV_FILE}"
echo " Note      : WebCodecs requires HTTPS when accessed via a"
echo "             remote IP/domain (put Nginx/Caddy TLS in front"
echo "             and forward WebSocket /ws upgrades)."
echo "============================================================"
