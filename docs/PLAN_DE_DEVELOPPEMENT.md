# Plan de développement — Twitch Voice Reader

## Phase 0 — Fondations (livrée dans cette itération)

- Architecture complète, arborescence, dépendances
- Scaffold Tauri 2 + React/TS/Vite/Tailwind fonctionnel
- Tous les modules backend MVP écrits (twitch, tts, audio, filters, config, db, overlay, stats)
- Interface complète (6 sections) reliée à l'IPC
- Scripts de build 3 plateformes + CI GitHub Actions
- **Statut** : code écrit intégralement, **non compilé** (pas de toolchain
  Rust/réseau dans l'environnement de génération) — première étape de la
  Phase 1 = validation de compilation.

## Phase 1 — Validation et stabilisation (1-2 semaines)

Objectif : passer d'un scaffold à une build qui tourne réellement.

1. `cargo check` / `cargo clippy` — corriger les erreurs de types, lifetimes,
   imports manquants (attendu sur un projet de cette taille écrit sans
   compilateur disponible).
2. `npm run build` côté frontend — corriger les éventuelles erreurs TS strictes.
3. Test manuel du flux complet : login Device Code -> connexion chaîne ->
   réception message -> filtre -> TTS -> lecture audio -> overlay.
4. Vérifier l'API `tray_by_id`/`on_tray_icon_event` contre la version exacte
   de `tauri` figée dans `Cargo.lock` (l'API tray a évolué au fil des betas
   Tauri 2 ; à valider précisément).
5. Télécharger/tester au moins 2 voix Piper par langue supportée.

## Phase 2 — Robustesse fonctionnelle (2-3 semaines)

1. Tests d'intégration sur la reconnexion automatique (coupure réseau simulée).
2. Validation des filtres/anti-spam en conditions réelles de chat à fort débit.
3. Gestion des cas limites : chaîne inexistante, jeton expiré en cours de
   session, périphérique audio déconnecté à chaud.
4. Ajout de tests end-to-end légers (ex : `tauri-driver` + WebDriver) sur
   les parcours critiques (connexion, réglages TTS, export config).
5. Rafraîchissement automatique du token OAuth avant expiration (actuellement
   rafraîchi à la reconnexion uniquement — cf `twitch/auth.rs::refresh`,
   pas encore appelé automatiquement en tâche de fond).

## Phase 3 — Fonctionnalités avancées et qualité audio

1. Process Piper persistant (latence) — voir cahier technique section 9.
2. Vrai pitch-shifting (`rubato` + phase-vocoder), découplé de la vitesse.
3. Migration du contrat IPC vers génération automatique de types (`specta`).
4. Implémentation EventSub comme second backend Twitch derrière
   `ChannelEvent` (abstraction déjà prête dans `irc_client.rs`).
5. Téléchargement de voix supplémentaires directement depuis l'onglet
   "Voix et TTS" (actuellement : seule la voix FR par défaut est bundlée).
6. Personnalisation visuelle de l'overlay depuis l'interface (actuellement :
   overlay_page.html statique, éditable manuellement).

## Phase 4 — Distribution commerciale

1. Signature de code Windows (certificat Authenticode).
2. Signature + notarization macOS (compte Apple Developer).
3. Canal de mise à jour automatique (`tauri-plugin-updater`).
4. Télémétrie d'usage opt-in (crash reporting, métriques d'adoption des
   fonctionnalités) — à spécifier séparément (RGPD/vie privée).
5. Documentation utilisateur finale (site, FAQ, tutoriels vidéo).
6. Mise en place d'un canal de support (Discord, formulaire).

## Notes de suivi

Ce document est destiné à être tenu à jour au fil de l'avancement réel
(cocher/dater les étapes, ajouter les découvertes de la Phase 1 qui
affinent les estimations des phases suivantes).
