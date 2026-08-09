import { useEffect, useState } from "react";
import { Play, Volume2, AlertTriangle, Loader2, RotateCw, CheckCircle2 } from "lucide-react";
import { useConfigStore } from "@/store/configStore";
import { Slider } from "@/components/ui/Slider";
import { Toggle } from "@/components/ui/Toggle";
import { api, events } from "@/lib/tauri";
import type { AudioDeviceInfo, InstallProgress } from "@/types/tts";

const ROLE_OPTIONS = [
  { value: "broadcaster", label: "Streamer" },
  { value: "moderator", label: "Modérateur" },
  { value: "vip", label: "VIP" },
  { value: "subscriber", label: "Abonné" },
  { value: "viewer", label: "Spectateur" },
];

/** Formate l'erreur renvoyée par une commande Tauri (AppError sérialisée
 * en `{ kind, message }`, ou une simple chaîne selon le point d'échec) en
 * un texte lisible — pour ne jamais laisser un échec totalement muet. */
function formatInvokeError(err: unknown): string {
  if (err && typeof err === "object" && "message" in err) {
    return String((err as { message: unknown }).message);
  }
  return String(err);
}

function describeProgress(p: InstallProgress): string {
  switch (p.stage) {
    case "CheckingExisting":
      return "Vérification de l'installation existante…";
    case "Downloading":
      return `Téléchargement de ${p.label}${p.percent != null ? ` (${p.percent}%)` : "…"}`;
    case "Extracting":
      return "Extraction de l'archive…";
    case "Verifying":
      return "Vérification du binaire téléchargé…";
    case "DownloadingVoice":
      return `Téléchargement de la voix « ${p.label} »…`;
    case "Done":
      return "Moteur vocal prêt.";
    case "Error":
      return p.message;
  }
}

type EngineStatus = "checking" | "installing" | "ready" | "error";

