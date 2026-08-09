import { useEffect, useState } from "react";
import { useStatsStore } from "@/store/statsStore";
import { RoleBadge } from "@/components/ui/RoleBadge";
import clsx from "clsx";

const REJECTION_LABEL: Record<string, string> = {
  UserIgnored: "Utilisateur ignoré",
  TooShort: "Trop court",
  TooLong: "Trop long",
  EmoteOnly: "Emotes uniquement",
  ContainsLink: "Contient un lien",
  Blacklisted: "Mot interdit",
  NotWhitelisted: "Hors liste blanche",
  RoleNotAllowed: "Rôle non autorisé",
};

export function History() {
  const { history, refreshHistory } = useStatsStore();
  const [filter, setFilter] = useState<"all" | "read" | "ignored">("all");

  useEffect(() => {
    refreshHistory(300);
  }, [refreshHistory]);

  const filtered = history.filter((h) => {
    if (filter === "read") return h.was_read_aloud;
    if (filter === "ignored") return !h.was_read_aloud;
    return true;
  });

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-semibold text-ink">Historique</h1>
          <p className="text-sm text-ink-faint">{filtered.length} messages</p>
        </div>
        <div className="flex gap-1 rounded-md border border-base-border bg-base-800 p-1">
          {(["all", "read", "ignored"] as const).map((f) => (
            <button
              key={f}
              onClick={() => setFilter(f)}
              className={clsx(
                "rounded px-3 py-1.5 text-xs font-medium transition-colors",
                filter === f ? "bg-signal text-white" : "text-ink-muted hover:text-ink",
              )}
            >
              {f === "all" ? "Tous" : f === "read" ? "Lus" : "Ignorés"}
            </button>
          ))}
        </div>
      </div>

      <div className="panel divide-y divide-base-border">
        {filtered.map((entry) => (
          <div key={entry.id} className="flex items-start gap-3 px-4 py-3">
            <span className="w-16 shrink-0 pt-0.5 font-mono text-[11px] text-ink-faint">
              {new Date(entry.created_at_ms).toLocaleTimeString("fr-FR", {
                hour: "2-digit",
                minute: "2-digit",
              })}
            </span>
            <div className="min-w-0 flex-1">
              <div className="flex items-center gap-2">
                <span className="text-sm font-medium text-ink">{entry.display_name}</span>
                <RoleBadge role={entry.role as any} compact />
                <span className="text-xs text-ink-faint">#{entry.channel}</span>
              </div>
              <p className={clsx("text-sm", entry.was_read_aloud ? "text-ink-muted" : "text-ink-faint line-through")}>
                {entry.text}
              </p>
            </div>
            {!entry.was_read_aloud && entry.rejection_reason && (
              <span className="shrink-0 rounded bg-alert/10 px-2 py-0.5 text-[10px] text-alert">
                {REJECTION_LABEL[entry.rejection_reason] ?? entry.rejection_reason}
              </span>
            )}
          </div>
        ))}
        {filtered.length === 0 && (
          <p className="px-4 py-8 text-center text-sm text-ink-faint">Aucun message dans l'historique.</p>
        )}
      </div>
    </div>
  );
}
