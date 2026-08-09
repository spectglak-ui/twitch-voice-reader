# Audit de stabilisation — Addendum 3 : Piper introuvable

Fait suite à `AUDIT_STABILISATION.md` et `AUDIT_STABILISATION_ADDENDUM2.md`.

## 1. Cause exacte

`src-tauri/resources/piper/` ne contenait que le `.gitkeep` et un dossier
`voices/` vide — c'est-à-dire exactement l'état du squelette livré, avant
toute exécution du script d'installation. Deux causes possibles, toutes
deux réelles et corrigées (impossible de trancher laquelle s'est produite
sans le message d'erreur exact de votre terminal, donc les deux sont
traitées) :

1. **Le script n'a jamais pu s'exécuter.** PowerShell bloque par défaut
   l'exécution de scripts `.ps1` non signés sur beaucoup de configurations
   Windows (`ExecutionPolicy Restricted`) — dans ce cas, `install-piper.ps1`
   échoue immédiatement, avant même d'essayer de télécharger quoi que ce
   soit, ce qui correspond exactement à un dossier `resources/piper/`
   resté vierge.
2. **Bug réel dans `install-piper.sh` (Linux/macOS)** trouvé en auditant
   le script en profondeur : `tar -xzf ... --strip-components=1` supprime
   le premier segment de chaque chemin de l'archive. Or les archives Piper
   sont **plates** (le binaire `piper` est directement à la racine de
   l'archive, pas dans un sous-dossier) — sur un fichier à la racine sans
   sous-dossier, `--strip-components=1` a pour effet de le **faire
   disparaître silencieusement** de l'extraction, sans erreur. Ce même
   script contenait aussi un nom d'archive macOS inexistant
   (`piper_macos_universal2.tar.gz` — Piper ne publie que des archives
   séparées par architecture, `piper_macos_x64.tar.gz` et
   `piper_macos_aarch64.tar.gz`).

**Plutôt que de corriger uniquement le symptôme Windows rapporté**, j'ai
traité la cause racine commune aux deux scripts (une étape manuelle
externe, avec plusieurs points de défaillance indépendants) en déplaçant
la responsabilité **dans l'application elle-même** — c'est le cœur de ce
correctif, détaillé section 2. Les scripts existants ont aussi été
corrigés (section 5) car ils restent utiles pour le packaging.

## 2. Nouveau système : installation automatique intégrée

### Nouveau module `src-tauri/src/tts/installer.rs`

- `ensure_piper_binary(install_dir, on_progress)` : télécharge l'archive
  correspondant à la plateforme/architecture **réelle** de la machine
  (`std::env::consts::OS`/`ARCH`, résolu à l'exécution — pas au build),
  vérifie que la taille du fichier téléchargé est plausible (rejette
  immédiatement une réponse anormalement petite, typique d'une page
  d'erreur HTML plutôt que du binaire réel), extrait l'archive (`zip` pour
  Windows, `tar`+`flate2` pour Linux/macOS — deux nouvelles dépendances
  Rust), recherche le binaire résultant de façon défensive (y compris si
  la structure de l'archive changeait), lui donne les droits d'exécution
  sous Unix, puis **l'exécute réellement** (`piper --version`) comme
  ultime vérification avant de le considérer prêt.
- `ensure_default_voice(voices_dir, on_progress)` : même logique pour la
  voix française par défaut (Hugging Face), avec contrôle de taille.
- Émet une série d'évènements de progression (`InstallProgress`) plutôt
  qu'un simple succès/échec binaire, pour que l'interface puisse afficher
  une vraie progression au lieu d'une attente silencieuse.

### `PiperEngine` (`tts/piper.rs`) — refonte

- Le chemin du binaire n'est plus figé une fois pour toutes à la
  construction : `binary_path: RwLock<Option<PathBuf>>`, mis à jour
  dynamiquement après une installation automatique réussie.
- `ensure_ready()` : nouvelle méthode publique qui vérifie et installe si
  besoin (binaire *et* voix par défaut), protégée par un verrou
  (`install_lock: tokio::sync::Mutex<()>`) — **détail de concurrence
  important** : sans ce verrou, un appel automatique au démarrage et un
  appel déclenché par l'ouverture de l'onglet Voix et TTS pourraient tous
  deux constater "rien d'installé" et lancer chacun un téléchargement vers
  le même dossier simultanément.
- `synthesize()` déclenche lui-même `ensure_ready()` en filet de sécurité
  si jamais appelé avant toute tentative d'installation.
- Bug de séquencement trouvé et corrigé **pendant l'implémentation** :
  l'évènement `Done` était émis dès la fin du téléchargement du binaire,
  avant même que la voix par défaut ne soit vérifiée/téléchargée — ce qui
  aurait fait passer l'interface à "prêt" prématurément. Déplacé en toute
  fin de `ensure_ready()`, une fois binaire *et* voix confirmés.

### `resolve_piper_binary()` (`lib.rs`) — durci

Retourne désormais `Option<PathBuf>` (au lieu d'un `PathBuf` toujours
"trouvé", y compris quand ce n'était qu'un nom de commande espéré sur le
`PATH` système sans jamais être vérifié). Priorité : chemin explicite en
config **s'il existe réellement sur disque** → ressources bundlées **si
présentes** → sinon `None`, qui déclenche l'installation automatique.
L'ancien repli silencieux sur le simple nom `piper`/`piper.exe` résolu via
le `PATH` a été retiré : il ne pouvait pas être vérifié à bas coût et
désactivait de fait l'installation automatique pour quiconque n'avait pas
Piper déjà sur son `PATH` — exactement la situation initiale, sans
détection ni action corrective.

### Nouvelle commande Tauri `tts_ensure_installed`

Déclenche l'installation à la demande (bouton "Réessayer"), avec
diffusion de la progression via l'évènement `piper://install-progress`.

### Frontend (`Voice.tsx`)

Remplace l'ancienne bannière d'avertissement statique par un vrai
indicateur d'état (`checking` / `installing` / `ready` / `error`) piloté
par les évènements de progression, avec libellés explicites
("Téléchargement de Piper (42 %)…", "Extraction de l'archive…",
"Vérification du binaire téléchargé…") et un bouton "Réessayer" en cas
d'échec réel — jamais un message générique renvoyant vers un script
externe.

## 3. "Vérification de l'intégrité des fichiers" — ce qui est fait et sa limite honnête

Aucun `SHA256SUMS` officiel n'est publié par le projet Piper pour cette
version (vérifié directement sur la page de release). Impossible donc de
comparer contre une empreinte de référence garantie sans en fabriquer une
moi-même, ce qui serait trompeur plutôt que rassurant. La vérification
d'intégrité mise en place est donc pragmatique et honnête sur ses
limites :

- Téléchargement exclusivement en HTTPS (intégrité du **transport**
  garantie par TLS).
- Contrôle de plausibilité de taille (détecte une page d'erreur HTML
  déguisée en téléchargement réussi).
- Le binaire extrait est **réellement exécuté** (`--version`) avant d'être
  considéré valide — détecte une corruption, une mauvaise architecture, ou
  un exécutable incomplet, ce qu'un simple hash ne garantirait pas mieux.

## 4. Fichiers modifiés/créés

**Nouveaux** : `tts/installer.rs`.
**Modifiés (Rust)** : `tts/piper.rs`, `tts/mod.rs`, `lib.rs`,
`commands/tts.rs`, `Cargo.toml` (+`zip`, `tar`, `flate2`).
**Modifiés (frontend)** : `pages/Voice.tsx`, `lib/tauri.ts`, `types/tts.ts`.
**Scripts** : `install-piper.sh` (suppression du `--strip-components=1`
erroné, correction du nom d'archive macOS, contrôle de taille, recherche
défensive, test d'exécution), `install-piper.ps1` (TLS 1.2 forcé, mêmes
contrôles), `build-macos.sh` + `.github/workflows/build.yml` (Piper n'est
plus pré-embarqué dans le build macOS **universel** — voir section 5).
**Documentation** : `GUIDE_COMPILATION.md`, `README.md`.

## 5. Point additionnel trouvé en auditant : macOS universel

Le build macOS cible `universal-apple-darwin` (un seul binaire Intel +
Apple Silicon), mais Piper ne publie **pas** de binaire universel — deux
archives séparées par architecture. Pré-embarquer l'une des deux dans le
bundle universel aurait cassé la moitié des Mac (l'inverse de
l'architecture embarquée). Corrigé en ne pré-embarquant plus Piper pour ce
build : l'installation automatique détecte correctement l'architecture
**réelle** de la machine à l'exécution, ce qui est la seule approche
cohérente avec un binaire universel. Documenté dans `build-macos.sh` et
le workflow CI.

## 6. Risque structurel à surveiller

Le dépôt `rhasspy/piper` est **archivé** (lecture seule depuis octobre
2025) ; toute évolution future se fait sous `OHF-Voice/piper1-gpl`, qui ne
publie plus d'exécutable Windows autonome (distribution via
`pip install piper-tts`). Les URLs utilisées ici pointent vers la
**dernière version figée** de l'ancien dépôt — elles resteront
fonctionnelles tant que GitHub conserve ces releases (l'archivage ne les
supprime pas), mais ne recevront plus jamais de mise à jour ni de
correctif. À surveiller sur le long terme ; une migration vers
`piper1-gpl` impliquerait d'embarquer un interpréteur Python, un
changement d'architecture plus large documenté comme piste de Phase 4
dans le cahier technique plutôt que traité ici.
