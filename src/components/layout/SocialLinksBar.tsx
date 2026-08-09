import { useState } from "react";
import { SiGithub, SiDiscord, SiTwitch, SiYoutube, SiX } from "@icons-pack/react-simple-icons";
import { openUrl } from "@tauri-apps/plugin-opener";

/**
 * Barre de liens sociaux permanente (pied de la barre latérale).
 *
 * Icônes officielles via `@icons-pack/react-simple-icons` (composants React
 * dédiés au-dessus de Simple Icons) plutôt que des tracés SVG recopiés à la
 * main : Lucide ne fournit délibérément aucune icône de marque (Discord,
 * Twitch, X sont explicitement refusés par leur politique — GitHub/YouTube
 * n'auraient couvert que 2 des 5 réseaux demandés), et un tracé SVG
 * mal recopié produit une icône silencieusement corrompue plutôt qu'une
 * erreur de compilation claire.
 *
 * `color="currentColor"` sur chaque icône : elles héritent de la couleur
 * de texte Tailwind du bouton parent plutôt que d'une couleur de marque
 * fixe, pour rester cohérentes avec le thème de l'application (et,
 * lorsqu'un mode clair sera implémenté, s'adapter automatiquement — voir
 * `general.theme` dans la configuration, actuellement non branché sur le
 * rendu).
 */

interface SocialLink {
  name: string;
  url: string;
  Icon: typeof SiGithub;
}

const SOCIAL_LINKS: SocialLink[] = [
  { name: "GitHub", url: "https://github.com/spectglak-ui", Icon: SiGithub },
  { name: "Discord", url: "https://discord.com/invite/rt3kMuU935", Icon: SiDiscord },
  { name: "Twitch", url: "https://www.twitch.tv/spectglack", Icon: SiTwitch },
  { name: "YouTube", url: "https://www.youtube.com/@Spectglack", Icon: SiYoutube },
  { name: "X (Twitter)", url: "https://x.com/spectglakstream", Icon: SiX },
];

function formatOpenError(err: unknown): string {
  if (err && typeof err === "object" && "message" in err) {
    return String((err as { message: unknown }).message);
  }
  return String(err);
}

export function SocialLinksBar() {
  const [error, setError] = useState<string | null>(null);

  const handleOpen = async (url: string) => {
    setError(null);
    try {
      // Plugin Tauri Opener déjà utilisé ailleurs dans l'app (Connexions
      // Twitch) — ouvre toujours le navigateur système par défaut, jamais
      // la WebView interne, sur les trois plateformes. Nécessite la
      // permission `opener:default` (déjà déclarée dans
      // `capabilities/default.json`).
      await openUrl(url);
    } catch (err) {
      setError(formatOpenError(err));
    }
  };

  return (
    <div className="border-t border-base-border px-5 py-3">
      <p className="mb-2 text-[10px] font-medium uppercase tracking-wide text-ink-faint">
        Retrouvez Spectglack sur
      </p>
      <div className="flex flex-wrap items-center gap-1">
        {SOCIAL_LINKS.map(({ name, url, Icon }) => (
          <div key={name} className="group relative">
            <button
              type="button"
              aria-label={name}
              title={name}
              onClick={() => handleOpen(url)}
              className="flex h-7 w-7 items-center justify-center rounded-md text-ink-faint
                         transition-colors hover:bg-base-800 hover:text-signal-bright"
            >
              <Icon size={15} color="currentColor" />
            </button>

            {/* Tooltip discret, cohérent avec le reste du design (panneaux
                base-800 / bordure base-border) plutôt qu'un `title`
                navigateur brut. */}
            <span
              role="tooltip"
              className="pointer-events-none absolute -top-8 left-1/2 -translate-x-1/2 whitespace-nowrap
                         rounded-md border border-base-border bg-base-800 px-2 py-1 text-[10px] text-ink
                         opacity-0 shadow-lg transition-opacity duration-150 group-hover:opacity-100"
            >
              {name}
            </span>
          </div>
        ))}
      </div>

      {error && <p className="mt-2 text-[10px] text-alert">{error}</p>}
    </div>
  );
}
