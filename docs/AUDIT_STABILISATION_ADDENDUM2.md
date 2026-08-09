# Audit de stabilisation — Addendum 2

Fait suite à `AUDIT_STABILISATION.md` (audit initial). Ce document couvre
uniquement les éléments nouveaux depuis : vérification de vos 4 correctifs
manuels, les 2 nouveaux problèmes remontés, la re-vérification du workflow
de connexion/déconnexion, et un nouveau balayage général sur l'état actuel
du code.

---

## 1. Vérification de vos correctifs manuels

| # | Correctif | Vérifié | Détail |
|---|---|---|---|
| 1 | `tokio::spawn` → `tauri::async_runtime::spawn` dans `lib.rs`, `tts/queue.rs`, `connection_manager.rs` | ✅ Confirmé + étendu | Un 4ᵉ site (`overlay/server.rs`) et un 5ᵉ (`commands/auth.rs`) avaient exactement le même défaut et n'étaient pas mentionnés dans votre liste — corrigés également. Recherche exhaustive relancée (`grep -rn "tokio::spawn" src-tauri/src`) : **zéro occurrence restante** en dehors des commentaires explicatifs. |
| 2 | `JoinHandle` : `tokio::task::JoinHandle` → `tauri::async_runtime::JoinHandle` | ✅ Appliqué (n'avait pas encore été répercuté dans la copie livrée) | Confirme au passage que `tauri::async_runtime::JoinHandle` est bien un type distinct de celui de `tokio` dans votre version de Tauri — information utile, ajoutée en commentaire de code pour ne pas retomber dans le piège. |
| 3 | `resources/piper` manquant | ✅ Cause racine identifiée et corrigée à la source | Le dossier était exclu en totalité par `.gitignore` (`src-tauri/resources/piper/`) — un clone/extraction neuf ne l'avait donc jamais, et Tauri exige que chaque entrée de `bundle.resources` existe physiquement pour démarrer, d'où l'erreur exacte que vous avez rencontrée. Corrigé : le dossier existe désormais physiquement (`.gitkeep`), seul son **contenu** volumineux reste ignoré par git. |
| 4 | Double logger | ✅ Cause identifiée avec confiance et corrigée | `tracing_subscriber::fmt().init()` **et** `tauri-plugin-log` tentent chacun d'installer le logger global du crate `log` (le crate `log` n'autorise qu'un seul logger pour toute la durée de vie du process — la seconde tentative échoue systématiquement). Retiré `tauri-plugin-log` (plugin **et** dépendance `Cargo.toml`) ; conservé `tracing_subscriber`, déjà utilisé partout dans le code. |

**Aucune régression détectée** dans les zones touchées par ces 4 correctifs
(relecture complète de `lib.rs`, vérification d'équilibre des blocs sur
tous les fichiers modifiés).

---

## 2. Critique — Client ID Twitch configurable depuis l'interface

### Implémenté

- **Backend** : nouveau champ `TwitchConfig { client_id: Option<String> }`
  dans le schéma de configuration (`config/schema.rs`), avec
  `#[serde(default)]` — **important** : sans cet attribut, charger un
  `config.json` existant (créé avant cette fonctionnalité) aurait fait
  échouer toute la désérialisation, et `ConfigStore::load` serait
  silencieusement retombé sur une configuration par défaut, **effaçant au
  passage tous vos réglages déjà enregistrés** (chaînes, TTS, filtres...).
  C'est un piège classique lors de l'ajout d'un champ à un schéma déjà
  distribué — corrigé dès l'implémentation plutôt que découvert plus tard.
- Nouvelle commande `update_twitch_config` avec validation stricte côté
  **backend** (jamais uniquement côté frontend, contournable) : un Client
  ID vide ou uniquement composé d'espaces est rejeté explicitement.
- `twitch::auth::resolve_client_id()` : priorité config utilisateur →
  variable d'environnement → placeholder, recalculée à **chaque tentative
  de connexion** plutôt que figée au démarrage — enregistrer un nouveau
  Client ID depuis l'interface prend effet immédiatement, sans redémarrage.
- `twitch_start_login` valide désormais explicitement qu'un Client ID est
  configuré *avant* d'appeler l'API Twitch, avec un message d'erreur clair
  plutôt qu'un échec HTTP Twitch peu compréhensible.

- **Frontend** (`Connections.tsx`) : section "Configuration Twitch" avec
  champ + bouton *Enregistrer* (retour de validation inline), bouton
  *Créer une application Twitch* (ouvre le portail développeur), et le
  bouton *Se connecter avec Twitch* est désactivé tant qu'aucun Client ID
  n'est configuré (avec message explicatif).

### Fichiers modifiés

`config/schema.rs`, `twitch/auth.rs`, `commands/auth.rs`,
`commands/config.rs`, `state.rs`, `lib.rs`, `types/config.ts`,
`lib/tauri.ts`, `store/configStore.ts`, `pages/Connections.tsx`.

---

## 3. Critique — Bouton "Ouvrir Twitch" inactif

### Cause identifiée

`capabilities/default.json` ne déclarait **pas** la permission
`opener:default`. Tauri 2 bloque silencieusement (sans crash, sans log
évident côté frontend) tout appel à une commande de plugin dont la
permission n'est pas explicitement accordée à la fenêtre appelante — et
comme l'appel à `openUrl(...)` n'était ni attendu ni entouré d'un `catch`
(même pattern que le bug du bouton "Tester" audité précédemment), l'échec
était total et invisible. C'est la combinaison exacte des deux qui
produisait "je clique et il ne se passe rien".

### Correctif appliqué

```diff
  "permissions": [
    ...
    "dialog:allow-open",
    "dialog:allow-save",
+   "opener:default"
  ]
```

Plus une gestion d'erreur explicite (`openExternalUrl()` dans
`Connections.tsx`, `try/await/catch` avec affichage du message en cas
d'échec), appliquée aux **deux** boutons qui ouvrent une URL externe
(*Ouvrir Twitch* et le nouveau *Créer une application Twitch*) — pour que
si un problème similaire (permission, absence de navigateur par défaut
détectable, etc.) survient encore sur une plateforme particulière, il soit
immédiatement visible au lieu de rester silencieux.

### Fichiers modifiés

`capabilities/default.json`, `pages/Connections.tsx`.

---

## 4. Re-vérification du workflow Connexions

Workflow retracé intégralement sur le code actuel : connexion Twitch →
ajout d'une chaîne → connexion → déconnexion → reconnexion.

Le correctif du tour précédent (source de vérité unique pour le statut +
`disconnect()` qui attend réellement la fin de la tâche avant d'annoncer
l'état final, voir `AUDIT_STABILISATION.md` section 2) est toujours en
place et cohérent avec les changements de cette session. Complété par :

- **`twitch_list_connections` fusionne désormais la configuration
  persistée et l'état vivant** (`commands/twitch.rs`) : une chaîne
  précédemment ajoutée mais déconnectée/désactivée reste visible dans la
  liste après un redémarrage de l'application (avant ce correctif, elle
  disparaissait purement et simplement puisque l'état de connexion vit en
  mémoire, réinitialisé à chaque lancement — c'était noté comme
  recommandation non appliquée dans l'audit précédent, section 9).
- Bouton **Reconnecter** déjà ajouté au tour précédent pour toute chaîne
  affichée comme déconnectée, complète le cycle sans avoir à retaper le
  nom de la chaîne.

**Statut : aucun problème résiduel identifié par relecture statique.**
Comme toujours, à confirmer par un test manuel réel après compilation —
c'est la seule vérification qu'une relecture de code ne peut pas remplacer.

---

## 5. Nettoyage trouvé en audit général (architecture)

`tauri-plugin-shell`, `tauri-plugin-fs` et `tauri-plugin-store` étaient
déclarés comme dépendances (`Cargo.toml` **et** `package.json` côté
frontend pour `fs`/`store`) et enregistrés dans le builder Tauri, sans
être appelés nulle part :

- La persistance de configuration utilise un `ConfigStore` maison
  (`std::fs` + `serde_json` directement) — jamais l'API `tauri-plugin-store`.
- L'ouverture d'URL utilise `tauri-plugin-opener`, pas `tauri-plugin-shell`.
- Aucun composant frontend n'importe `@tauri-apps/plugin-fs` ni
  `@tauri-apps/plugin-store`.

Au-delà du poids mort, garder des plugins non utilisés **enregistrés**
élargit inutilement la surface de permissions/capacités de l'application
sans aucun bénéfice — retirés des deux côtés (`Cargo.toml`, `lib.rs`,
`package.json`, `capabilities/default.json`).

---

## 6. Points de vigilance pour votre prochaine compilation

Sans environnement Rust disponible ici, ces points restent à confirmer
par `cargo check` (déjà signalés dans l'audit précédent, toujours valables) :

- `tauri::async_runtime::spawn_blocking` (utilisé dans `lib.rs` et
  `tts/queue.rs`) — l'existence de `tauri::async_runtime::spawn` et
  `JoinHandle` étant maintenant confirmée par votre propre compilation,
  la probabilité que `spawn_blocking` existe aussi sous ce chemin est
  élevée (API cohérente), mais non vérifiée directement.
- `tray_by_id("main")` / `on_tray_icon_event` — dépend de la présence de
  `"id": "main"` dans `tauri.conf.json` (déjà en place) et de la version
  exacte de `tauri` figée dans `Cargo.lock`.

Si l'un de ces points échoue à la compilation, envoyez-moi le message
d'erreur exact — comme pour les 4 correctifs de cette session, la cause
est presque toujours mécanique et rapide à corriger une fois le message
en main.
