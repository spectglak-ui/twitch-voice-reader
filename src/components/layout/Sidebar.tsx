import { NavLink } from "react-router-dom";
import clsx from "clsx";
import {
  LayoutDashboard,
  Radio,
  Mic2,
  ListFilter,
  History,
  Settings,
} from "lucide-react";
import logo from "@/assets/logo.webp";
import { SocialLinksBar } from "@/components/layout/SocialLinksBar";

const NAV_ITEMS = [
  { to: "/", label: "Tableau de bord", icon: LayoutDashboard, end: true },
  { to: "/connections", label: "Connexions Twitch", icon: Radio },
  { to: "/voice", label: "Voix et TTS", icon: Mic2 },
  { to: "/filters", label: "Filtres", icon: ListFilter },
  { to: "/history", label: "Historique", icon: History },
  { to: "/settings", label: "Paramètres", icon: Settings },
];

export function Sidebar() {
  return (
    <aside className="flex h-full w-60 shrink-0 flex-col border-r border-base-border bg-base-900">
      <div className="flex items-center gap-2.5 px-5 py-5">
        <img src={logo} alt="Twitch Voice Reader" className="h-8 w-8 rounded-md object-cover" />
        <div>
          <div className="font-display text-sm font-semibold leading-tight text-ink">
            Twitch Voice
          </div>
          <div className="font-display text-sm font-semibold leading-tight text-signal-bright">
            Reader
          </div>
        </div>
      </div>

      <nav className="flex-1 space-y-1 px-3">
        {NAV_ITEMS.map(({ to, label, icon: Icon, end }) => (
          <NavLink
            key={to}
            to={to}
            end={end}
            className={({ isActive }) =>
              clsx(
                "flex items-center gap-3 rounded-md px-3 py-2.5 text-sm font-medium transition-colors",
                isActive
                  ? "bg-signal/15 text-signal-bright"
                  : "text-ink-muted hover:bg-base-800 hover:text-ink",
              )
            }
          >
            <Icon size={17} strokeWidth={2} />
            {label}
          </NavLink>
        ))}
      </nav>

      <SocialLinksBar />
      <div className="px-5 py-4 text-[11px] text-ink-faint">Twitch Voice Reader v0.1.0</div>
    </aside>
  );
}
