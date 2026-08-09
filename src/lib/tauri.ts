// Point d'entrée unique vers le backend Rust. Centraliser tous les appels
// `invoke(...)` ici (plutôt que de les disperser dans les composants)
// garantit un typage cohérent et un seul endroit à mettre à jour si une
// commande change de signature côté Rust.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AppConfig,
  AntiSpamConfig,
  AudioConfig,
  FiltersConfig,
  GeneralConfig,
  OverlayConfig,
  TtsConfig,
} from "@/types/config";
import type { ConnectionStatus, IncomingChatEvent } from "@/types/chat";
import type {
  AudioDeviceInfo,
  DeviceCodeResponse,
  HistoryEntry,
  InstallProgress,
  SessionStatsSnapshot,
  StatsSummary,
  TtsPlaybackEvent,
} from "@/types/tts";

// --- Authentification -----------------------------------------------------
export const api = {
  auth: {
    startLogin: () => invoke<DeviceCodeResponse>("twitch_start_login"),
    logout: () => invoke<void>("twitch_logout"),
    currentAccount: () => invoke<string | null>("twitch_current_account"),
  },

  twitch: {
    connectChannel: (login: string) => invoke<void>("twitch_connect_channel", { login }),
    disconnectChannel: (login: string) => invoke<void>("twitch_disconnect_channel", { login }),
    listConnections: () => invoke<[string, ConnectionStatus][]>("twitch_list_connections"),
  },

  config: {
    get: () => invoke<AppConfig>("get_config"),
    updateTwitchClientId: (clientId: string) => invoke<AppConfig>("update_twitch_config", { clientId }),
    updateTts: (tts: TtsConfig) => invoke<AppConfig>("update_tts_config", { tts }),
    updateAudio: (audio: AudioConfig) => invoke<AppConfig>("update_audio_config", { audio }),
    updateFilters: (filters: FiltersConfig) => invoke<AppConfig>("update_filters_config", { filters }),
    updateAntiSpam: (antiSpam: AntiSpamConfig) =>
      invoke<AppConfig>("update_anti_spam_config", { antiSpam }),
    updateOverlay: (overlay: OverlayConfig) => invoke<AppConfig>("update_overlay_config", { overlay }),
    updateGeneral: (general: GeneralConfig) => invoke<AppConfig>("update_general_config", { general }),
    setUserVoice: (login: string, voiceId: string | null) =>
      invoke<AppConfig>("set_user_voice_assignment", { login, voiceId }),
    setRoleVoice: (role: string, voiceId: string | null) =>
      invoke<AppConfig>("set_role_voice_assignment", { role, voiceId }),
    export: (destinationPath: string) => invoke<void>("export_config", { destinationPath }),
    import: (sourcePath: string) => invoke<AppConfig>("import_config", { sourcePath }),
    reset: () => invoke<AppConfig>("reset_config"),
  },

  tts: {
    listInstalledVoices: () => invoke<string[]>("tts_list_installed_voices"),
    checkInstallation: () => invoke<string>("tts_check_installation"),
    ensureInstalled: () => invoke<void>("tts_ensure_installed"),
    testVoice: (params: { text: string; voiceId: string; volume: number; rate: number; pitch: number }) =>
      invoke<void>("tts_test_voice", params),
    listOutputDevices: () => invoke<AudioDeviceInfo[]>("audio_list_output_devices"),
    switchOutputDevice: (deviceName: string | null) =>
      invoke<void>("audio_switch_output_device", { deviceName }),
  },

  stats: {
    session: () => invoke<SessionStatsSnapshot>("get_session_stats"),
    summary: (days: number) => invoke<StatsSummary>("get_stats_summary", { days }),
    history: (limit: number) => invoke<HistoryEntry[]>("get_history", { limit }),
  },

  overlay: {
    start: () => invoke<number>("overlay_start"),
    stop: () => invoke<void>("overlay_stop"),
    isRunning: () => invoke<boolean>("overlay_is_running"),
  },
};

// --- Écoute d'évènements temps réel émis par le backend --------------------
// Fine couche au-dessus de `listen(...)` pour garder un typage fort côté
// composants plutôt que de manipuler des `unknown` partout.
export const events = {
  onChatMessage: (cb: (evt: IncomingChatEvent) => void): Promise<UnlistenFn> =>
    listen<IncomingChatEvent>("twitch://message", (e) => cb(e.payload)),

  onConnectionStatus: (
    cb: (evt: { channel: string; status: ConnectionStatus }) => void,
  ): Promise<UnlistenFn> => listen("twitch://status", (e) => cb(e.payload as any)),

  onNotice: (cb: (evt: { channel: string; text: string }) => void): Promise<UnlistenFn> =>
    listen("twitch://notice", (e) => cb(e.payload as any)),

  onTtsEvent: (cb: (evt: TtsPlaybackEvent) => void): Promise<UnlistenFn> =>
    listen<TtsPlaybackEvent>("tts://event", (e) => cb(e.payload)),

  onPiperInstallProgress: (cb: (evt: InstallProgress) => void): Promise<UnlistenFn> =>
    listen<InstallProgress>("piper://install-progress", (e) => cb(e.payload)),

  onAuthPolling: (cb: () => void): Promise<UnlistenFn> => listen("auth://polling", () => cb()),
  onAuthCompleted: (cb: (login: string) => void): Promise<UnlistenFn> =>
    listen<string>("auth://completed", (e) => cb(e.payload)),
  onAuthFailed: (cb: (error: string) => void): Promise<UnlistenFn> =>
    listen<string>("auth://failed", (e) => cb(e.payload)),
};
