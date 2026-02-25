#!/bin/bash
# ============================================================
#  Déploiement massif RustDesk — macOS
#  Usage : sudo bash deploy_rustdesk_macos.sh [OPTIONS]
#
#  Options :
#    --force      Re-applique la config même si déjà faite
#    --reinstall  Réinstalle RustDesk même s'il est déjà présent
#
#  Ce script :
#   1. Installe RustDesk si absent (téléchargement depuis GitHub)
#   2. Installe les fichiers helpers dans /Library/Application Support/
#   3. Installe un LaunchAgent qui configure chaque utilisateur à son login
#   4. Applique immédiatement la config aux utilisateurs déjà existants
# ============================================================
set -euo pipefail

FORCE=false
REINSTALL=false
for arg in "$@"; do
    [[ "$arg" == "--force"     ]] && FORCE=true
    [[ "$arg" == "--reinstall" ]] && REINSTALL=true
done

# ────────────────────────────────────────────────────────────
# Config serveur
# ────────────────────────────────────────────────────────────
RDSERVER="antoineca.synology.me"
RDRELAY="antoineca.synology.me"
RDKEY="AT0toZ0Xss9i3jny3GaEe54nfy4yDBTqMGQYl9d2PS8="
RDAPI=""   # vide = pas de serveur API Pro

echo "╔══════════════════════════════════════════╗"
echo "║  Déploiement RustDesk — macOS            ║"
echo "╚══════════════════════════════════════════╝"
echo "  Serveur : $RDSERVER"
echo "  Relay   : $RDRELAY"
echo "  Clef    : $RDKEY"
echo "  API     : ${RDAPI:-<vide>}"
echo ""

# ────────────────────────────────────────────────────────────
# 1. Installation de RustDesk
# ────────────────────────────────────────────────────────────
APP="/Applications/RustDesk.app"

if [[ -d "$APP" && "$REINSTALL" == "false" ]]; then
    echo "RustDesk déjà installé — passe à la configuration."
    echo "  (utiliser --reinstall pour forcer une réinstallation)"
