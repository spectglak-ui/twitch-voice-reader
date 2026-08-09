# Cahier technique — Twitch Voice Reader

Version 0.1 — Document vivant, à mettre à jour à chaque évolution
d'architecture significative.

## 1. Objectif et périmètre

Application de bureau multiplateforme lisant à voix haute, via synthèse
vocale locale, le chat d'une ou plusieurs chaînes Twitch, à destination des
streamers. Le périmètre MVP couvre : authentification, connexion
multi-chaînes, lecture TTS configurable, filtres, anti-spam, overlay,
historique, statistiques, export/import de configuration.

## 2. Stack technique

| Couche | Technologie | Justification |
|---|---|---|
| Shell applicatif | Tauri 2.x | Binaire natif léger (vs Electron), sécurité par capacités, accès système contrôlé |
| Backend | Rust (edition 2021) | Performance, sûreté mémoire, écosystème async mature (Tokio) |
| Frontend | React 18 + TypeScript + Vite | Écosystème riche, typage fort partagé avec le backend via miroir manuel |
| Style | Tailwind CSS | Cohérence de design tokenisée, pas de CSS-in-JS coûteux à l'exécution |
| Base de données | SQLite (rusqlite + r2d2) | Embarquée, zéro configuration, suffisante pour un usage mono-utilisateur |
| TTS | Piper (processus externe) | Neuronal, local, hors ligne, multi-langue, licence permissive |
| Audio | rodio + cpal | Cross-platform, API haut niveau suffisante pour de la lecture séquentielle |

## 3. Architecture générale

```
┌─────────────────────────────┐        IPC (invoke/emit)       ┌──────────────────────────────┐
│      Frontend (WebView)      │ ◄─────────────────────────────► │          Backend (Rust)        │
│  React + Zustand + Tailwind  │                                  │                                │
└─────────────────────────────┘                                  │  ┌──────────┐  ┌─────────────┐  │
                                                                   │  │  twitch  │  │     tts      │  │
                                                                   │  └────┬─────┘  └──────┬──────┘  │
                                                                   │       │               │          │
                                                                   │       ▼               ▼          │
                                                                   │  ┌──────────────────────────┐    │
                                                                   │  │   Pipeline central         │   │
                                                                   │  │  (filters -> anti-spam ->  │   │
                                                                   │  │   db -> tts queue -> UI)   │   │
                                                                   │  └──────────────────────────┘    │
                                                                   │       │               │          │
                                                                   │       ▼               ▼          │
                                                                   │  ┌──────────┐  ┌─────────────┐  │
                                                                   │  │    db     │  │   overlay    │  │
                                                                   │  └──────────┘  └─────────────┘  │
                                                                   └──────────────────────────────────┘
```

### 3.1 Modèle de concurrence

- **Un acteur asynchrone par chaîne Twitch** (tâche Tokio dédiée, backoff
  exponentiel indépendant) — voir `twitch/connection_manager.rs`.
- **Un pipeline central unique** consomme tous les évènements
  multi-chaînes, applique filtres + anti-spam, journalise, et pousse vers
  la file TTS — garantit un traitement cohérent indépendant du nombre de
  chaînes connectées.
- **Une file TTS séquentielle unique** (une seule lecture à la fois, quelle
  que soit la source) — condition nécessaire à l'intelligibilité.
- **Un thread OS dédié pour l'audio** (`audio/player.rs`) : `rodio::OutputStream`
  n'étant pas `Send` de manière fiable sur toutes les plateformes, le flux
  audio est possédé exclusivement par un thread système communiquant par
  canal de commandes — voir la documentation de module pour le détail.

## 4. Choix techniques comparés

### 4.1 Authentification Twitch : Device Code Flow

Twitch ne supporte pas PKCE pour les clients publics (confirmé sur le forum
développeurs Twitch). Alternatives évaluées :

| Flux | Secret embarqué | Serveur local | UX |
|---|:---:|:---:|---|
| Authorization Code | Oui (risque de fuite) | Oui | Bonne mais complexe multiplateforme |
| Implicit | Non | Non | Dépréciée par Twitch |
| **Device Code (retenu)** | **Non** | **Non** | Code à copier-coller (acceptable, standard CLI/TV) |

### 4.2 Réception du chat : IRC WebSocket vs EventSub

IRC over WebSocket retenu pour le MVP (stable, pas de gestion de session
complexe, lecture des tags suffisante). EventSub documenté comme migration
possible si Twitch fait évoluer ses priorités (voir `twitch/irc_client.rs`).

### 4.3 Synthèse vocale : Piper en sous-processus

