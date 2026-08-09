import { useEffect, useState } from "react";
import { Plus, Trash2, ExternalLink, AlertCircle, Save, ArrowUpRight } from "lucide-react";
import { useAuthStore } from "@/store/authStore";
import { useConnectionStore } from "@/store/connectionStore";
import { useConfigStore } from "@/store/configStore";
import { StatusBadge } from "@/components/ui/StatusBadge";
import { openUrl } from "@tauri-apps/plugin-opener";

const TWITCH_DEV_CONSOLE_URL = "https://dev.twitch.tv/console/apps";

/** Ouvre une URL dans le navigateur par défaut, avec un retour d'erreur
 * visible en cas d'échec (permission manquante, plateforme sans
 * navigateur par défaut détectable, etc.) — auparavant l'appel n'était ni
 * attendu ni entouré d'un `catch`, donc un échec ne produisait
 * strictement aucun effet visible ("le bouton ne fait rien"). */
async function openExternalUrl(url: string, onError: (message: string) => void) {
  try {
    await openUrl(url);
  } catch (err) {
    const message = err && typeof err === "object" && "message" in err ? String((err as { message: unknown }).message) : String(err);
    onError(`Impossible d'ouvrir le navigateur : ${message}`);
  }
}

export function Connections() {
  const { currentAccount, pendingDeviceCode, isPolling, error, init, startLogin, logout } = useAuthStore();
  const {
    statuses,
    notices,
    lastActionError,
    init: initConnections,
    connect,
    disconnect,
    dismissActionError,
  } = useConnectionStore();
  const { config, load: loadConfig, setTwitchClientId } = useConfigStore();
  const [newChannel, setNewChannel] = useState("");
  const [clientIdDraft, setClientIdDraft] = useState("");
  const [clientIdError, setClientIdError] = useState<string | null>(null);
  const [clientIdSaved, setClientIdSaved] = useState(false);
  const [openUrlError, setOpenUrlError] = useState<string | null>(null);

  useEffect(() => {
    init();
    initConnections();
    loadConfig();
  }, [init, initConnections, loadConfig]);

  // Pré-remplit le champ dès que la configuration est chargée, sans
  // écraser une saisie en cours si l'utilisateur est déjà en train de
  // modifier la valeur.
  useEffect(() => {
    if (config && clientIdDraft === "") {
      setClientIdDraft(config.twitch.client_id ?? "");
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [config]);

  const isClientIdConfigured = Boolean(config?.twitch.client_id?.trim());

  const handleAddChannel = async () => {
    const login = newChannel.trim().toLowerCase();
    if (!login) return;
    await connect(login);
    setNewChannel("");
  };

  const handleSaveClientId = async () => {
    setClientIdError(null);
    setClientIdSaved(false);
    try {
      await setTwitchClientId(clientIdDraft);
      setClientIdSaved(true);
      setTimeout(() => setClientIdSaved(false), 2500);
    } catch (err) {
      const message = err && typeof err === "object" && "message" in err ? String((err as { message: unknown }).message) : String(err);
      setClientIdError(message);
    }
  };

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-semibold text-ink">Connexions Twitch</h1>
        <p className="text-sm text-ink-faint">Authentification et gestion des chaînes suivies</p>
      </div>

      {lastActionError && (
        <div className="panel flex items-start justify-between gap-3 border-alert/40 bg-alert/10 p-4">
          <div className="flex items-start gap-2 text-sm text-alert">
            <AlertCircle size={16} className="mt-0.5 shrink-0" />
            <span>{lastActionError}</span>
          </div>
          <button onClick={dismissActionError} className="text-alert/70 hover:text-alert">
            ✕
          </button>
        </div>
      )}

      {openUrlError && (
        <div className="panel flex items-start justify-between gap-3 border-alert/40 bg-alert/10 p-4">
          <div className="flex items-start gap-2 text-sm text-alert">
            <AlertCircle size={16} className="mt-0.5 shrink-0" />
            <span>{openUrlError}</span>
          </div>
          <button onClick={() => setOpenUrlError(null)} className="text-alert/70 hover:text-alert">
            ✕
          </button>
        </div>
      )}

      <div className="panel p-5">
        <h2 className="mb-1 text-sm font-medium text-ink">Configuration Twitch</h2>
        <p className="mb-3 text-xs text-ink-faint">
          Nécessaire avant de pouvoir vous connecter : créez une application sur le portail
          développeur Twitch et renseignez son Client ID (identifiant public, ce n'est pas un
          secret).
        </p>
        <div className="mb-3 flex gap-2">
          <input
            className="text-input"
            placeholder="Client ID Twitch"
            value={clientIdDraft}
            onChange={(e) => {
              setClientIdDraft(e.target.value);
              setClientIdError(null);
            }}
            onKeyDown={(e) => e.key === "Enter" && handleSaveClientId()}
          />
          <button className="btn-primary shrink-0" onClick={handleSaveClientId}>
            <Save size={15} /> Enregistrer
          </button>
        </div>
        {clientIdError && <p className="mb-2 text-xs text-alert">{clientIdError}</p>}
        {clientIdSaved && <p className="mb-2 text-xs text-live">Client ID enregistré.</p>}
        <button
          className="btn-secondary"
          onClick={() => openExternalUrl(TWITCH_DEV_CONSOLE_URL, setOpenUrlError)}
        >
          <ArrowUpRight size={15} /> Créer une application Twitch
        </button>
      </div>

      <div className="panel p-5">
        <h2 className="mb-3 text-sm font-medium text-ink">Compte Twitch</h2>
        {currentAccount ? (
          <div className="flex items-center justify-between">
            <span className="text-sm text-ink">
              Connecté en tant que <strong>{currentAccount}</strong>
            </span>
            <button className="btn-danger" onClick={logout}>
              Se déconnecter
            </button>
          </div>
        ) : pendingDeviceCode ? (
          <div className="space-y-3">
            <p className="text-sm text-ink-muted">
              Ouvrez la page ci-dessous et saisissez le code pour autoriser l'application :
            </p>
            <div className="flex items-center gap-3">
              <code className="rounded-md bg-base-800 px-4 py-2 font-mono text-lg tracking-widest text-signal-bright">
                {pendingDeviceCode.user_code}
              </code>
              <button
                className="btn-secondary"
                onClick={() => openExternalUrl(pendingDeviceCode.verification_uri, setOpenUrlError)}
              >
                <ExternalLink size={15} /> Ouvrir Twitch
              </button>
            </div>
            {isPolling && <p className="text-xs text-ink-faint">En attente de validation…</p>}
          </div>
        ) : (
          <>
            <button className="btn-primary" onClick={startLogin} disabled={!isClientIdConfigured}>
              Se connecter avec Twitch
            </button>
            {!isClientIdConfigured && (
              <p className="mt-2 text-xs text-caution">
                Renseignez d'abord un Client ID Twitch ci-dessus.
              </p>
            )}
          </>
        )}
        {error && <p className="mt-3 text-xs text-alert">{error}</p>}
      </div>

      <div className="panel p-5">
        <h2 className="mb-3 text-sm font-medium text-ink">Chaînes suivies</h2>
        <div className="mb-4 flex gap-2">
          <input
            className="text-input"
            placeholder="nom_de_la_chaine"
            value={newChannel}
            onChange={(e) => setNewChannel(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && handleAddChannel()}
          />
          <button className="btn-primary" onClick={handleAddChannel}>
            <Plus size={15} /> Ajouter
          </button>
        </div>

        <div className="space-y-2">
          {Object.entries(statuses).map(([channel, status]) => (
            <div
              key={channel}
              className="flex items-center justify-between rounded-md border border-base-border bg-base-800 px-4 py-3"
            >
              <span className="text-sm text-ink">#{channel}</span>
              <div className="flex items-center gap-3">
                <StatusBadge status={status} />
                {status === "disconnected" ? (
                  <button
                    className="text-xs text-signal-bright hover:underline"
                    onClick={() => connect(channel)}
                  >
                    Reconnecter
                  </button>
                ) : (
                  <button
                    className="text-ink-faint hover:text-alert"
                    onClick={() => disconnect(channel)}
                    title="Déconnecter"
                  >
                    <Trash2 size={16} />
                  </button>
                )}
              </div>
            </div>
          ))}
          {Object.keys(statuses).length === 0 && (
            <p className="text-xs text-ink-faint">Aucune chaîne ajoutée pour le moment.</p>
          )}
        </div>
      </div>

      {notices.length > 0 && (
        <div className="panel p-5">
          <h2 className="mb-3 text-sm font-medium text-ink">Journal de connexion</h2>
          <div className="max-h-48 space-y-1 overflow-y-auto font-mono text-xs text-ink-faint">
            {notices.map((n, i) => (
              <div key={i}>
                <span className="text-signal-bright">#{n.channel}</span> — {n.text}
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
