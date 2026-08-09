#!/usr/bin/env bash
# Build de production de Twitch Voice Reader pour Linux (x86_64).
#
# Prérequis système (Debian/Ubuntu) :
#   sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev \
#                     librsvg2-dev patchelf build-essential curl
#
# Nécessite TWITCH_CLIENT_ID défini en variable d'environnement.

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "== Twitch Voice Reader — Build Linux =="

command -v node >/dev/null || { echo "Node.js est requis." >&2; exit 1; }
command -v cargo >/dev/null || { echo "Rust/Cargo est requis (https://rustup.rs)." >&2; exit 1; }

if [[ -z "${TWITCH_CLIENT_ID:-}" ]]; then
  echo "AVERTISSEMENT : TWITCH_CLIENT_ID n'est pas défini (auth Twitch indisponible à l'exécution)." >&2
fi

echo "-> Installation des dépendances npm..."
npm ci

echo "-> Récupération du moteur Piper (Linux x86_64)..."
bash "$SCRIPT_DIR/install-piper.sh" linux-x64

echo "-> Build Tauri (release)..."
npm run tauri build -- --target x86_64-unknown-linux-gnu

echo "== Build terminé =="
echo "Paquets générés dans src-tauri/target/x86_64-unknown-linux-gnu/release/bundle/ (.deb, .AppImage)"
