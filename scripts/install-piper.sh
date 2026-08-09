#!/usr/bin/env bash
# Télécharge le binaire Piper et les voix par défaut dans
# `src-tauri/resources/piper/`, bundlé dans l'application finale via
# `bundle.resources` (tauri.conf.json).
#
# Usage : install-piper.sh <linux-x64|macos>
#
# Pour macOS, l'architecture (Intel vs Apple Silicon) est détectée
# automatiquement via `uname -m` — voir la note ci-dessous sur les noms
# d'archive.
#
# Source officielle : https://github.com/rhasspy/piper/releases
#   ⚠️ Ce dépôt a été archivé par son propriétaire (lecture seule depuis
#   octobre 2025) ; le développement se poursuit sous
#   https://github.com/OHF-Voice/piper1-gpl, qui ne publie plus
#   d'exécutable autonome (distribution via `pip install piper-tts`
#   uniquement désormais). Ce script cible donc délibérément la dernière
#   version figée (2023.11.14-2) — les anciens artefacts restent
#   téléchargeables malgré l'archivage.
# Voix officielles   : https://github.com/rhasspy/piper/blob/master/VOICES.md
#
# Remarque : l'application elle-même sait désormais aussi télécharger
# Piper automatiquement au premier lancement si absent (voir
# `src-tauri/src/tts/installer.rs`) — ce script reste utile pour
# *packager* Piper à l'intérieur de l'installeur final (afin qu'un
# utilisateur final n'ait besoin d'aucun accès réseau), mais n'est plus un
# prérequis pour que l'application fonctionne en développement.

set -euo pipefail

PLATFORM="${1:?Usage: install-piper.sh <linux-x64|macos>}"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEST_DIR="$ROOT_DIR/src-tauri/resources/piper"
PIPER_VERSION="2023.11.14-2"
# En dessous de ce seuil, la réponse HTTP est presque certainement une
# page d'erreur plutôt que l'archive réelle (plusieurs dizaines de Mo) —
# on préfère échouer immédiatement avec un message clair que de laisser
# `tar` produire une extraction vide ou corrompue sans expliquer pourquoi.
MIN_PLAUSIBLE_BYTES=5000000

mkdir -p "$DEST_DIR"

case "$PLATFORM" in
  linux-x64)
    ASSET="piper_linux_x86_64.tar.gz"
    ;;
  macos)
    # Les releases Piper publient deux archives macOS distinctes (pas de
    # binaire universel) — voir .github/workflows/main.yml du projet
    # source. On sélectionne celle qui correspond à la machine courante.
    case "$(uname -m)" in
      arm64) ASSET="piper_macos_aarch64.tar.gz" ;;
      x86_64) ASSET="piper_macos_x64.tar.gz" ;;
      *)
        echo "Architecture macOS non reconnue : $(uname -m)" >&2
        exit 1
        ;;
    esac
    ;;
  *)
    echo "Plateforme inconnue : $PLATFORM (attendu: linux-x64 ou macos)" >&2
    exit 1
    ;;
esac

if [[ ! -f "$DEST_DIR/piper" ]]; then
  echo "-> Téléchargement de Piper $PIPER_VERSION ($ASSET)..."
  TMP_ARCHIVE="$(mktemp)"
  trap 'rm -f "$TMP_ARCHIVE"' EXIT

  curl -L --fail --show-error -o "$TMP_ARCHIVE" \
    "https://github.com/rhasspy/piper/releases/download/$PIPER_VERSION/$ASSET"

  ACTUAL_SIZE=$(wc -c < "$TMP_ARCHIVE")
  if [[ "$ACTUAL_SIZE" -lt "$MIN_PLAUSIBLE_BYTES" ]]; then
    echo "Erreur : fichier téléchargé anormalement petit ($ACTUAL_SIZE octets)." >&2
    echo "Il s'agit probablement d'une page d'erreur plutôt que de l'archive Piper." >&2
    exit 1
  fi

  # IMPORTANT : pas de --strip-components ici. Les archives Piper sont
  # plates (piper, espeak-ng-data/, *.onnx... directement à la racine de
  # l'archive, pas dans un sous-dossier) — un --strip-components=1 aurait
  # ici pour effet de perdre le binaire `piper` lui-même (un fichier à la
  # racine n'a pas de composant à retirer). C'était un bug réel de ce
  # script avant correction.
  tar -xzf "$TMP_ARCHIVE" -C "$DEST_DIR"
else
  echo "-> Binaire Piper déjà présent, téléchargement ignoré."
fi

if [[ ! -f "$DEST_DIR/piper" ]]; then
  # Filet de sécurité : si la structure de l'archive changeait un jour
  # (dossier imbriqué), on cherche avant d'abandonner plutôt que de
  # laisser l'utilisateur avec un dossier "resources/piper" vide et aucune
  # explication — exactement le symptôme initialement rapporté.
  FOUND="$(find "$DEST_DIR" -maxdepth 2 -type f -name piper | head -n1)"
  if [[ -n "$FOUND" ]]; then
    mv "$FOUND" "$DEST_DIR/piper"
  else
    echo "Erreur : l'archive a été extraite mais aucun binaire 'piper' n'a été trouvé." >&2
    echo "Contenu extrait :" >&2
    find "$DEST_DIR" -maxdepth 2 >&2
    exit 1
  fi
fi

chmod +x "$DEST_DIR/piper"

# Smoke test : confirme que le binaire s'exécute réellement (mauvaise
# architecture, binaire corrompu, etc. sinon détectés bien plus tard,
# silencieusement, au moment du premier vrai essai de synthèse).
if ! "$DEST_DIR/piper" --version >/dev/null 2>&1; then
  echo "Attention : le binaire Piper téléchargé ne s'exécute pas correctement sur cette machine." >&2
fi

# Voix française par défaut (siwis, qualité medium) — les autres voix
# proposées dans l'application (EN/ES/DE/IT) sont téléchargeables à la
# demande depuis l'onglet "Voix et TTS" plutôt que bundlées par défaut,
# pour garder un installeur de taille raisonnable (~60 Mo/voix).
VOICES_DIR="$ROOT_DIR/src-tauri/resources/piper/voices"
mkdir -p "$VOICES_DIR"
DEFAULT_VOICE="fr_FR-siwis-medium"
BASE_URL="https://huggingface.co/rhasspy/piper-voices/resolve/main/fr/fr_FR/siwis/medium"

if [[ ! -f "$VOICES_DIR/$DEFAULT_VOICE.onnx" ]]; then
  echo "-> Téléchargement de la voix par défaut ($DEFAULT_VOICE)..."
  curl -L --fail --show-error -o "$VOICES_DIR/$DEFAULT_VOICE.onnx" "$BASE_URL/$DEFAULT_VOICE.onnx"
  curl -L --fail --show-error -o "$VOICES_DIR/$DEFAULT_VOICE.onnx.json" "$BASE_URL/$DEFAULT_VOICE.onnx.json"

  VOICE_SIZE=$(wc -c < "$VOICES_DIR/$DEFAULT_VOICE.onnx")
  if [[ "$VOICE_SIZE" -lt 1000000 ]]; then
    echo "Erreur : modèle de voix téléchargé anormalement petit ($VOICE_SIZE octets)." >&2
    exit 1
  fi
fi

echo "-> Piper installé et vérifié dans $DEST_DIR"
