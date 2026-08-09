#!/usr/bin/env bash
# Build de production de Twitch Voice Reader pour macOS (universal : Intel + Apple Silicon).
#
# Nécessite Xcode Command Line Tools, Rust (cibles x86_64-apple-darwin et
# aarch64-apple-darwin) et TWITCH_CLIENT_ID en variable d'environnement.
#
# La signature de code / notarisation Apple (requise pour une distribution
# publique hors App Store sans avertissement Gatekeeper) n'est PAS incluse
# ici : elle nécessite un compte Apple Developer payant et des secrets
# (APPLE_CERTIFICATE, APPLE_ID, APPLE_PASSWORD) à fournir séparément. Voir
# le job `build-macos` de `.github/workflows/build.yml` pour l'intégration
# CI complète avec signature.

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "== Twitch Voice Reader — Build macOS =="

command -v node >/dev/null || { echo "Node.js est requis." >&2; exit 1; }
command -v cargo >/dev/null || { echo "Rust/Cargo est requis (https://rustup.rs)." >&2; exit 1; }

rustup target add x86_64-apple-darwin aarch64-apple-darwin

if [[ -z "${TWITCH_CLIENT_ID:-}" ]]; then
  echo "AVERTISSEMENT : TWITCH_CLIENT_ID n'est pas défini (auth Twitch indisponible à l'exécution)." >&2
fi

echo "-> Installation des dépendances npm..."
npm ci

# Piper n'est volontairement PAS pré-embarqué dans ce build macOS : les
# releases Piper ne publient pas de binaire universel (Intel + Apple
# Silicon), seulement deux archives séparées par architecture. Embarquer
# l'une des deux dans ce bundle *universel* casserait la moitié des Macs
# (Intel si on embarque l'ARM, ou l'inverse). L'application détecte et
# télécharge automatiquement la bonne archive pour l'architecture réelle
# de la machine au premier lancement (voir `src-tauri/src/tts/installer.rs`)
# — c'est le comportement correct pour un binaire universel. Pour un build
# mono-architecture (`--target x86_64-apple-darwin` ou
# `aarch64-apple-darwin` seul), `install-piper.sh macos` reste utilisable
# pour pré-embarquer Piper sans dépendre du réseau au premier lancement.

echo "-> Build Tauri (release, universal binary)..."
npm run tauri build -- --target universal-apple-darwin

echo "== Build terminé =="
echo "Bundle généré dans src-tauri/target/universal-apple-darwin/release/bundle/ (.app, .dmg)"
echo "Piper sera téléchargé automatiquement au premier lancement (nécessite un accès réseau)."
