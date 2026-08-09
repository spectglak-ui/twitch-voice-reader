-- Schéma SQLite de Twitch Voice Reader.
-- Exécuté au démarrage via `db::repository::Repository::migrate`.
-- Toute évolution du schéma doit être ajoutée comme une nouvelle migration
-- numérotée dans `MIGRATIONS` (repository.rs), jamais par modification
-- rétroactive de ce fichier, pour ne pas casser les bases existantes.

CREATE TABLE IF NOT EXISTS schema_migrations (
    version     INTEGER PRIMARY KEY,
    applied_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Historique des messages reçus (lus ou non), utilisé par l'onglet
-- "Historique" et purgé selon `general.history_retention_days`.
CREATE TABLE IF NOT EXISTS message_history (
    id              TEXT PRIMARY KEY,
    channel         TEXT NOT NULL,
    username_login  TEXT NOT NULL,
    display_name    TEXT NOT NULL,
    role            TEXT NOT NULL,
    text            TEXT NOT NULL,
    was_read_aloud  INTEGER NOT NULL,
    rejection_reason TEXT,
    created_at_ms   INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_message_history_created_at ON message_history (created_at_ms);
CREATE INDEX IF NOT EXISTS idx_message_history_channel ON message_history (channel);

-- Agrégats journaliers pour l'onglet "Statistiques" (évite de recompter
-- l'intégralité de l'historique à chaque ouverture du tableau de bord).
CREATE TABLE IF NOT EXISTS stats_daily (
    day                 TEXT PRIMARY KEY, -- format YYYY-MM-DD (UTC)
    messages_read       INTEGER NOT NULL DEFAULT 0,
    messages_ignored    INTEGER NOT NULL DEFAULT 0,
    reading_time_ms      INTEGER NOT NULL DEFAULT 0
);

-- Utilisateurs actifs vus récemment (pour le compteur "utilisateurs actifs"
-- et pour pré-remplir les suggestions d'attribution de voix par utilisateur).
CREATE TABLE IF NOT EXISTS known_users (
    login           TEXT PRIMARY KEY,
    display_name    TEXT NOT NULL,
    last_seen_ms    INTEGER NOT NULL,
    message_count   INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_known_users_last_seen ON known_users (last_seen_ms);
