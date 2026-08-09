import { useEffect, useState } from "react";
import { save, open } from "@tauri-apps/plugin-dialog";
import { useConfigStore } from "@/store/configStore";
import { Toggle } from "@/components/ui/Toggle";
import { Slider } from "@/components/ui/Slider";
import { api } from "@/lib/tauri";
import { Download, Upload, RotateCcw, Copy } from "lucide-react";

export function Settings() {
  const { config, load, setOverlay, setGeneral, exportTo, importFrom, reset } = useConfigStore();
  const [overlayRunning, setOverlayRunning] = useState(false);

  useEffect(() => {
    load();
    api.overlay.isRunning().then(setOverlayRunning);
  }, [load]);

  if (!config) return <p className="text-sm text-ink-faint">Chargement…</p>;
  const { overlay } = config;

  const toggleOverlay = async () => {
    if (overlayRunning) {
      await api.overlay.stop();
      setOverlayRunning(false);
    } else {
      await api.overlay.start();
      setOverlayRunning(true);
    }
  };

  const overlayUrl = `http://127.0.0.1:${overlay.http_port}/overlay`;

  const handleExport = async () => {
    const path = await save({ defaultPath: "twitch-voice-reader-config.json", filters: [{ name: "JSON", extensions: ["json"] }] });
    if (path) await exportTo(path);
  };

  const handleImport = async () => {
    const path = await open({ multiple: false, filters: [{ name: "JSON", extensions: ["json"] }] });
    if (typeof path === "string") await importFrom(path);
  };

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-semibold text-ink">Paramètres</h1>
        <p className="text-sm text-ink-faint">Général, overlay de stream et gestion de la configuration</p>
      </div>

      <div className="panel p-5">
        <h2 className="mb-4 text-sm font-medium text-ink">Overlay de stream</h2>
        <p className="mb-4 text-xs text-ink-faint">
          Ajoutez cette URL comme « Browser Source » dans OBS Studio / Streamlabs pour afficher le
          message en cours de lecture, le pseudo et une animation.
        </p>
        <div className="mb-4 flex items-center gap-2">
          <code className="flex-1 truncate rounded-md bg-base-800 px-3 py-2 font-mono text-xs text-ink-muted">
            {overlayUrl}
          </code>
          <button className="btn-secondary" onClick={() => navigator.clipboard.writeText(overlayUrl)}>
            <Copy size={14} />
          </button>
          <button className={overlayRunning ? "btn-danger" : "btn-primary"} onClick={toggleOverlay}>
            {overlayRunning ? "Arrêter" : "Démarrer"}
          </button>
        </div>
        <Toggle
          checked={overlay.show_avatar}
          onChange={(v) => setOverlay({ ...overlay, show_avatar: v })}
          label="Afficher l'avatar"
        />
        <Toggle
          checked={overlay.show_username}
          onChange={(v) => setOverlay({ ...overlay, show_username: v })}
          label="Afficher le pseudo"
        />
      </div>

      <div className="panel p-5">
        <h2 className="mb-4 text-sm font-medium text-ink">Général</h2>
        <Toggle
          checked={config.general.start_minimized_to_tray}
          onChange={(v) => setGeneral({ ...config.general, start_minimized_to_tray: v })}
          label="Démarrer minimisé dans la zone de notification"
        />
        <Toggle
          checked={config.general.launch_on_system_startup}
          onChange={(v) => setGeneral({ ...config.general, launch_on_system_startup: v })}
          label="Lancer au démarrage du système"
        />
        <Slider
          label="Rétention de l'historique"
          value={config.general.history_retention_days}
          min={1}
          max={90}
          step={1}
          onChange={(v) => setGeneral({ ...config.general, history_retention_days: v })}
          formatValue={(v) => `${v} jours`}
        />
      </div>

      <div className="panel p-5">
        <h2 className="mb-4 text-sm font-medium text-ink">Configuration</h2>
        <div className="flex gap-3">
          <button className="btn-secondary" onClick={handleExport}>
            <Download size={15} /> Exporter en JSON
          </button>
          <button className="btn-secondary" onClick={handleImport}>
            <Upload size={15} /> Importer un JSON
          </button>
          <button
            className="btn-danger"
            onClick={() => {
              if (confirm("Réinitialiser tous les paramètres ? Cette action est irréversible.")) reset();
            }}
          >
            <RotateCcw size={15} /> Réinitialiser
          </button>
        </div>
        <p className="mt-3 text-[11px] text-ink-faint">
          Les jetons d'authentification Twitch ne sont jamais inclus dans l'export (stockés séparément
          dans le trousseau système pour votre sécurité).
        </p>
      </div>
    </div>
  );
}
