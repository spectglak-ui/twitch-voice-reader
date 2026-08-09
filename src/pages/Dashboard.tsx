import { useEffect } from "react";
import { LineChart, Line, XAxis, YAxis, Tooltip, ResponsiveContainer, CartesianGrid } from "recharts";
import { MessageSquare, MicOff, Users, Timer } from "lucide-react";
import { useStatsStore } from "@/store/statsStore";
import { useChatStore } from "@/store/chatStore";
import { useConnectionStore } from "@/store/connectionStore";
import { RoleBadge } from "@/components/ui/RoleBadge";
import { StatusBadge } from "@/components/ui/StatusBadge";

function StatCard({
  icon: Icon,
  label,
  value,
  accent,
}: {
  icon: typeof MessageSquare;
  label: string;
  value: string;
  accent: string;
}) {
  return (
    <div className="panel flex items-center gap-4 p-4">
      <div className={`flex h-10 w-10 shrink-0 items-center justify-center rounded-md ${accent}`}>
        <Icon size={18} />
      </div>
      <div>
        <div className="text-xl font-display font-semibold text-ink">{value}</div>
        <div className="text-xs text-ink-faint">{label}</div>
      </div>
    </div>
  );
}

function formatDuration(ms: number): string {
  const minutes = Math.floor(ms / 60000);
  const seconds = Math.floor((ms % 60000) / 1000);
  return `${minutes}m ${seconds}s`;
}

export function Dashboard() {
  const session = useStatsStore((s) => s.session);
  const summary = useStatsStore((s) => s.summary);
  const refreshSession = useStatsStore((s) => s.refreshSession);
  const refreshSummary = useStatsStore((s) => s.refreshSummary);
  const messages = useChatStore((s) => s.messages);
  const statuses = useConnectionStore((s) => s.statuses);

  useEffect(() => {
    refreshSession();
    refreshSummary(14);
    const interval = setInterval(refreshSession, 5000);
    return () => clearInterval(interval);
  }, [refreshSession, refreshSummary]);

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-semibold text-ink">Tableau de bord</h1>
        <p className="text-sm text-ink-faint">Vue d'ensemble de la session en cours</p>
      </div>

      <div className="grid grid-cols-4 gap-4">
        <StatCard
          icon={MessageSquare}
          label="Messages lus"
          value={String(session?.messages_read ?? 0)}
          accent="bg-live/15 text-live"
        />
        <StatCard
          icon={MicOff}
          label="Messages ignorés"
          value={String(session?.messages_ignored ?? 0)}
          accent="bg-alert/15 text-alert"
        />
        <StatCard
          icon={Users}
          label="Utilisateurs actifs"
          value={String(session?.active_users_count ?? 0)}
          accent="bg-signal/15 text-signal-bright"
        />
        <StatCard
          icon={Timer}
          label="Temps de lecture"
          value={formatDuration(session?.total_reading_time_ms ?? 0)}
          accent="bg-caution/15 text-caution"
        />
      </div>

      <div className="grid grid-cols-3 gap-6">
        <div className="panel col-span-2 p-5">
          <h2 className="mb-4 text-sm font-medium text-ink">Messages lus (14 derniers jours)</h2>
          <ResponsiveContainer width="100%" height={220}>
            <LineChart data={summary?.daily_breakdown ?? []}>
              <CartesianGrid strokeDasharray="3 3" stroke="#2A2440" />
              <XAxis dataKey="day" tick={{ fontSize: 11, fill: "#7A7195" }} tickLine={false} axisLine={false} />
              <YAxis tick={{ fontSize: 11, fill: "#7A7195" }} tickLine={false} axisLine={false} />
              <Tooltip
                contentStyle={{ background: "#181426", border: "1px solid #2A2440", borderRadius: 8 }}
                labelStyle={{ color: "#F1EEFB" }}
              />
              <Line type="monotone" dataKey="messages_read" stroke="#7C5CFC" strokeWidth={2} dot={false} />
            </LineChart>
          </ResponsiveContainer>
        </div>

        <div className="panel p-5">
          <h2 className="mb-4 text-sm font-medium text-ink">Chaînes</h2>
          <div className="space-y-2">
            {Object.entries(statuses).length === 0 && (
              <p className="text-xs text-ink-faint">Aucune chaîne connectée pour le moment.</p>
            )}
            {Object.entries(statuses).map(([channel, status]) => (
              <div key={channel} className="flex items-center justify-between">
                <span className="text-sm text-ink">#{channel}</span>
                <StatusBadge status={status} />
              </div>
            ))}
          </div>
        </div>
      </div>

      <div className="panel p-5">
        <h2 className="mb-4 text-sm font-medium text-ink">Flux de chat récent</h2>
        <div className="max-h-80 space-y-2 overflow-y-auto">
          {messages.slice(0, 30).map((m) => (
            <div
              key={m.id}
              className={`flex items-start gap-2 rounded-md px-2 py-1.5 text-sm ${
                m.wasReadAloud ? "" : "opacity-40"
              }`}
            >
              <span className="font-medium text-ink">{m.display_name}</span>
              <RoleBadge role={m.role} />
              <span className="min-w-0 flex-1 truncate text-ink-muted">{m.text}</span>
            </div>
          ))}
          {messages.length === 0 && (
            <p className="text-xs text-ink-faint">
              Aucun message reçu pour l'instant — connectez une chaîne depuis l'onglet « Connexions ».
            </p>
          )}
        </div>
      </div>
    </div>
  );
}
