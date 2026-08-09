import { useEffect, useState } from "react";
import { useConfigStore } from "@/store/configStore";
import { Toggle } from "@/components/ui/Toggle";
import { Slider } from "@/components/ui/Slider";
import { X, Plus } from "lucide-react";

function WordListEditor({
  words,
  onChange,
  placeholder,
}: {
  words: string[];
  onChange: (words: string[]) => void;
  placeholder: string;
}) {
  const [draft, setDraft] = useState("");
  return (
    <div>
      <div className="mb-2 flex gap-2">
        <input
          className="text-input"
          placeholder={placeholder}
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && draft.trim()) {
              onChange([...words, draft.trim()]);
              setDraft("");
            }
          }}
        />
        <button
          className="btn-secondary shrink-0"
          onClick={() => {
            if (draft.trim()) {
              onChange([...words, draft.trim()]);
              setDraft("");
            }
          }}
        >
          <Plus size={15} />
        </button>
      </div>
      <div className="flex flex-wrap gap-2">
        {words.map((w, i) => (
          <span
            key={`${w}-${i}`}
            className="flex items-center gap-1.5 rounded-full bg-base-800 px-3 py-1 text-xs text-ink-muted"
          >
            {w}
            <button onClick={() => onChange(words.filter((_, idx) => idx !== i))}>
              <X size={12} />
            </button>
          </span>
        ))}
      </div>
    </div>
  );
}

export function Filters() {
  const { config, load, setFilters, setAntiSpam } = useConfigStore();

  useEffect(() => {
    load();
  }, [load]);

  if (!config) return <p className="text-sm text-ink-faint">Chargement…</p>;
  const { filters, anti_spam } = config;

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-semibold text-ink">Filtres</h1>
        <p className="text-sm text-ink-faint">Contrôlez précisément ce qui est lu à voix haute</p>
      </div>

      <div className="grid grid-cols-2 gap-6">
        <div className="panel p-5">
          <h2 className="mb-4 text-sm font-medium text-ink">Longueur des messages</h2>
          <Slider
            label="Longueur minimale"
            value={filters.min_length}
            min={0}
            max={50}
            step={1}
            onChange={(min_length) => setFilters({ ...filters, min_length })}
            formatValue={(v) => `${v} caractères`}
          />
          <Slider
            label="Longueur maximale"
            value={filters.max_length}
            min={20}
            max={500}
            step={10}
            onChange={(max_length) => setFilters({ ...filters, max_length })}
            formatValue={(v) => `${v} caractères`}
          />

          <div className="mt-2">
            <Toggle
              checked={filters.ignore_emote_only_messages}
              onChange={(v) => setFilters({ ...filters, ignore_emote_only_messages: v })}
              label="Ignorer les messages uniquement composés d'emotes"
            />
            <Toggle
              checked={filters.ignore_links}
              onChange={(v) => setFilters({ ...filters, ignore_links: v })}
              label="Ignorer les messages contenant un lien"
            />
          </div>
        </div>

        <div className="panel p-5">
          <h2 className="mb-4 text-sm font-medium text-ink">Restriction par rôle</h2>
          <p className="mb-2 text-xs text-ink-faint">
            Si aucune case n'est cochée, tout le monde est lu. Chaque case ajoute un rôle autorisé.
          </p>
          <Toggle
            checked={filters.roles.broadcaster_only}
            onChange={(v) => setFilters({ ...filters, roles: { ...filters.roles, broadcaster_only: v } })}
            label="Streamer"
          />
          <Toggle
            checked={filters.roles.moderators_only}
            onChange={(v) => setFilters({ ...filters, roles: { ...filters.roles, moderators_only: v } })}
            label="Modérateurs"
          />
          <Toggle
            checked={filters.roles.vips_only}
            onChange={(v) => setFilters({ ...filters, roles: { ...filters.roles, vips_only: v } })}
            label="VIP"
          />
          <Toggle
            checked={filters.roles.subscribers_only}
            onChange={(v) => setFilters({ ...filters, roles: { ...filters.roles, subscribers_only: v } })}
            label="Abonnés"
          />
        </div>
      </div>

      <div className="grid grid-cols-2 gap-6">
        <div className="panel p-5">
          <h2 className="mb-4 text-sm font-medium text-ink">Liste noire</h2>
          <WordListEditor
            words={filters.blacklist_words}
            onChange={(blacklist_words) => setFilters({ ...filters, blacklist_words })}
            placeholder="Ajouter un mot interdit…"
          />
        </div>
        <div className="panel p-5">
          <div className="mb-4 flex items-center justify-between">
            <h2 className="text-sm font-medium text-ink">Liste blanche</h2>
            <Toggle
              checked={filters.whitelist_mode_enabled}
              onChange={(v) => setFilters({ ...filters, whitelist_mode_enabled: v })}
              label="Mode strict"
              description="Lire uniquement les messages contenant un mot listé"
            />
          </div>
          <WordListEditor
            words={filters.whitelist_words}
            onChange={(whitelist_words) => setFilters({ ...filters, whitelist_words })}
            placeholder="Ajouter un mot…"
          />
        </div>
      </div>

      <div className="panel p-5">
        <h2 className="mb-4 text-sm font-medium text-ink">Utilisateurs ignorés</h2>
        <WordListEditor
          words={filters.ignored_users}
          onChange={(ignored_users) => setFilters({ ...filters, ignored_users })}
          placeholder="pseudo Twitch à ignorer…"
        />
      </div>

      <div className="panel p-5">
        <div className="mb-4 flex items-center justify-between">
          <h2 className="text-sm font-medium text-ink">Anti-spam</h2>
          <Toggle
            checked={anti_spam.enabled}
            onChange={(v) => setAntiSpam({ ...anti_spam, enabled: v })}
          />
        </div>
        <div className="grid grid-cols-3 gap-6">
          <Slider
            label="Messages max / minute"
            value={anti_spam.max_messages_per_minute}
            min={1}
            max={60}
            step={1}
            onChange={(max_messages_per_minute) =>
              setAntiSpam({ ...anti_spam, max_messages_per_minute })
            }
            formatValue={(v) => `${v}/min`}
          />
          <Slider
            label="Fenêtre de regroupement des doublons"
            value={anti_spam.duplicate_grouping_window_secs}
            min={1}
            max={60}
            step={1}
            onChange={(duplicate_grouping_window_secs) =>
              setAntiSpam({ ...anti_spam, duplicate_grouping_window_secs })
            }
            formatValue={(v) => `${v}s`}
          />
          <Slider
            label="Seuil de répétition (coupure)"
            value={anti_spam.repetition_threshold}
            min={2}
            max={20}
            step={1}
            onChange={(repetition_threshold) =>
              setAntiSpam({ ...anti_spam, repetition_threshold })
            }
            formatValue={(v) => `${v}×`}
          />
        </div>
      </div>
    </div>
  );
}