else
    echo "Installation de RustDesk..."

    # Détection architecture
    if [[ "$(arch)" == "arm64" ]]; then
        ARCH_SUFFIX="aarch64"
    else
        ARCH_SUFFIX="x86_64"
    fi
    echo "  Architecture : $ARCH_SUFFIX"

    # Récupérer l'URL du DMG via l'API GitHub (plus fiable que le scraping HTML)
    echo "  Récupération de la dernière version..."
    DMG_URL=$(curl -sf https://api.github.com/repos/rustdesk/rustdesk/releases/latest \
        | python3 -c "
import sys, json
data = json.load(sys.stdin)
arch = sys.argv[1]
for asset in data.get('assets', []):
    url = asset.get('browser_download_url', '')
    if arch in url and url.endswith('.dmg'):
        print(url)
        break
" "$ARCH_SUFFIX")

    if [[ -z "$DMG_URL" ]]; then
        echo "ERREUR : impossible de trouver le DMG pour $ARCH_SUFFIX." >&2
        exit 1
    fi
    echo "  URL : $DMG_URL"

    # Téléchargement dans un dossier temporaire
    WORKDIR=$(mktemp -d)
    DMG_FILE="$WORKDIR/rustdesk.dmg"
    MOUNT_POINT="/Volumes/RustDesk"

    echo "  Téléchargement..."
    curl -L --progress-bar "$DMG_URL" -o "$DMG_FILE"

    # Démontage préventif si le point de montage est déjà utilisé
    if [[ -d "$MOUNT_POINT" ]]; then
        hdiutil detach "$MOUNT_POINT" -quiet 2>/dev/null || true
    fi

    # Montage
    echo "  Montage du DMG..."
    if ! hdiutil attach "$DMG_FILE" -mountpoint "$MOUNT_POINT" -quiet; then
        echo "ERREUR : impossible de monter le DMG." >&2
        rm -rf "$WORKDIR"
        exit 1
    fi

    # Copie vers /Applications
    echo "  Copie vers /Applications..."
    rm -rf "$APP"
    cp -R "$MOUNT_POINT/RustDesk.app" /Applications/

    # Supprimer le flag quarantine (évite le blocage Gatekeeper au 1er lancement)
    xattr -rd com.apple.quarantine "$APP" 2>/dev/null || true

    # Démontage et nettoyage
    hdiutil detach "$MOUNT_POINT" -quiet
    rm -rf "$WORKDIR"

    echo "  RustDesk installé dans /Applications."
fi
echo ""

# ────────────────────────────────────────────────────────────
# Chemins helpers
# ────────────────────────────────────────────────────────────
SYSDIR="/Library/Application Support/com.carriez.RustDesk"
AGENT_PLIST="/Library/LaunchAgents/com.carriez.rustdesk-config.plist"
SENTINEL_NAME=".rustdesk_config_applied_v1"

mkdir -p "$SYSDIR"

# ────────────────────────────────────────────────────────────
# 2. server.env  (lu par le script per-user)
# ────────────────────────────────────────────────────────────
cat > "$SYSDIR/server.env" << ENV_EOF
RDSERVER="$RDSERVER"
RDRELAY="$RDRELAY"
RDKEY="$RDKEY"
RDAPI="$RDAPI"
ENV_EOF
chmod 644 "$SYSDIR/server.env"

# ────────────────────────────────────────────────────────────
# 3. update_config.py  (met à jour le TOML sans l'écraser)
# ────────────────────────────────────────────────────────────
cat > "$SYSDIR/update_config.py" << 'PY_EOF'
#!/usr/bin/env python3
"""
Met à jour (ou crée) RustDesk.toml avec les options serveur.
Fusionne avec la config existante : seules les clefs serveur sont modifiées.
"""
import re, os, sys

config_file = sys.argv[1]
server      = sys.argv[2]
relay       = sys.argv[3]
key         = sys.argv[4]
api         = sys.argv[5] if len(sys.argv) > 5 else ""


def set_option(text: str, key: str, value: str) -> str:
    """Insère ou remplace une clef dans la section [options]."""
    pattern = rf'^{re.escape(key)}\s*=.*$'
    line    = f'{key} = "{value}"'
    if re.search(pattern, text, re.MULTILINE):
        return re.sub(pattern, line, text, flags=re.MULTILINE)
    if '[options]' in text:
        return text.replace('[options]', '[options]\n' + line, 1)
    return text + f'\n[options]\n{line}\n'


os.makedirs(os.path.dirname(config_file), exist_ok=True)
try:
    content = open(config_file).read()
except FileNotFoundError:
    content = ''

content = set_option(content, 'custom-rendezvous-server', server)
content = set_option(content, 'relay-server', relay)
content = set_option(content, 'key', key)
if api:
    content = set_option(content, 'api-server', api)

with open(config_file, 'w') as f:
    f.write(content)

print(f'[RustDesk] Config écrite : {config_file}')
PY_EOF
chmod 755 "$SYSDIR/update_config.py"

# ────────────────────────────────────────────────────────────
# 4. apply_config.sh  (exécuté en contexte utilisateur)
# ────────────────────────────────────────────────────────────
cat > "$SYSDIR/apply_config.sh" << SH_EOF
#!/bin/bash
SENTINEL="\$HOME/Library/Application Support/com.carriez.RustDesk/$SENTINEL_NAME"
[ -f "\$SENTINEL" ] && exit 0

# Charger les valeurs serveur
source "/Library/Application Support/com.carriez.RustDesk/server.env"

CONFIG_FILE="\$HOME/Library/Application Support/com.carriez.RustDesk/RustDesk.toml"

python3 "/Library/Application Support/com.carriez.RustDesk/update_config.py" \\
    "\$CONFIG_FILE" "\$RDSERVER" "\$RDRELAY" "\$RDKEY" "\$RDAPI"

touch "\$SENTINEL"
SH_EOF
chmod 755 "$SYSDIR/apply_config.sh"

# ────────────────────────────────────────────────────────────
# 5. LaunchAgent (s'exécute au login de chaque utilisateur)
# ────────────────────────────────────────────────────────────
cat > "$AGENT_PLIST" << 'PLIST_EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
    "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.carriez.rustdesk-config</string>
    <key>ProgramArguments</key>
    <array>
        <string>/bin/bash</string>
        <string>/Library/Application Support/com.carriez.RustDesk/apply_config.sh</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
</dict>
</plist>
PLIST_EOF
chmod 644 "$AGENT_PLIST"

# ────────────────────────────────────────────────────────────
# 6. Appliquer immédiatement aux utilisateurs existants
# ────────────────────────────────────────────────────────────
echo "Application de la config aux comptes existants..."
APPLIED=0
SKIPPED=0

while IFS=: read -r uname _ uid _ _ uhome _; do
    # Ignorer les comptes système (uid < 500) et les home inexistants
    [[ "$uid" -lt 500 ]] && continue
    [[ ! -d "/Users/$uname" ]] && continue

    SENTINEL_PATH="/Users/$uname/Library/Application Support/com.carriez.RustDesk/$SENTINEL_NAME"

    if [[ "$FORCE" == "true" && -f "$SENTINEL_PATH" ]]; then
        rm -f "$SENTINEL_PATH"
    fi

    if [[ -f "$SENTINEL_PATH" ]]; then
        echo "  ↷ $uname (déjà configuré — utiliser --force pour réappliquer)"
        (( SKIPPED++ )) || true
    else
        echo "  ✓ $uname"
        su - "$uname" -c "bash '/Library/Application Support/com.carriez.RustDesk/apply_config.sh'" \
            && (( APPLIED++ )) || true
    fi
done < /etc/passwd

echo ""
echo "────────────────────────────────────────"
echo "  Configurés maintenant : $APPLIED"
echo "  Déjà configurés       : $SKIPPED"
echo "  Nouveaux utilisateurs : auto au prochain login"
echo ""
echo "  Fichiers déployés dans : $SYSDIR"
echo "  LaunchAgent            : $AGENT_PLIST"
echo ""
echo "  Pour forcer une ré-application de la config :"
echo "    sudo bash $(basename "$0") --force"
echo "  Pour réinstaller RustDesk + réappliquer la config :"
echo "    sudo bash $(basename "$0") --reinstall --force"
echo "────────────────────────────────────────"