export function Voice() {
  const { config, load, setTts, setAudio, setUserVoice, setRoleVoice } = useConfigStore();
  const [voices, setVoices] = useState<string[]>([]);
  const [devices, setDevices] = useState<AudioDeviceInfo[]>([]);
  const [testText, setTestText] = useState("Bonjour, ceci est un test de la synthèse vocale.");
  const [newUserLogin, setNewUserLogin] = useState("");
  const [newUserVoice, setNewUserVoice] = useState("");

  // Le moteur Piper est désormais installé automatiquement par
  // l'application elle-même si absent (téléchargement + extraction +
  // vérification), plutôt que de dépendre d'un script externe exécuté
  // manuellement avant le premier lancement — voir `tts/installer.rs`.
  // Cet état reflète la progression réelle de cette installation.
  const [engineStatus, setEngineStatus] = useState<EngineStatus>("checking");
  const [progressLabel, setProgressLabel] = useState("Vérification du moteur vocal…");
  const [installErrorDetail, setInstallErrorDetail] = useState<string | null>(null);

  const [testState, setTestState] = useState<"idle" | "loading" | "error">("idle");
  const [testError, setTestError] = useState<string | null>(null);

  const runEnsureInstalled = () => {
    setEngineStatus("installing");
    setInstallErrorDetail(null);
    api.tts
      .ensureInstalled()
      .then(() => {
        setEngineStatus("ready");
        api.tts.listInstalledVoices().then(setVoices);
      })
      .catch((err) => {
        setEngineStatus("error");
        setInstallErrorDetail(formatInvokeError(err));
      });
  };

  useEffect(() => {
    load();
    api.tts.listOutputDevices().then(setDevices);
    api.tts.listInstalledVoices().then(setVoices);

    const unlistenPromise = events.onPiperInstallProgress((progress) => {
      setProgressLabel(describeProgress(progress));
      if (progress.stage === "Done") {
        setEngineStatus("ready");
        api.tts.listInstalledVoices().then(setVoices);
      } else if (progress.stage === "Error") {
        setEngineStatus("error");
        setInstallErrorDetail(progress.message);
      }
    });

    runEnsureInstalled();

    return () => {
      unlistenPromise.then((unlisten) => unlisten());
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [load]);

  if (!config) return <p className="text-sm text-ink-faint">Chargement…</p>;
  const { tts, audio } = config;

  const handleTestVoice = async () => {
    setTestState("loading");
    setTestError(null);
    try {
      await api.tts.testVoice({
        text: testText,
        voiceId: tts.default_voice_id,
        volume: audio.master_volume * tts.volume,
        rate: tts.rate,
        pitch: tts.pitch,
      });
      setTestState("idle");
    } catch (err) {
      // C'est ici que se situait le bug d'origine : l'appel n'était ni
      // attendu ni entouré d'un `catch`, donc un échec (voix manquante,
      // Piper introuvable, périphérique audio invalide) ne produisait
      // strictement aucun retour visible — seulement un rejet de promesse
      // silencieux dans la console. Voir le rapport d'audit, section TTS.
      setTestState("error");
      setTestError(formatInvokeError(err));
    }
  };

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-semibold text-ink">Voix et TTS</h1>
        <p className="text-sm text-ink-faint">Synthèse vocale locale (Piper), voix et audio</p>
      </div>

      {engineStatus !== "ready" && (
        <div
          className={`panel flex items-start justify-between gap-3 p-4 ${
            engineStatus === "error" ? "border-alert/40 bg-alert/10" : "border-caution/40 bg-caution/10"
          }`}
        >
          <div className={`flex items-start gap-3 text-sm ${engineStatus === "error" ? "text-alert" : "text-caution"}`}>
            {engineStatus === "error" ? (
              <AlertTriangle size={18} className="mt-0.5 shrink-0" />
            ) : (
              <Loader2 size={18} className="mt-0.5 shrink-0 animate-spin" />
            )}
            <div>
              <p>{engineStatus === "error" ? "Installation du moteur vocal impossible" : progressLabel}</p>
              {engineStatus === "error" && installErrorDetail && (
                <p className="mt-1 text-xs opacity-80">{installErrorDetail}</p>
              )}
            </div>
          </div>
          {engineStatus === "error" && (
            <button className="btn-secondary shrink-0" onClick={runEnsureInstalled}>
              <RotateCw size={14} /> Réessayer
            </button>
          )}
        </div>
      )}

      {engineStatus === "ready" && voices.length > 0 && (
        <div className="flex items-center gap-2 text-xs text-live">
          <CheckCircle2 size={14} />
          Moteur vocal prêt ({voices.length} voix installée{voices.length !== 1 ? "s" : ""}).
        </div>
      )}

      <div className="grid grid-cols-2 gap-6">
        <div className="panel p-5">
          <h2 className="mb-4 text-sm font-medium text-ink">Voix par défaut</h2>
          <select
            className="text-input mb-4"
            value={tts.default_voice_id}
            onChange={(e) => setTts({ ...tts, default_voice_id: e.target.value })}
          >
            {voices.length === 0 && <option value={tts.default_voice_id}>{tts.default_voice_id}</option>}
            {voices.map((v) => (
              <option key={v} value={v}>
                {v}
              </option>
            ))}
          </select>

          <Slider
            label="Volume"
            value={tts.volume}
            min={0}
            max={1}
            onChange={(volume) => setTts({ ...tts, volume })}
            formatValue={(v) => `${Math.round(v * 100)}%`}
          />
          <Slider
            label="Vitesse"
            value={tts.rate}
            min={0.5}
            max={2}
            onChange={(rate) => setTts({ ...tts, rate })}
            formatValue={(v) => `${v.toFixed(2)}×`}
          />
          <Slider
            label="Hauteur de voix"
            value={tts.pitch}
            min={-0.9}
            max={1}
            onChange={(pitch) => setTts({ ...tts, pitch })}
            formatValue={(v) => (v > 0 ? `+${Math.round(v * 100)}%` : `${Math.round(v * 100)}%`)}
          />

          <div className="mt-4 flex gap-2">
            <input
              className="text-input"
              value={testText}
              onChange={(e) => setTestText(e.target.value)}
            />
            <button
              className="btn-primary shrink-0"
              onClick={handleTestVoice}
              disabled={testState === "loading"}
            >
              {testState === "loading" ? <Loader2 size={15} className="animate-spin" /> : <Play size={15} />}
              Tester
            </button>
          </div>
          {testState === "error" && testError && (
            <p className="mt-2 text-xs text-alert">{testError}</p>
          )}
        </div>

        <div className="panel p-5">
          <h2 className="mb-4 text-sm font-medium text-ink">Audio</h2>
          <label className="field-label mb-1 block">Périphérique de sortie</label>
          <select
            className="text-input mb-4"
            value={audio.output_device_name ?? ""}
            onChange={(e) =>
              setAudio({ ...audio, output_device_name: e.target.value || null })
            }
          >
            <option value="">Périphérique par défaut du système</option>
            {devices.map((d) => (
              <option key={d.name} value={d.name}>
                {d.name} {d.is_default ? "(par défaut)" : ""}
              </option>
            ))}
          </select>

          <Slider
            label="Volume général"
            value={audio.master_volume}
            min={0}
            max={1.5}
            onChange={(master_volume) => setAudio({ ...audio, master_volume })}
            formatValue={(v) => `${Math.round(v * 100)}%`}
          />

          <div className="mt-2 flex items-center gap-2 text-xs text-ink-faint">
            <Volume2 size={13} />
            Le volume final = volume général × volume TTS × volume par voix
          </div>

          <div className="mt-6">
            <Toggle
              checked={tts.read_username_before_message}
              onChange={(v) => setTts({ ...tts, read_username_before_message: v })}
              label="Annoncer le pseudo avant le message"
              description={'Ex : "Alex dit : salut le chat"'}
            />
            <Toggle
              checked={tts.auto_detect_language}
              onChange={(v) => setTts({ ...tts, auto_detect_language: v })}
              label="Détection automatique de la langue"
              description="Sélectionne une voix adaptée (FR/EN/ES/DE/IT) par message"
            />
          </div>
        </div>
      </div>

      {tts.auto_detect_language && (
        <div className="panel p-5">
          <h2 className="mb-4 text-sm font-medium text-ink">Voix par langue détectée</h2>
          <div className="grid grid-cols-5 gap-3">
            {Object.entries(tts.language_voice_map).map(([lang, voiceId]) => (
              <div key={lang}>
                <label className="field-label mb-1 block uppercase">{lang}</label>
                <input
                  className="text-input"
                  value={voiceId}
                  onChange={(e) =>
                    setTts({
                      ...tts,
                      language_voice_map: { ...tts.language_voice_map, [lang]: e.target.value },
                    })
                  }
                />
              </div>
            ))}
          </div>
        </div>
      )}

      <div className="panel p-5">
        <h2 className="mb-4 text-sm font-medium text-ink">Voix par rôle Twitch</h2>
        <div className="grid grid-cols-5 gap-3">
          {ROLE_OPTIONS.map(({ value, label }) => (
            <div key={value}>
              <label className="field-label mb-1 block">{label}</label>
              <select
                className="text-input"
                value={config.voice_assignments.per_role[value] ?? ""}
                onChange={(e) => setRoleVoice(value, e.target.value || null)}
              >
                <option value="">Voix par défaut</option>
                {voices.map((v) => (
                  <option key={v} value={v}>
                    {v}
                  </option>
                ))}
              </select>
            </div>
          ))}
        </div>
      </div>

      <div className="panel p-5">
        <h2 className="mb-4 text-sm font-medium text-ink">Voix par utilisateur</h2>
        <div className="mb-4 flex gap-2">
          <input
            className="text-input"
            placeholder="pseudo Twitch"
            value={newUserLogin}
            onChange={(e) => setNewUserLogin(e.target.value)}
          />
          <select className="text-input" value={newUserVoice} onChange={(e) => setNewUserVoice(e.target.value)}>
            <option value="">Choisir une voix…</option>
            {voices.map((v) => (
              <option key={v} value={v}>
                {v}
              </option>
            ))}
          </select>
          <button
            className="btn-primary shrink-0"
            onClick={() => {
              if (!newUserLogin || !newUserVoice) return;
              setUserVoice(newUserLogin.toLowerCase(), newUserVoice);
              setNewUserLogin("");
              setNewUserVoice("");
            }}
          >
            Attribuer
          </button>
        </div>
        <div className="space-y-2">
          {Object.entries(config.voice_assignments.per_user).map(([login, voiceId]) => (
            <div key={login} className="flex items-center justify-between text-sm">
              <span className="text-ink">{login}</span>
              <div className="flex items-center gap-3">
                <span className="font-mono text-xs text-ink-muted">{voiceId}</span>
                <button className="text-alert text-xs" onClick={() => setUserVoice(login, null)}>
                  Retirer
                </button>
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
