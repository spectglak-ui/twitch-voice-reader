import clsx from "clsx";
import type { TwitchRole } from "@/types/chat";

const ROLE_LABEL: Record<TwitchRole, string> = {
  viewer: "Spectateur",
  subscriber: "Abonné",
  vip: "VIP",
  moderator: "Modérateur",
  broadcaster: "Streamer",
};

const ROLE_COLOR: Record<TwitchRole, string> = {
  viewer: "text-ink-faint",
  subscriber: "text-signal-bright",
  vip: "text-caution",
  moderator: "text-live",
  broadcaster: "text-alert",
};

export function RoleBadge({ role, compact }: { role: TwitchRole; compact?: boolean }) {
  if (compact) {
    return <span className={clsx("text-xs font-medium", ROLE_COLOR[role])}>{ROLE_LABEL[role]}</span>;
  }
  return (
    <span className={clsx("rounded border border-current/30 px-1.5 py-0.5 text-[10px] font-medium", ROLE_COLOR[role])}>
      {ROLE_LABEL[role]}
    </span>
  );
}