Un processus Piper est lancé par message, sortie PCM brute sur stdout (pas
de fichier temporaire). Alternative "process persistant" documentée comme
optimisation Phase 3 si la latence perçue devient un problème (voir
`tts/piper.rs`).

### 4.4 Base de données : rusqlite (sync) + r2d2 vs sqlx (async)

rusqlite retenu : complexité d'intégration moindre, empreinte binaire plus
faible, suffisant pour une charge mono-utilisateur. `sqlx` deviendrait
pertinent en cas de synchronisation cloud multi-appareils (hors périmètre
actuel).

### 4.5 Stockage des jetons : trousseau système vs fichier

Les jetons OAuth sont stockés via la crate `keyring` (Credential
Manager/Keychain/Secret Service), **jamais** dans `config.json` — pour que
l'export/import de configuration ne puisse jamais exposer de secret (voir
`twitch/token_store.rs`).

### 4.6 Détection de langue : whatlang vs lingua-rs

`whatlang` retenu pour sa rapidité sur texte court (messages de chat) et
son empreinte minimale, au prix d'une précision légèrement inférieure sur
les textes très courts — atténué par un seuil de confiance
(`is_reliable()`) avec repli sur la voix par défaut.

## 5. Schéma de base de données

Voir `src-tauri/src/db/schema.sql`. Tables :

- `message_history` — historique consultable (purge automatique selon rétention configurée)
- `stats_daily` — agrégats journaliers (messages lus/ignorés, temps de lecture)
- `known_users` — utilisateurs vus récemment (compteur d'utilisateurs actifs, suggestions d'attribution de voix)

## 6. Contrat IPC frontend ↔ backend

Les types TypeScript (`src/types/*.ts`) sont un **miroir manuel** des
structures Rust (`src-tauri/src/**/*.rs`). Choix assumé : pas de
génération automatique de types (ex: `ts-rs`, `specta`) pour le MVP, afin
de limiter la complexité de build initiale. **Recommandation Phase 3** :
migrer vers `specta` + `tauri-specta` pour garantir la cohérence par
construction plutôt que par discipline manuelle, à mesure que la surface
IPC grossit.

Convention de nommage : les commandes Tauri utilisent des paramètres
`snake_case` côté Rust ; le pont IPC de Tauri convertit automatiquement
depuis du `camelCase` côté JavaScript — c'est pourquoi `src/lib/tauri.ts`
appelle les commandes avec des clés `camelCase`.

## 7. Sécurité

- Jetons OAuth jamais persistés en clair (trousseau système)
- CSP stricte (`tauri.conf.json`) limitant les origines réseau autorisées
- Aucune commande n'exécute de code arbitraire fourni par le chat Twitch
  (les messages sont traités comme des données, jamais interprétés)
- Le texte envoyé à Piper passe par stdin (pas d'interpolation shell)
- Entitlements macOS minimaux (réseau + accès fichiers utilisateur uniquement)

## 8. CI/CD et distribution

`.github/workflows/build.yml` construit les trois cibles sur des runners
natifs (windows-latest, ubuntu-22.04, macos-latest) via
`tauri-apps/tauri-action`. Non inclus par défaut (nécessite des secrets
supplémentaires à fournir séparément) :

- Signature Authenticode Windows
- Signature + notarization Apple (compte développeur payant requis)

## 9. Limitations connues et pistes d'évolution

| Limitation | Impact | Piste de résolution |
|---|---|---|
| Pitch-shifting couplé au tempo | Qualité audio du réglage "hauteur de voix" | Intégrer `rubato` + phase-vocoder (Phase 3) |
| Un process Piper par message | ~50-150ms de latence de démarrage | Process Piper persistant avec framing stdin/stdout (Phase 3) |
| Pas de génération automatique de types IPC | Risque de désynchronisation manuelle Rust/TS | Migrer vers `specta`/`tauri-specta` (Phase 3) |
| EventSub non implémenté | Pas d'accès aux données enrichies EventSub | Ajouter une implémentation alternative derrière la même interface `ChannelEvent` (Phase 4) |
| Démarrage minimisé : fenêtre cachée mais pas de vérification de support tray sur toutes les distributions Linux | Comportement tray potentiellement absent sous certains DE Linux minimalistes | Documenter dans l'aide utilisateur, prévoir un repli "fermer = quitter" configurable |

## 10. Conventions de code

- Tout module public documenté en commentaires `///`/`//!` expliquant le
  **pourquoi**, pas seulement le quoi.
- Erreurs unifiées via `AppError` (`thiserror`), jamais de `unwrap()` en
  dehors des tests.
- Tests unitaires colocalisés (`#[cfg(test)] mod tests`) sur la logique
  pure (parsing IRC, filtres, anti-spam).
