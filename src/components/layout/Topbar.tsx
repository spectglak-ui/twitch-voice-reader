import { useMemo } from "react";
import { Radio, User, AlertCircle, X } from "lucide-react";
import { useConnectionStore } from "@/store/connectionStore";
import { useChatStore } from "@/store/chatStore";
import { useAuthStore } from "@/store/authStore";
import { WaveformIndicator } from "@/components/ui/WaveformIndicator";

export function Topbar() {
  const statuses = useConnectionStore((s) => s.statuses);
  const currentlySpeaking = useChatStore((s) => s.currentlySpeaking);
  const queueSize = useChatStore((s) => s.queueSize);
  const lastTtsError = useChatStore((s) => s.lastTtsError);
  const dismissTtsError = useChatStore((s) => s.dismissTtsError);
  const currentAccount = useAuthStore((s) => s.currentAccount);

  const connectedCount = useMemo(
    () => Object.values(statuses).filter((s) => s === "connected").length,
    [statuses],
  );

  return (
    <div className="flex shrink-0 flex-col">
      <header className="flex h-16 shrink-0 items-center justify-between border-b border-base-border bg-base-950/60 px-6 backdrop-blur">
        <div className="flex items-center gap-5">
          <div className="flex items-center gap-2 text-sm text-ink-muted">
            <Radio size={16} className={connectedCount > 0 ? "text-live" : "text-ink-faint"} />
            <span>
              {connectedCount} chaîne{connectedCount !== 1 ? "s" : ""} connectée
              {connectedCount !== 1 ? "s" : ""}
            </span>
          </div>

          {queueSize > 0 && (
            <div className="rounded-full bg-caution/15 px-2.5 py-1 text-xs font-medium text-caution">
              {queueSize} message{queueSize !== 1 ? "s" : ""} en file
            </div>
          )}

          {currentlySpeaking && (
            <div className="flex items-center gap-2 rounded-full bg-live/10 px-3 py-1.5 text-xs text-live">
              <WaveformIndicator active className="text-live" />
              <span className="max-w-[280px] truncate">
                <strong>{currentlySpeaking.displayName}</strong> — {currentlySpeaking.text}
              </span>
            </div>
          )}
        </div>

        <div className="flex items-center gap-2 text-sm text-ink-muted">
          <User size={16} />
          {currentAccount ?? "Non connecté"}
        </div>
      </header>

      {/* Bandeau d'erreur TTS : avant ce correctif, un échec de lecture
          (voix manquante, Piper indisponible, timeout, périphérique audio
          invalide) n'était visible nulle part dans l'interface — voir le
          rapport d'audit. Placé dans le layout global pour rester visible
          quelle que soit la page consultée, puisque le chat continue d'être
          reçu et « lu » (silencieusement) en arrière-plan. */}
      {lastTtsError && (
        <div className="flex items-center justify-between gap-3 border-b border-alert/30 bg-alert/10 px-6 py-2">
          <div className="flex items-center gap-2 text-xs text-alert">
            <AlertCircle size={14} className="shrink-0" />
            <span>Erreur de lecture vocale : {lastTtsError.message}</span>
          </div>
          <button onClick={dismissTtsError} className="text-alert/70 hover:text-alert">
            <X size={14} />
          </button>
        </div>
      )}
    </div>
  );
}
