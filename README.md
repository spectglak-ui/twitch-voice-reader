# Twitch Voice Reader

<img width="500" height="500" alt="TTS (1)" src="https://github.com/user-attachments/assets/efc6bfd4-0e8e-4e17-b9c1-0870c7d229c7" />

Application de bureau (Windows / Linux / macOS) qui lit à voix haute le chat
d'une ou plusieurs chaînes Twitch en temps réel, via une synthèse vocale
**100 % locale et hors ligne** ([Piper](https://github.com/rhasspy/piper)).

Construite avec **Tauri 2**, **Rust** et **React + TypeScript**.

---

## Sommaire

- [Fonctionnalités](#fonctionnalités)
- [Démarrage rapide (développement)](#démarrage-rapide-développement)
- [Configuration du Client ID Twitch](#configuration-du-client-id-twitch)
- [Build de production](#build-de-production)
- [Architecture](#architecture)
- [Documentation complémentaire](#documentation-complémentaire)
- [Limitations connues](#limitations-connues)

---

## Fonctionnalités

- Connexion multi-chaînes Twitch (Device Code Flow OAuth), reconnexion automatique
- Lecture vocale en temps réel avec file d'attente séquentielle intelligente
- Voix Piper multiples, réglables (volume, vitesse, hauteur), attribuables par utilisateur ou par rôle
- Détection automatique de la langue (FR/EN/ES/DE/IT) avec voix adaptée
- Filtres complets (longueur, emotes, liens, listes noire/blanche, rôles, utilisateurs ignorés)
- Anti-spam (déduplication, regroupement, limite de débit)
- Overlay web (Browser Source OBS/Streamlabs) avec animation temps réel
- Historique local (SQLite) et statistiques (session + persistées)
- Export/import de configuration JSON, réinitialisation
- Support multi-périphérique audio, tray icon, autodémarrage système

## Démarrage rapide (développement)

### Prérequis

- [Node.js](https://nodejs.org) ≥ 18
- [Rust](https://rustup.rs) (stable) + cibles de votre plateforme
- Dépendances système Tauri : voir la [documentation officielle](https://v2.tauri.app/start/prerequisites/)
- [Piper](https://github.com/rhasspy/piper) + au moins une voix `.onnx` (voir `scripts/install-piper.sh` / `.ps1`)

### Installation

```bash
npm install
bash scripts/install-piper.sh linux-x64   # ou macos-universal ; sous Windows : scripts/install-piper.ps1
export TWITCH_CLIENT_ID="votre_client_id"  # optionnel : peut aussi être renseigné depuis l'interface, voir section suivante
npm run tauri dev
```

## Configuration du Client ID Twitch

L'application utilise le **Device Code Grant Flow** de Twitch (voir
justification technique dans `src-tauri/src/twitch/auth.rs`), qui ne
nécessite **aucun secret client**, uniquement un `client_id` **public**.

1. Créez une application sur <https://dev.twitch.tv/console/apps> (un
   bouton "Créer une application Twitch" dans l'onglet Connexions de
   l'application ouvre directement cette page)
2. Type d'OAuth Client : **Public** (aucune URL de redirection n'est nécessaire pour le Device Flow)
3. Copiez le Client ID généré

**Deux façons de le renseigner**, par ordre de priorité :

1. **Depuis l'interface** (recommandé pour un utilisateur final) : onglet
   *Connexions Twitch* → section *Configuration Twitch* → coller le Client
   ID → *Enregistrer*. Persisté dans `config.json` et pris en compte
   immédiatement, sans redémarrage. C'est la méthode destinée aux
   utilisateurs finaux d'un build distribué.
2. **Variable d'environnement** (pratique en développement/CI, utilisée
   uniquement si aucune valeur n'est enregistrée depuis l'interface) :

   ```bash
   export TWITCH_CLIENT_ID="xxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
   ```

En CI (GitHub Actions), définissez le secret de dépôt `TWITCH_CLIENT_ID`
(déjà référencé dans `.github/workflows/build.yml`).

## Build de production

```bash
# Windows (PowerShell)
./scripts/build-windows.ps1

# Linux
./scripts/build-linux.sh

# macOS
./scripts/build-macos.sh
```

Chaque script installe les dépendances, télécharge Piper + la voix par
défaut, puis lance `tauri build`. Les installeurs générés se trouvent dans
`src-tauri/target/<cible>/release/bundle/`.

⚠️ La **signature de code** (Windows Authenticode / macOS notarization)
n'est pas incluse par défaut — voir le cahier technique, section 8, pour
les secrets à fournir en CI.

## Architecture

Voir [`docs/CAHIER_TECHNIQUE.md`](docs/CAHIER_TECHNIQUE.md) pour le détail
complet (schémas, choix techniques comparés, base de données, sécurité) et
[`docs/PLAN_DE_DEVELOPPEMENT.md`](docs/PLAN_DE_DEVELOPPEMENT.md) pour le
séquencement des phases.

Résumé :

```
frontend (React/TS)  <-- IPC Tauri -->  backend (Rust)
                                           ├── twitch/    (auth, IRC WS, multi-chaînes)
                                           ├── tts/       (Piper, langue, file de lecture)
                                           ├── audio/     (thread dédié rodio/cpal)
                                           ├── filters/   (règles, anti-spam)
                                           ├── config/    (schéma, persistance JSON)
                                           ├── db/        (SQLite : historique, stats)
                                           ├── overlay/   (serveur HTTP+WS pour OBS)
                                           └── stats/     (compteurs de session)
```

## Documentation complémentaire

- [`docs/CAHIER_TECHNIQUE.md`](docs/CAHIER_TECHNIQUE.md) — spécification technique détaillée
- [`docs/PLAN_DE_DEVELOPPEMENT.md`](docs/PLAN_DE_DEVELOPPEMENT.md) — plan de développement par phases

## Limitations connues

Documentées honnêtement (pas de simplification cachée) — voir le cahier
technique section 9 pour le détail et les pistes de résolution :

1. **Pitch-shifting** approximatif (couplé à la vitesse), un vrai
   pitch-shift indépendant nécessite un phase-vocoder (`rubato`).
2. **Latence Piper** : un processus est démarré par message (~50-150 ms de
   surcoût), suffisant pour l'usage visé mais un mode "process persistant"
   est documenté comme optimisation possible.
3. **EventSub** non implémenté (IRC WebSocket utilisé pour le MVP) — voir
   comparaison dans `twitch/irc_client.rs`.
4. Code non compilé dans l'environnement de génération (pas de toolchain
   Rust/réseau disponible) : passez `cargo check` avant votre premier build.
