import { create } from "zustand";
import { api } from "@/lib/tauri";
import type {
  AppConfig,
  AntiSpamConfig,
  AudioConfig,
  FiltersConfig,
  GeneralConfig,
  OverlayConfig,
  TtsConfig,
} from "@/types/config";

interface ConfigState {
  config: AppConfig | null;
  isLoading: boolean;
  error: string | null;

  load: () => Promise<void>;
  setTwitchClientId: (clientId: string) => Promise<void>;
  setTts: (tts: TtsConfig) => Promise<void>;
  setAudio: (audio: AudioConfig) => Promise<void>;
  setFilters: (filters: FiltersConfig) => Promise<void>;
  setAntiSpam: (antiSpam: AntiSpamConfig) => Promise<void>;
  setOverlay: (overlay: OverlayConfig) => Promise<void>;
  setGeneral: (general: GeneralConfig) => Promise<void>;
  setUserVoice: (login: string, voiceId: string | null) => Promise<void>;
  setRoleVoice: (role: string, voiceId: string | null) => Promise<void>;
  exportTo: (path: string) => Promise<void>;
  importFrom: (path: string) => Promise<void>;
  reset: () => Promise<void>;
}

/** Petite aide pour factoriser le pattern "appeler l'IPC, puis remplacer
 * l'état local par la config retournée (source de vérité = backend)". */
function applyUpdate(
  set: (partial: Partial<ConfigState>) => void,
  promise: Promise<AppConfig>,
): Promise<void> {
  return promise
    .then((config) => set({ config, error: null }))
    .catch((err) => set({ error: String(err) }));
}

export const useConfigStore = create<ConfigState>((set) => ({
  config: null,
  isLoading: false,
  error: null,

  load: async () => {
    set({ isLoading: true });
    try {
      const config = await api.config.get();
      set({ config, isLoading: false, error: null });
    } catch (err) {
      set({ isLoading: false, error: String(err) });
    }
  },

  // Rejette l'erreur (au lieu de seulement la stocker dans `error` comme
  // les autres setters) : le champ Client ID a besoin d'un retour de
  // validation immédiat et localisé (ex: "ne peut pas être vide"), pas
  // d'un message d'erreur générique perdu ailleurs dans la page.
  setTwitchClientId: async (clientId) => {
    const config = await api.config.updateTwitchClientId(clientId);
    set({ config, error: null });
  },

  setTts: (tts) => applyUpdate(set, api.config.updateTts(tts)),
  setAudio: (audio) => applyUpdate(set, api.config.updateAudio(audio)),
  setFilters: (filters) => applyUpdate(set, api.config.updateFilters(filters)),
  setAntiSpam: (antiSpam) => applyUpdate(set, api.config.updateAntiSpam(antiSpam)),
  setOverlay: (overlay) => applyUpdate(set, api.config.updateOverlay(overlay)),
  setGeneral: (general) => applyUpdate(set, api.config.updateGeneral(general)),
  setUserVoice: (login, voiceId) => applyUpdate(set, api.config.setUserVoice(login, voiceId)),
  setRoleVoice: (role, voiceId) => applyUpdate(set, api.config.setRoleVoice(role, voiceId)),
  exportTo: (path) => api.config.export(path),
  importFrom: (path) => applyUpdate(set, api.config.import(path)),
  reset: () => applyUpdate(set, api.config.reset()),
}));
