#!/usr/bin/env bash
# Installs RustDesk for unattended access and prints the connection info.
# Run as root. Safe to re-run. See README.md in this directory.
set -euo pipefail

usage() {
	cat <<'EOF'
Usage: sudo ./install-linux.sh [options]

  --package PATH|URL   RustDesk .deb, .rpm or .pkg.tar.zst (local path or
                       https URL) to install.
                       Omit if RustDesk is already installed.
  --password PASS      Permanent password to set (random if omitted).
  --config STRING      Server config: "host=...,key=..." or the encoded
                       config string from your server console.
  --report-url URL     POST the connection info to this URL as JSON.
  --out-file PATH      Write the connection info to this file as JSON
                       (contains the password; created root-only).
  -h, --help           Show this help.
EOF
}

die() {
	echo "Error: $*" >&2
	exit 1
}

PACKAGE=""
PASSWORD=""
CONFIG=""
REPORT_URL=""
OUT_FILE=""
while [ $# -gt 0 ]; do
	case "$1" in
	--package) PACKAGE="${2:?--package needs a value}"; shift 2 ;;
	--password) PASSWORD="${2:?--password needs a value}"; shift 2 ;;
	--config) CONFIG="${2:?--config needs a value}"; shift 2 ;;
	--report-url) REPORT_URL="${2:?--report-url needs a value}"; shift 2 ;;
	--out-file) OUT_FILE="${2:?--out-file needs a value}"; shift 2 ;;
	-h | --help) usage; exit 0 ;;
	*) usage >&2; die "unknown option: $1" ;;
	esac
done

case "$PASSWORD" in
*[[:cntrl:]]*) die "--password must not contain control characters." ;;
esac
[ "$(id -u)" -eq 0 ] || die "run this script as root (sudo)."
command -v systemctl >/dev/null || die "systemd is required."

rustdesk_cli() {
	{ rustdesk "$@" 2>/dev/null || true; } | tail -n 1 | tr -d '\r'
}

wait_until() {
	local timeout="$1" what="$2"
	shift 2
	local deadline=$((SECONDS + timeout))
	until "$@"; do
		[ "$SECONDS" -lt "$deadline" ] || die "timed out after ${timeout}s waiting for $what."
		sleep 2
	done
}

install_package() {
	local pkg="$1"
	if [[ "$pkg" =~ ^https?:// ]]; then
		local dir name
		dir="$(mktemp -d)"
		name="$(basename "${pkg%%\?*}")"
		[ -n "$name" ] || name="rustdesk-package"
		[[ "$pkg" =~ ^https:// ]] || die "refusing to download the package over http. Use an https URL or a local file."
		command -v curl >/dev/null || die "curl is required to download the package."
		echo "Downloading $pkg"
		curl -fL --proto '=https' --proto-redir '=https' --retry 3 -o "$dir/$name" "$pkg"
		pkg="$dir/$name"
	fi
	[ -f "$pkg" ] || die "package not found: $pkg"
	pkg="$(cd "$(dirname "$pkg")" && pwd)/$(basename "$pkg")"

	echo "Installing $pkg"
	case "$pkg" in
	*.deb)
		DEBIAN_FRONTEND=noninteractive apt-get install -y "$pkg"
		;;
	*.rpm)
		if command -v dnf >/dev/null; then
			dnf install -y "$pkg"
		elif command -v zypper >/dev/null; then
			zypper --non-interactive install --allow-unsigned-rpm "$pkg"
		elif command -v yum >/dev/null; then
			yum install -y "$pkg"
		else
			die "no rpm package manager (dnf, zypper, yum) found."
		fi
		;;
	*.pkg.tar.zst)
		pacman -U --noconfirm "$pkg"
		;;
	*)
		die "unsupported package type: $pkg"
		;;
	esac
}

server_running() {
	pgrep -f "rustdesk --server" >/dev/null
}

# The CLI talks to the running --server over IPC. Until that answers,
# --get-id falls back to a freshly generated local ID, so the password
# being accepted is what tells us the service is reachable.
set_password() {
	[ "$(rustdesk_cli --password "$PASSWORD")" = "Done!" ]
}

json_escape() {
	printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g'
}

if [ -n "$PACKAGE" ]; then
	install_package "$PACKAGE"
fi
command -v rustdesk >/dev/null || die "rustdesk is not installed. Pass --package to install it."

systemctl enable rustdesk >/dev/null 2>&1 || die "could not enable rustdesk.service."
systemctl start rustdesk
wait_until 120 "rustdesk --server to start (it needs a graphical session or login screen)" server_running

if [ -z "$PASSWORD" ]; then
	PASSWORD="$(set +o pipefail; LC_ALL=C tr -dc 'ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnpqrstuvwxyz23456789' </dev/urandom | head -c 16)"
fi
wait_until 120 "RustDesk to accept the password" set_password

if [ -n "$CONFIG" ]; then
	echo "Applying server config"
	rustdesk_cli --config "$CONFIG" >/dev/null
fi

if [ "$(rustdesk_cli --option approve-mode)" = "click" ]; then
	echo "Switching approve mode from click-only to password"
	rustdesk_cli --option approve-mode password >/dev/null
fi
if [ "$(rustdesk_cli --option verification-method)" = "use-temporary-password" ]; then
	echo "Enabling the permanent password alongside the one-time password"
	rustdesk_cli --option verification-method use-both-passwords >/dev/null
fi

ID="$(rustdesk_cli --get-id | tr -d '[:space:]')"
[ -n "$ID" ] || die "RustDesk did not report its ID."
SERVER="$(rustdesk_cli --option custom-rendezvous-server)"
[ -n "$SERVER" ] || SERVER="public RustDesk server"
COMPUTER="$(hostname)"
TEXT="$COMPUTER: RustDesk ID $ID, password $PASSWORD, server $SERVER"

JSON="$(printf '{\n  "computer": "%s",\n  "id": "%s",\n  "password": "%s",\n  "server": "%s",\n  "text": "%s"\n}' \
	"$(json_escape "$COMPUTER")" "$(json_escape "$ID")" "$(json_escape "$PASSWORD")" \
	"$(json_escape "$SERVER")" "$(json_escape "$TEXT")")"

cat <<EOF

RustDesk unattended access is ready.
  Computer : $COMPUTER
  ID       : $ID
  Password : $PASSWORD
  Server   : $SERVER

EOF

if [ -n "$OUT_FILE" ]; then
	(umask 077 && printf '%s\n' "$JSON" >"$OUT_FILE")
	echo "Saved to $OUT_FILE"
fi

if [ -n "$REPORT_URL" ]; then
	[[ "$REPORT_URL" =~ ^https:// ]] || echo "Warning: report URL is not https; the password will be sent unencrypted." >&2
	command -v curl >/dev/null || die "curl is required for --report-url."
	if printf '%s' "$JSON" | curl -fsS -X POST -H 'Content-Type: application/json' --data-binary @- "$REPORT_URL" >/dev/null; then
		echo "Reported to $REPORT_URL"
	else
		die "reporting to $REPORT_URL failed."
	fi
fi
