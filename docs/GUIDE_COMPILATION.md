# Guide de compilation et de packaging — Twitch Voice Reader

Ce guide couvre, dans l'ordre :
1. [Installer les prérequis](#1-installer-les-prérequis) (par OS)
2. [Récupérer le projet et ses dépendances](#2-récupérer-le-projet-et-ses-dépendances)
3. [Lancer l'application en mode développement](#3-lancer-lapplication-en-mode-développement)
4. [Créer l'installateur Windows (.exe / .msi)](#4-créer-linstallateur-windows-exe--msi)
5. [Créer le paquet Linux (.AppImage / .deb)](#5-créer-le-paquet-linux-appimage--deb)
6. [Créer l'installateur macOS (.app / .dmg)](#6-créer-linstallateur-macos-app--dmg)
7. [Construire les 3 plateformes sans posséder les 3 machines (CI)](#7-construire-les-3-plateformes-sans-posséder-les-3-machines-ci)
8. [Problèmes fréquents](#8-problèmes-fréquents)

**Point important à comprendre avant de commencer** : Tauri produit un
binaire **natif** pour chaque OS. On ne peut pas fabriquer un `.exe`
Windows fonctionnel depuis macOS ou Linux (le cross-compilation
Windows↔Linux↔macOS pour des applications avec WebView natif n'est pas
fiable, même si des workarounds existent pour certains cas simples). La
méthode fiable pour obtenir les 3 installateurs sans posséder 3 machines
est la CI GitHub Actions déjà fournie dans le projet (section 7) — elle
compile chaque cible sur un runner natif de l'OS correspondant.

---

## 1. Installer les prérequis

### 1.1 Commun aux trois plateformes

- **Node.js 18 ou plus récent** : <https://nodejs.org> (choisir la version LTS)
  Vérifier : `node -v` et `npm -v`
- **Rust (stable)** via rustup : <https://rustup.rs>
  Vérifier : `rustc --version` et `cargo --version`

### 1.2 Windows

1. **Microsoft C++ Build Tools** (nécessaire pour compiler les dépendances
   Rust natives) : installer "Desktop development with C++" depuis le
   [Visual Studio Installer](https://visualstudio.microsoft.com/visual-cpp-build-tools/).
2. **WebView2** : préinstallé sur Windows 10/11 à jour. Sinon :
   <https://developer.microsoft.com/microsoft-edge/webview2/>
3. Cible Rust par défaut (`x86_64-pc-windows-msvc`) déjà installée par
   rustup sous Windows — rien à faire de plus.
4. **PowerShell 5.1+** (préinstallé sous Windows 10/11).

### 1.3 Linux (Debian/Ubuntu — adapter pour votre distribution)

```bash
sudo apt update
sudo apt install -y \
  libwebkit2gtk-4.1-dev \
  libgtk-3-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev \
  patchelf \
  build-essential \
  curl \
  wget \
  file
```

Sur Fedora : `sudo dnf install webkit2gtk4.1-devel gtk3-devel libappindicator-gtk3-devel librsvg2-devel`
Sur Arch : `sudo pacman -S webkit2gtk-4.1 gtk3 libappindicator-gtk3 librsvg patchelf base-devel`

### 1.4 macOS

1. **Xcode Command Line Tools** :
   ```bash
   xcode-select --install
   ```
2. Cibles Rust pour un binaire universel (Intel + Apple Silicon) :
   ```bash
   rustup target add x86_64-apple-darwin aarch64-apple-darwin
   ```

---

## 2. Récupérer le projet et ses dépendances

Décompressez l'archive du projet, puis à la racine :

```bash
cd twitch-voice-reader
npm install
```

**Piper (moteur de synthèse vocale) n'a plus besoin d'être installé
manuellement avant le premier lancement** : l'application le télécharge et
l'installe automatiquement elle-même (binaire + voix française par
défaut) au démarrage si absent, avec une barre de progression visible
dans l'onglet "Voix et TTS" (voir `src-tauri/src/tts/installer.rs`).
Il vous faut simplement un accès réseau au premier lancement.

`scripts/install-piper.sh`/`.ps1` reste disponible et utile pour un cas
différent : **pré-embarquer** Piper à l'intérieur de l'installeur final
(`.exe`/`.deb`/`.dmg`) généré par `tauri build`, afin qu'un utilisateur
final n'ait besoin d'aucun accès réseau au tout premier lancement de
l'application installée. C'est ce que font les scripts `build-windows.ps1`
et `build-linux.sh` (section 4 et 5) — inutile de l'exécuter vous-même en
développement.

```bash
# Pour pré-embarquer Piper dans un build (optionnel en développement) :
bash scripts/install-piper.sh linux-x64   # ou : macos
./scripts/install-piper.ps1 -Platform windows-x64
```

> ⚠️ Le dépôt source de Piper (`rhasspy/piper`) a été **archivé** par son
> propriétaire (lecture seule depuis octobre 2025) ; le développement se
> poursuit sous [`OHF-Voice/piper1-gpl`](https://github.com/OHF-Voice/piper1-gpl),
> qui ne publie plus d'exécutable Windows autonome. Les anciens artefacts
> utilisés ici restent téléchargeables (l'archivage ne supprime pas les
> releases publiées) mais ne recevront plus de mise à jour — voir le
> rapport d'audit correspondant pour le détail et la piste de migration
> envisagée à plus long terme.

### Configurer `TWITCH_CLIENT_ID`

L'application a besoin d'un identifiant client Twitch **public** (pas de
secret, voir le cahier technique pour le détail du Device Code Flow) :

1. Créez une application sur <https://dev.twitch.tv/console/apps> (un
   bouton dans l'onglet Connexions Twitch de l'application ouvre
   directement cette page)
2. Récupérez le **Client ID**
3. **Depuis l'interface** (recommandé) : onglet *Connexions Twitch* →
   section *Configuration Twitch* → coller le Client ID → *Enregistrer*.
   Persisté automatiquement, aucune variable d'environnement nécessaire.

Alternative pour le développement/CI, si vous préférez ne pas passer par
l'interface (utilisée uniquement si aucune valeur n'est enregistrée) :

```bash
# Linux/macOS
export TWITCH_CLIENT_ID="votre_client_id"

# Windows PowerShell
$env:TWITCH_CLIENT_ID = "votre_client_id"
```

### Vérifier que tout compile

Avant de lancer l'application, il est recommandé de vérifier la
compilation du backend Rust seul (plus rapide qu'un build complet) :

```bash
cd src-tauri
cargo check
cd ..
```

Corrigez les éventuelles erreurs signalées avant de continuer (voir
section 8 pour les problèmes les plus courants).

---

## 3. Lancer l'application en mode développement

Depuis la racine du projet :

```bash
npm run tauri dev
```

Cette commande :
1. Démarre le serveur de développement Vite (frontend, port 1420)
2. Compile le backend Rust en mode debug
3. Ouvre la fenêtre de l'application avec rechargement à chaud du frontend

Le premier lancement est plus long (compilation Rust complète des
dépendances) ; les suivants sont incrémentaux et beaucoup plus rapides.

---

## 4. Créer l'installateur Windows (.exe / .msi)

**Doit être exécuté sur une machine Windows** (ou via la CI, section 7).

```powershell
$env:TWITCH_CLIENT_ID = "votre_client_id"
./scripts/build-windows.ps1
```

Ce script installe les dépendances npm, récupère Piper, puis lance
`tauri build`. Il produit deux formats d'installateur dans :

```
src-tauri/target/x86_64-pc-windows-msvc/release/bundle/
├── nsis/
│   └── Twitch Voice Reader_0.1.0_x64-setup.exe   <- installateur .exe (NSIS)
└── msi/
    └── Twitch Voice Reader_0.1.0_x64_en-US.msi   <- installateur .msi
```

Le `.exe` NSIS est généralement préféré pour une distribution grand public
(installation plus légère, personnalisable — langue française/anglaise déjà
configurée dans `tauri.conf.json`).

### Signature de code (optionnel mais recommandé pour la distribution publique)

Sans signature, Windows SmartScreen affichera un avertissement "Éditeur
inconnu" au premier lancement. Pour signer :

1. Obtenir un certificat de signature de code (Authenticode) auprès d'une
   autorité de certification (ex: DigiCert, Sectigo) — généralement payant.
2. Configurer les variables `TAURI_SIGNING_PRIVATE_KEY` et le certificat
   dans `tauri.conf.json` → `bundle.windows.certificateThumbprint`, ou
   fournir les secrets équivalents à `tauri-apps/tauri-action` en CI (voir
   section 7).

Sans certificat, l'installateur reste **parfaitement fonctionnel** — seul
l'avertissement Windows Defender/SmartScreen apparaît, que l'utilisateur
peut contourner via "Informations complémentaires → Exécuter quand même".

---

## 5. Créer le paquet Linux (.AppImage / .deb)

**Doit être exécuté sur une machine Linux** (ou via la CI, section 7).

```bash
export TWITCH_CLIENT_ID="votre_client_id"
bash scripts/build-linux.sh
```

Produit dans `src-tauri/target/x86_64-unknown-linux-gnu/release/bundle/` :

```
appimage/
└── twitch-voice-reader_0.1.0_amd64.AppImage   <- exécutable autonome, aucune installation requise
deb/
└── twitch-voice-reader_0.1.0_amd64.deb        <- paquet Debian/Ubuntu (apt/dpkg)
```

- L'**AppImage** fonctionne sur la plupart des distributions sans
  installation : `chmod +x *.AppImage && ./twitch-voice-reader*.AppImage`
- Le **.deb** s'installe via `sudo dpkg -i twitch-voice-reader_*.deb`
  (ou double-clic dans un gestionnaire de paquets graphique).

Pour générer aussi un `.rpm` (Fedora/openSUSE), ajoutez `rpm` à la liste
`bundle.targets` dans `src-tauri/tauri.conf.json` (actuellement `"all"`,
ce qui inclut déjà toutes les cibles disponibles sur la plateforme de
build — vérifiez que `rpmbuild` est installé sur la machine de build).

---

## 6. Créer l'installateur macOS (.app / .dmg)

**Doit être exécuté sur une machine macOS** (Apple ne permet pas de
compiler ni de notariser un `.app` macOS depuis un autre OS — ou via la
CI, section 7, qui utilise un runner `macos-latest`).

```bash
export TWITCH_CLIENT_ID="votre_client_id"
bash scripts/build-macos.sh
```

Produit un binaire **universel** (Intel + Apple Silicon en un seul
fichier) dans `src-tauri/target/universal-apple-darwin/release/bundle/` :

```
macos/
└── Twitch Voice Reader.app
dmg/
└── Twitch Voice Reader_0.1.0_universal.dmg   <- image disque d'installation
```

### Signature et notarization Apple (nécessaire pour une distribution sans avertissement Gatekeeper)

Sans signature Apple, macOS bloquera l'ouverture de l'application
("impossible d'ouvrir car l'éditeur ne peut être vérifié"). Pour signer et
notariser :

1. Un compte **Apple Developer Program** (99 $/an)
2. Un certificat "Developer ID Application" créé depuis Xcode ou le
   portail développeur Apple
3. Fournir en variables d'environnement lors du build :
   ```bash
   export APPLE_CERTIFICATE="<certificat .p12 encodé en base64>"
   export APPLE_CERTIFICATE_PASSWORD="..."
   export APPLE_SIGNING_IDENTITY="Developer ID Application: Votre Nom (TEAMID)"
   export APPLE_ID="votre@identifiant.apple"
   export APPLE_PASSWORD="mot-de-passe-application-spécifique"
   export APPLE_TEAM_ID="TEAMID"
   ```
4. Relancer `tauri build` — Tauri signe et notarise automatiquement quand
   ces variables sont présentes.

Ces mêmes variables sont directement utilisables comme secrets GitHub dans
le workflow CI fourni (`.github/workflows/build.yml`, lignes commentées à
décommenter).

---

## 7. Construire les 3 plateformes sans posséder les 3 machines (CI)

Le projet inclut déjà `.github/workflows/build.yml`, qui compile les trois
cibles en parallèle sur des runners GitHub natifs (Windows, Ubuntu, macOS)
— c'est la méthode recommandée pour produire les 3 installateurs sans
matériel Apple/Windows/Linux dédié.

### Mise en place

1. Poussez le projet sur un dépôt GitHub.
2. Dans **Settings → Secrets and variables → Actions**, ajoutez le secret
   `TWITCH_CLIENT_ID` avec votre Client ID Twitch.
3. (Optionnel, pour la signature) ajoutez les secrets Windows/Apple
   mentionnés aux sections 4 et 6.
4. Déclenchez le workflow :
   - automatiquement en poussant un tag `v*` (ex: `git tag v0.1.0 && git push --tags`)
   - ou manuellement depuis l'onglet **Actions → Build Twitch Voice Reader → Run workflow**
5. Une fois le workflow terminé, les installateurs des 3 plateformes sont
   téléchargeables comme **artefacts** du run (section "Artifacts" en bas
   de la page du run).

Cette approche est aussi celle à utiliser en continu pour toute release
future : elle garantit que chaque plateforme est compilée sur son OS
natif, dans un environnement propre et reproductible.

---

## 8. Problèmes fréquents

| Symptôme | Cause probable | Solution |
|---|---|---|
| `cargo: command not found` | Rust non installé ou pas dans le PATH | Réinstaller via rustup, redémarrer le terminal |
| Erreur `webkit2gtk` introuvable (Linux) | Dépendances système manquantes | Voir section 1.3 |
| `error: Microsoft Visual C++ 14.0 or greater is required` (Windows) | Build Tools C++ non installés | Voir section 1.2 |
| L'authentification Twitch échoue immédiatement | Aucun Client ID Twitch configuré | Renseignez-le dans l'onglet "Connexions Twitch" (section Configuration Twitch), voir section 2 |
| Bannière "Moteur Piper introuvable" persistante dans l'onglet Voix et TTS | Échec du téléchargement automatique (pas de réseau au premier lancement, pare-feu/proxy d'entreprise) | Cliquez sur "Réessayer" une fois le réseau disponible ; en environnement sans accès réseau, pré-embarquez Piper au build avec `scripts/install-piper.sh`/`.ps1` (voir section 2) |
| Aucun son ne sort alors que "Moteur vocal prêt" s'affiche | Périphérique audio invalide/débranché | Tester un autre périphérique dans l'onglet "Voix et TTS" |
| Build très long au premier lancement | Normal | Rust compile toutes les dépendances natives la première fois (plusieurs minutes) ; les builds suivants sont incrémentaux |
| macOS : "l'éditeur ne peut être vérifié" | Application non signée/notariée | Voir section 6, ou clic droit → Ouvrir pour contourner ponctuellement |
| Windows : SmartScreen bloque l'installateur | Application non signée | Voir section 4, ou "Informations complémentaires → Exécuter quand même" |

Pour toute erreur de compilation Rust non listée ici, exécutez
`cargo check` dans `src-tauri/` pour obtenir un message d'erreur précis
(fichier, ligne, cause) avant de chercher plus loin.
