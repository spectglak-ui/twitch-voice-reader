# Audit de stabilisation — Twitch Voice Reader

Audit réalisé par relecture exhaustive du code source (backend Rust +
frontend React/TS), avec correctifs déjà appliqués directement dans le
projet livré. Chaque section indique : cause, fichier(s), correctif exact,
et justification. Les niveaux de confiance sont indiqués explicitement là
où je n'ai pas pu vérifier par compilation réelle (voir note en fin de
document).

**Sur vos deux correctifs déjà appliqués** : les deux sont corrects et
nécessaires. `tray-icon` est bien la feature manquante pour l'API tray de
Tauri 2, et `tokio::spawn` → `tauri::async_runtime::spawn` est exactement
le bon réflexe face à `there is no reactor running`. J'ai étendu cette
correction à **tous** les autres points du code qui avaient exactement le
même défaut (section 1).

---

## Sommaire

1. [Critique — `tokio::spawn` restants (audit complet)](#1-critique--tokiospawn-restants)
2. [Critique — Bouton « Déconnecter » Twitch](#2-critique--bouton-déconnecter-twitch)
3. [Critique — Bouton « Tester » TTS sans son](#3-critique--bouton-tester-tts-sans-son)
4. [Important — Fuite de processus Piper](#4-important--fuite-de-processus-piper)
5. [Important — File TTS pouvant se bloquer indéfiniment](#5-important--file-tts-pouvant-se-bloquer-indéfiniment)
6. [Important — Connexion Twitch "silencieusement morte"](#6-important--connexion-twitch-silencieusement-morte)
7. [Important — Appels SQLite bloquants sur les threads async](#7-important--appels-sqlite-bloquants-sur-les-threads-async)
8. [Important — Erreurs avalées en silence (pattern systémique)](#8-important--erreurs-avalées-en-silence-pattern-systémique)
9. [Important — Liste des chaînes non fusionnée avec la config persistée](#9-important--liste-des-chaînes-non-fusionnée-avec-la-config-persistée)
10. [Mineur — Divers](#10-mineur--divers)
11. [Plan de stabilisation avant release publique](#11-plan-de-stabilisation-avant-release-publique)

---

## 1. Critique — `tokio::spawn` restants

### Cause

`tokio::spawn` exige que le thread appelant ait un runtime Tokio "entré"
dans son contexte thread-local **au moment de l'appel**. C'est le cas à
l'intérieur de toute tâche déjà pilotée par le runtime (ex: le corps d'une
commande Tauri `async fn`), mais **pas** à l'intérieur du callback
synchrone `.setup(|app| { ... })`, qui est exécuté directement par le
bootstrap de Tauri, hors de tout contexte de tâche. `tauri::async_runtime::spawn`
contourne ce problème en soumettant la tâche directement au handle de
runtime que Tauri conserve en interne, sans dépendre du contexte ambiant.

### Recherche exhaustive effectuée

```bash
grep -rn "tokio::spawn" src-tauri/src
```

9 occurrences trouvées, réparties ainsi :

| Fichier | Contexte d'appel | Dangereux avant correctif ? |
|---|---|---|
| `lib.rs` (×5) | Directement dans `.setup()` | **Oui — cause probable du crash déjà observé** |
| `tts/queue.rs` (`TtsQueue::spawn`) | Fonction sync appelée **depuis** `.setup()` | **Oui, même défaut, un niveau d'indirection plus loin** |
| `twitch/connection_manager.rs` (`connect()`) | `connect()` est `async`, appelée depuis commandes async ET depuis une tâche spawnée au démarrage | Non, mais fragile (voir ci-dessous) |
| `commands/auth.rs` (`twitch_start_login`) | Commande `async fn` | Non |
| `overlay/server.rs` (`OverlayServer::start`) | Appelée depuis une commande `async fn` | Non |

### Correctif appliqué

**Les 9 occurrences** ont été converties vers `tauri::async_runtime::spawn`
(et `tauri::async_runtime::spawn_blocking` là où c'est pertinent, voir
section 7), y compris les 3 qui n'étaient pas strictement dangereuses dans
leur contexte d'appel actuel. Raison : `connect()` en particulier est
appelée à la fois depuis une commande (contexte sûr) et depuis une tâche
de reconnexion automatique lancée au démarrage (contexte `.setup()`, donc
risqué) — le même code doit être sûr dans les deux cas, et le seul moyen
de le garantir sans dupliquer la logique est d'utiliser partout l'API qui
fonctionne dans les deux contextes. C'est aussi la recommandation
généralement admise dans la communauté Tauri : **ne jamais utiliser
`tokio::spawn` directement dans une application Tauri, toujours
`tauri::async_runtime::spawn`**, précisément pour éviter ce genre de piège
lors de refactors futurs qui déplaceraient un appel d'un contexte sûr vers
un contexte `.setup()`.

```diff
- tokio::spawn(async move {
+ tauri::async_runtime::spawn(async move {
```

Fichiers modifiés : `lib.rs`, `tts/queue.rs`, `overlay/server.rs`,
`commands/auth.rs`, `twitch/connection_manager.rs`.

### Niveau de confiance

Élevé pour `spawn`. **Moyen** pour `tauri::async_runtime::spawn_blocking`
(utilisé section 7) : cette fonction existe dans les versions récentes de
Tauri 2, mais son nom/emplacement exact a pu varier entre versions
mineures de la crate `tauri`. **À vérifier avec `cargo check`** ; en cas
d'erreur de compilation sur ce point précis, repli immédiat possible vers
`tokio::task::spawn_blocking` (fonctionnellement équivalent dans tous les
sites d'appel concernés, puisqu'ils sont tous à l'intérieur de tâches déjà
spawnées via `tauri::async_runtime::spawn` — donc dans un contexte où le
runtime est garanti entré).

---

## 2. Critique — Bouton « Déconnecter » Twitch

### Cause réelle (deux bugs distincts trouvés)

**Bug 2a — le statut affiché était figé dès la connexion.**

```rust
// twitch/connection_manager.rs — AVANT
struct ChannelHandle {
    status: ConnectionStatus,   // écrit une seule fois à l'insertion,
    task: JoinHandle<()>,       // jamais mis à jour ensuite
    stop_tx: mpsc::Sender<()>,
}

pub async fn connected_channels(&self) -> Vec<(String, ConnectionStatus)> {
    self.channels.lock().await.iter()
        .map(|(k, v)| (k.clone(), v.status))   // toujours "Connecting"
        .collect()
}
```

`v.status` valait `Connecting` à l'insertion et n'était **plus jamais
réécrit** — tous les changements d'état réels (Connected, Reconnecting,
Disconnected) n'étaient envoyés qu'au frontend via l'évènement broadcast,
jamais répercutés dans cette structure. Toute requête qui relit l'état
côté backend (`twitch_list_connections`, appelée au montage de la page
Connexions) retournait donc un état obsolète.

**Bug 2b — condition de course à la déconnexion.**

```rust
// AVANT
pub async fn disconnect(&self, channel_login: &str) {
    if let Some(handle) = self.channels.lock().await.remove(&channel_login) {
        handle.stop_tx.send(()).await.ok();
        handle.task.abort();                 // ne bloque pas, n'attend rien
        self.events_tx.send(StatusChanged{Disconnected}).ok(); // envoyé immédiatement après
    }
}
```

`task.abort()` **ne bloque pas** : il marque la tâche pour annulation, qui
ne prend effet qu'au prochain point d'attente (`.await`) *à l'intérieur*
de cette tâche. Entre l'appel à `abort()` et l'arrêt effectif de la tâche,
celle-ci peut, sur un autre thread du pool Tokio, être en train de traiter
un évènement déjà en vol (ex: `ChannelEvent::Connected` reçu juste avant
la déconnexion) et envoyer `StatusChanged { Connected }`. Selon
l'ordonnancement exact, ce message peut arriver au frontend **après**
celui de déconnexion — la chaîne semble alors « se reconnecter toute
seule » juste après le clic, ce qui ressemble exactement à « le bouton ne
fonctionne pas ».

**Effet combiné constaté :** au mieux le statut affiché reflète un état
obsolète après rechargement, au pire un évènement de connexion tardif
écrase visuellement la déconnexion qui vient de se produire.

### Correctif appliqué

Réécriture de `ConnectionManager` (`twitch/connection_manager.rs`) :

1. **Source de vérité unique** : ajout d'une map `statuses: Arc<Mutex<HashMap<String, ConnectionStatus>>>`
   mise à jour par un seul point de passage (`set_status`), qui met à jour
   la map *et* émet l'évènement dans la même fonction — impossible que les
   deux divergent.

   ```rust
   async fn set_status(
       statuses: &Arc<Mutex<HashMap<String, ConnectionStatus>>>,
       events_tx: &mpsc::UnboundedSender<ManagerEvent>,
       channel: &str,
       status: ConnectionStatus,
   ) {
       statuses.lock().await.insert(channel.to_string(), status);
       events_tx.send(ManagerEvent::StatusChanged { channel: channel.to_string(), status }).ok();
   }
   ```

   `connected_channels()` lit désormais cette map directement — plus jamais
   figée.

2. **Suppression de la condition de course** : `disconnect()` **attend
   la terminaison réelle** de la tâche avant d'annoncer l'état final.

   ```rust
   pub async fn disconnect(&self, channel_login: &str) {
       let channel_login = channel_login.to_lowercase();
       if let Some(handle) = self.channels.lock().await.remove(&channel_login) {
           handle.stop_tx.send(()).await.ok();
           handle.task.abort();
           let _ = handle.task.await;  // <- attend la fin effective (Err attendu, tâche annulée)

           set_status(&self.statuses, &self.events_tx, &channel_login, ConnectionStatus::Disconnected).await;
       }
   }
   ```

   Une fois `handle.task.await` résolu, la tâche est **garantie** arrêtée :
   elle ne peut plus émettre le moindre évènement après coup. L'ordre
   `Disconnected` en dernier est donc désormais garanti, pas seulement
   probable.

3. **Bug additionnel corrigé au passage** : si une chaîne était ajoutée
   *avant* toute authentification Twitch, la tâche se terminait
   immédiatement (pas de token) mais restait dans le registre `channels`
   — `connect()` la croyait donc « déjà active » pour toujours, et un essai
   après connexion ne faisait plus rien. La tâche se retire désormais
   elle-même du registre dans ce cas précis.

### Correctif frontend associé

`src/store/connectionStore.ts` : `connect`/`disconnect` n'avaient **aucune
gestion d'erreur** (`await` sans `try/catch`) — un échec de la commande
Tauri produisait un rejet de promesse silencieux, invisible dans
l'interface. Ajout d'un état `lastActionError` affiché dans
`Connections.tsx`. Voir aussi section 8 (pattern systémique).

`src/pages/Connections.tsx` : ajout d'un bouton « Reconnecter » quand le
statut est `disconnected` (auparavant, seule l'icône de suppression était
présente, sans moyen de rétablir la connexion sans retaper le nom de la
chaîne).

---

## 3. Critique — Bouton « Tester » TTS sans son

### Cause réelle (plusieurs facteurs cumulés)

**Cause principale — aucune gestion d'erreur côté frontend :**

```tsx
// src/pages/Voice.tsx — AVANT
const handleTestVoice = () => {
  api.tts.testVoice({ text: testText, voiceId: tts.default_voice_id, ... });
  // ni `await`, ni `.catch(...)` : un rejet de promesse est totalement
  // silencieux (juste une "Unhandled promise rejection" en console DevTools,
  // invisible pour l'utilisateur final)
};
```

Résultat : **quelle que soit la cause réelle de l'échec**, le symptôme
observable est exactement « rien ne se passe ». Ce correctif seul ne
suffit pas à faire fonctionner le TTS, mais il est indispensable pour que
n'importe quel problème (ci-dessous) devienne diagnosticable depuis
l'interface plutôt que depuis la console développeur.

**Causes probables de l'échec réel sous-jacent :**

- Le binaire Piper et les voix `.onnx` ne sont **pas installés par défaut**
  — ils nécessitent l'exécution de `scripts/install-piper.sh`/`.ps1` avant
  le premier lancement (voir `docs/GUIDE_COMPILATION.md`, section 2). Sans
  cette étape, `PiperEngine::synthesize()` retourne systématiquement
  `AppError::TtsUnavailable("Modèle de voix introuvable...")`.
- En mode `tauri dev`, `resource_dir()` ne contient les voix que si
  `src-tauri/resources/piper/` a été peuplé **avant** le lancement — un
  oubli fréquent puisque rien n'empêchait l'application de démarrer sans.

### Correctifs appliqués

**Backend (`tts/piper.rs`)** :
- Détection explicite d'une sortie Piper vide (`samples.is_empty()`) : si
  Piper se termine avec succès mais ne produit aucun échantillon (modèle
  incompatible avec la version du binaire, texte vide), une erreur
  explicite est désormais renvoyée au lieu d'un `Ok` silencieux.
- Message d'erreur enrichi pointant vers le script d'installation.
- Voir aussi section 4 (fuite de process) et section 5 (timeout), qui
  couvrent des angles morts adjacents de la même chaîne d'exécution.

**Frontend (`src/pages/Voice.tsx`)** :
- `handleTestVoice` est désormais `async`, avec `try/catch` et affichage
  du message d'erreur exact sous le bouton.
- Vérification proactive au chargement de la page (`tts_check_installation`
  + `tts_list_installed_voices`) avec un bandeau d'avertissement visible
  si Piper est introuvable ou si aucune voix n'est installée — **avant**
  même que l'utilisateur ait cliqué sur Tester.

```tsx
const handleTestVoice = async () => {
  setTestState("loading");
  setTestError(null);
  try {
    await api.tts.testVoice({ text: testText, voiceId: tts.default_voice_id, ... });
    setTestState("idle");
  } catch (err) {
    setTestState("error");
    setTestError(formatInvokeError(err));
  }
};
```

**Frontend (`src/components/layout/Topbar.tsx` + `src/store/chatStore.ts`)** :
Les erreurs de lecture survenant pendant le fonctionnement normal (pas
seulement le bouton de test) étaient elles aussi invisibles :
`TtsPlaybackEvent::Error` était reçu par le frontend mais son message
n'était **jamais stocké ni affiché**, seulement utilisé pour réinitialiser
l'indicateur "en cours de lecture". Un streamer dont le chat cesse d'être
lu (voix supprimée en cours de session, Piper qui plante) n'avait donc
aucune indication du problème. Ajout d'un bandeau d'erreur persistant dans
le layout global.

### Action requise de votre côté

Ces correctifs rendent le problème **diagnosticable**, mais si Piper n'est
toujours pas installé sur votre machine de test, le message d'erreur
maintenant visible vous le confirmera explicitement. Vérifiez :

```bash
ls src-tauri/resources/piper/          # doit contenir piper(.exe)
ls src-tauri/resources/piper/voices/   # doit contenir au moins un .onnx
```

Si absent, relancez `scripts/install-piper.sh` (ou `.ps1`) puis
`npm run tauri dev`.

---

## 4. Important — Fuite de processus Piper

### Cause

`tokio::process::Child` **ne tue pas** son processus enfant au `Drop` — un
piège Rust bien connu mais facile à manquer. Avant correctif :

```rust
// AVANT
let mut child = Command::new(...).spawn()?;
stdin.write_all(...).await?;   // si erreur ici, `?` sort de la fonction,
                                 // `child` est droppé SANS être tué
let status = child.wait().await?;
```

Tout chemin d'erreur avant `child.wait()` — et, plus grave, **toute
annulation externe de la future** (exactement ce que fait le timeout
ajouté en section 5) — laisse le processus Piper devenir orphelin,
continuant de tourner indéfiniment en arrière-plan. Sur une session
longue avec des échecs répétés (ex: modèle de voix corrompu), ceci
accumule des processus zombies.

### Correctif appliqué

Ajout d'un garde RAII `KillOnDropChild` dans `tts/piper.rs` :

```rust
struct KillOnDropChild {
    child: Child,
    waited: bool,
}

impl Drop for KillOnDropChild {
    fn drop(&mut self) {
        if !self.waited {
            let _ = self.child.start_kill(); // sync, non bloquant — utilisable dans Drop
        }
    }
}
```

`waited` n'est mis à `true` qu'après un `child.wait()` réussi (le process
est alors déjà terminé, rien à tuer). Dans **tous les autres cas** —
retour d'erreur anticipé via `?`, ou future abandonnée par un timeout
externe — le `Drop` du garde envoie le signal de terminaison. C'est la
seule approche qui couvre à la fois les sorties d'erreur explicites *et*
l'annulation implicite (`Drop` s'exécute toujours, quelle que soit la
raison de la sortie de scope).

---

## 5. Important — File TTS pouvant se bloquer indéfiniment

### Cause

La file TTS traite les messages strictement séquentiellement (nécessaire
pour l'intelligibilité). Avant correctif, ni `piper.synthesize()` ni
`audio_player.play_pcm()` n'avaient de limite de temps :

```rust
// AVANT — aucun timeout
match piper.synthesize(&text_to_speak, &voice_id, ...).await { ... }
```

Si Piper reste bloqué (process zombie, bug interne, modèle corrompu qui le
fait boucler plutôt qu'échouer proprement), **toute la file** — donc la
lecture de l'intégralité du chat, toutes chaînes confondues — se fige
silencieusement pour le reste de la session, sans le moindre message
d'erreur.

### Correctif appliqué (`tts/queue.rs`)

```rust
const SYNTHESIS_TIMEOUT: Duration = Duration::from_secs(15);
const PLAYBACK_TIMEOUT: Duration = Duration::from_secs(60);

let synthesis_result = tokio::time::timeout(
    SYNTHESIS_TIMEOUT,
    piper.synthesize(&text_to_speak, &voice_id, ...),
).await;

match synthesis_result {
    Err(_elapsed) => { /* émet TtsPlaybackEvent::Error, passe au message suivant */ }
    Ok(Err(e))    => { /* erreur normale de synthèse */ }
    Ok(Ok(audio)) => { /* lecture, elle aussi enveloppée d'un timeout de 60s */ }
}
```

Un message problématique échoue désormais proprement après 15s (synthèse)
ou 60s (lecture, volontairement plus généreux pour ne pas couper un long
message légitime) au lieu de geler la file indéfiniment. Combiné au
correctif de la section 4, le processus Piper associé est également tué
proprement dans ce cas.

---

## 6. Important — Connexion Twitch "silencieusement morte"

### Cause

`read.next().await` (lecture du flux WebSocket IRC) n'avait, avant
correctif, aucune limite de temps. Une coupure réseau qui ne ferme pas
proprement le socket (bascule Wi-Fi, veille système, certains routeurs/NAT
qui coupent silencieusement une connexion inactive) laisse cet appel
suspendu indéfiniment : la tâche ne détecte jamais la coupure, la logique
de reconnexion à backoff exponentiel ne se déclenche donc **jamais**, et
l'interface continue d'afficher « En direct » alors que plus aucun message
n'arrivera.

### Correctif appliqué (`twitch/connection_manager.rs`)

```rust
const IDLE_TIMEOUT: Duration = Duration::from_secs(6 * 60); // Twitch PING ~5 min

tokio::select! {
    _ = stop_rx.recv() => break,
    result = tokio::time::timeout(IDLE_TIMEOUT, &mut run_future) => {
        if result.is_err() {
            send_notice(&events_tx, &channel_for_task, "Connexion inactive depuis plus de 6 minutes, reconnexion forcée");
        }
    }
    _ = &mut bridge => {}
}
```

Twitch envoie un `PING` serveur toutes les ~5 minutes sur une connexion
active (déjà géré par le `PONG` existant dans `irc_client.rs`) ; 6 minutes
sans le moindre octet reçu est donc un signal fiable de connexion morte.
Au-delà de ce délai, la boucle de reconnexion à backoff se déclenche
normalement, comme pour toute autre déconnexion.

---

## 7. Important — Appels SQLite bloquants sur les threads async

### Cause

`rusqlite` est une API **synchrone**. Deux points l'appelaient directement
depuis du code `async` partagé avec **toutes** les connexions Twitch
actives et la lecture audio :

- `lib.rs::process_incoming_message` — à **chaque message de chat reçu**,
  toutes chaînes confondues.
- `tts/queue.rs` — à la fin de chaque lecture (mise à jour du temps de
  lecture cumulé).

Une écriture disque synchrone bloque le thread du pool Tokio qui
l'exécute ; avec un pool multi-thread cela reste généralement discret,
mais sous fort débit de chat (raids, chaînes très actives) ou sur un
disque lent/réseau, cela peut retarder mesurablement le traitement des
messages d'autres chaînes partageant le même pool.

### Correctif appliqué

Déport systématique vers `tauri::async_runtime::spawn_blocking` :

```rust
// lib.rs::process_incoming_message
let db = state.db.clone();
tauri::async_runtime::spawn_blocking(move || {
    db.insert_message(&history_entry).ok();
});
```

Même traitement pour `add_reading_time` (`tts/queue.rs`) et
`purge_older_than` (tâche périodique de purge, `lib.rs`).

**Non modifiés, à raison** : `commands/stats.rs::get_stats_summary` et
`get_history` sont déclarées comme commandes **synchrones**
(`pub fn`, pas `pub async fn`) — Tauri les exécute déjà sur un pool de
threads dédié aux tâches bloquantes, distinct du runtime async partagé.
Aucun changement nécessaire pour ces deux-là.

---

## 8. Important — Erreurs avalées en silence (pattern systémique)

Au-delà des deux bugs rapportés, ce pattern (`invoke(...)` non attendu, ou
`await` sans `catch`) a été trouvé à plusieurs endroits. Corrigés dans
cette passe : `Voice.tsx` (test vocal), `connectionStore.ts`
(connexion/déconnexion). **Non corrigés, à traiter en Phase 1 de
stabilisation** (même pattern, moindre urgence puisque non rapportés comme
bugs bloquants) :

- `Filters.tsx`, `Settings.tsx` : les setters de configuration
  (`setFilters`, `setAntiSpam`, `setOverlay`, `setGeneral`) stockent déjà
  l'erreur dans `configStore.error`, mais **rien dans l'interface ne
  l'affiche actuellement**. Le mécanisme existe, il manque juste le
  branchement visuel (même pattern que le correctif appliqué à
  `Topbar.tsx`/`Connections.tsx` — à répliquer).

**Recommandation générale** : tout appel à `invoke(...)` (direct ou via
`src/lib/tauri.ts`) doit systématiquement être `await`é dans un
`try/catch`, jamais laissé en `Promise` flottante — un rejet non géré en
JavaScript est silencieux par défaut, contrairement à une exception non
rattrapée dans la plupart des autres langages.

---

## 9. Important — Liste des chaînes non fusionnée avec la config persistée

### Cause

`ConnectionManager.statuses` (source de vérité de l'onglet Connexions,
voir section 2) est un état **en mémoire**, réinitialisé à chaque
redémarrage de l'application. `config.channels` (persisté dans
`config.json`) reste la seule trace durable des chaînes ajoutées. Au
démarrage, seules les chaînes marquées `enabled: true` sont reconnectées
automatiquement — une chaîne précédemment **désactivée/déconnectée**
disparaît donc purement et simplement de la liste affichée après un
redémarrage, jusqu'à ce que l'utilisateur retape son nom.

### Recommandation (non appliquée dans cette passe — nécessite une nouvelle
commande, décrite ici pour la Phase 1)

Faire de `twitch_list_connections` une fusion de `config.channels`
(persisté) et de `connection_manager.connected_channels()` (état vivant),
plutôt que de n'exposer que ce dernier :

```rust
#[tauri::command]
pub async fn twitch_list_connections(state: State<'_, AppState>) -> Result<Vec<(String, ConnectionStatus)>, AppError> {
    let live = state.connection_manager.connected_channels().await;
    let mut merged: HashMap<String, ConnectionStatus> = state.config.get().channels.iter()
        .map(|c| (c.login.clone(), ConnectionStatus::Disconnected))
        .collect();
    merged.extend(live);
    Ok(merged.into_iter().collect())
}
```

---

## 10. Mineur — Divers

| # | Problème | Fichier | Statut |
|---|---|---|---|
| 10.1 | `app_data_dir().expect(...)` panique brutalement au lieu de remonter une erreur de démarrage propre | `lib.rs` | **Corrigé** (converti en `?` via `map_err`) |
| 10.2 | Texte envoyé à Piper non nettoyé des sauts de ligne (Piper traite stdin ligne par ligne — un `\n` dans un message le scinderait en deux énoncés) | `tts/piper.rs` | **Corrigé** |
| 10.3 | Pas de fermeture propre des connexions WebSocket Twitch à la fermeture de l'application (juste terminaison de process) | `lib.rs` | Non corrigé — impact réel négligeable (l'OS ferme les sockets), cosmétique côté Twitch |
| 10.4 | Ajout de nombreuses chaînes très rapprochées pourrait heurter le rate-limit Twitch sur les `JOIN` (~20/10s pour un compte non vérifié) | `twitch/irc_client.rs` | Non corrigé — peu probable en usage normal (ajout manuel un par un), à surveiller si une fonctionnalité d'import en masse est ajoutée |
| 10.5 | `disconnect_all()` déconnecte maintenant séquentiellement (chaque `disconnect()` attend la fin de la tâche précédente avant de traiter la suivante) — légèrement plus lent qu'avant, imperceptible sauf avec un très grand nombre de chaînes | `twitch/connection_manager.rs` | Comportement accepté (correction nécessaire pour la section 2, le léger surcoût est un compromis raisonnable) |

---

## 11. Plan de stabilisation avant release publique

### Déjà fait (cette passe)
- [x] Audit et correction de tous les `tokio::spawn` (9 sites)
- [x] Correction du bug de déconnexion (statut figé + condition de course)
- [x] Correction du bug de test vocal (gestion d'erreur bout en bout)
- [x] Fuite de processus Piper (garde kill-on-drop)
- [x] Timeouts sur la synthèse, la lecture audio, et la lecture réseau IRC
- [x] Déport des appels SQLite bloquants hors du runtime async partagé
- [x] Détection d'une sortie Piper vide
- [x] Erreurs TTS et connexion désormais visibles dans l'interface

### À faire avant release publique (par ordre de priorité)

1. **`cargo check` puis `cargo clippy --all-targets`** — valider
   mécaniquement l'ensemble des correctifs ci-dessus, en particulier
   `tauri::async_runtime::spawn_blocking` (voir note de confiance, section 1)
   et le type de retour de `tauri::async_runtime::spawn` dans
   `connection_manager.rs` (`ChannelHandle.task: tokio::task::JoinHandle<()>`
   — à confirmer que c'est bien le type retourné).
2. Répliquer l'affichage des erreurs de configuration dans `Filters.tsx`
   et `Settings.tsx` (section 8).
3. Fusionner chaînes persistées et état vivant dans `twitch_list_connections`
   (section 9).
4. Test de charge manuel : rejoindre une chaîne à fort trafic (raid,
   >10 messages/s) et observer la latence de lecture et la stabilité de
   la file TTS sur une session de plusieurs heures.
5. Test de résilience réseau : couper la connexion internet en cours de
   session (mode avion, désactivation Wi-Fi) et vérifier que la
   reconnexion se déclenche dans les ~6 minutes (section 6) puis
   fonctionne normalement au retour du réseau.
6. Test du rafraîchissement automatique du token OAuth avant expiration
   (actuellement `TwitchAuthClient::refresh` existe mais n'est appelé
   qu'implicitement — vérifier le comportement réel après 4h de session
   continue, durée de vie typique d'un token Twitch).
7. Vérifier le comportement de `KillOnDropChild`/timeouts sous Windows
   spécifiquement (`start_kill()` et la gestion de process ont des
   comportements légèrement différents entre Unix et Windows — à tester,
   pas seulement relu).

### Note méthodologique

Cet audit a été réalisé par relecture statique exhaustive (recherche
systématique par catégorie, traçage manuel de chaque chemine d'exécution
critique) sans environnement de compilation Rust disponible. Les
correctifs sont soigneusement raisonnés mais **doivent être validés par
`cargo check`** avant d'être considérés définitifs — en particulier tout
point signalé avec un niveau de confiance "moyen" dans ce document. Un
audit de cette ampleur gagnerait, en Phase 1, à être complété par une
relecture croisée après une première compilation réussie.
