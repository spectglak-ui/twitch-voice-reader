import clsx from "clsx";
import type { ConnectionStatus } from "@/types/chat";

const STATUS_LABEL: Record<ConnectionStatus, string> = {
  connected: "En direct",
  connecting: "Connexion…",
  reconnecting: "Reconnexion…",
  disconnected: "Déconnecté",
};

const STATUS_COLOR: Record<ConnectionStatus, string> = {
  connected: "bg-live/15 text-live",
  connecting: "bg-caution/15 text-caution",
  reconnecting: "bg-caution/15 text-caution",
  disconnected: "bg-alert/15 text-alert",
};

export function StatusBadge({ status }: { status: ConnectionStatus }) {
  return (
    <span
      className={clsx(
        "inline-flex items-center gap-1.5 rounded-full px-2.5 py-1 text-xs font-medium",
        STATUS_COLOR[status],
      )}
    >
      <span
        className={clsx(
          "h-1.5 w-1.5 rounded-full bg-current",
          (status === "connecting" || status === "reconnecting") && "animate-pulse-dot",
        )}
      />
      {STATUS_LABEL[status]}
    </span>
  );
}
