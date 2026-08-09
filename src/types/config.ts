// Miroir exact de `src-tauri/src/config/schema.rs`.

export interface TwitchConfig {
  client_id: string | null;
}

export interface ChannelConfig {
  login: string;
  enabled: boolean;
  auto_reconnect: boolean;
}

export interface TtsConfig {
  piper_binary_path: string | null;
  default_voice_id: string;
  volume: number;
  rate: number;
  pitch: number;
  auto_detect_language: boolean;
  language_voice_map: Record<string, string>;
  read_username_before_message: boolean;
  max_queue_size: number;
}

export interface AudioConfig {
  output_device_name: string | null;
  master_volume: number;
  per_voice_volume: Record<string, number>;
}

export interface RoleFilterConfig {
  subscribers_only: boolean;
  vips_only: boolean;
  moderators_only: boolean;
  broadcaster_only: boolean;
}

export interface FiltersConfig {
  min_length: number;
  max_length: number;
  ignore_emote_only_messages: boolean;
  ignore_links: boolean;
  blacklist_words: string[];
  whitelist_words: string[];
  whitelist_mode_enabled: boolean;
  ignored_users: string[];
  roles: RoleFilterConfig;
}

export interface AntiSpamConfig {
  enabled: boolean;
  max_messages_per_minute: number;
  duplicate_grouping_window_secs: number;
  repetition_threshold: number;
}

export interface VoiceAssignments {
  per_user: Record<string, string>;
  per_role: Record<string, string>;
}

export type OverlayAnimation = "Fade" | "SlideUp" | "Bounce";

export interface OverlayConfig {
  enabled: boolean;
  http_port: number;
  show_avatar: boolean;
  show_username: boolean;
  animation: OverlayAnimation;
}

export type AppTheme = "Dark" | "Light" | "System";

export interface GeneralConfig {
  start_minimized_to_tray: boolean;
  launch_on_system_startup: boolean;
  theme: AppTheme;
  locale: string;
  history_retention_days: number;
}

export interface AppConfig {
  schema_version: number;
  twitch: TwitchConfig;
  channels: ChannelConfig[];
  tts: TtsConfig;
  audio: AudioConfig;
  filters: FiltersConfig;
  anti_spam: AntiSpamConfig;
  voice_assignments: VoiceAssignments;
  overlay: OverlayConfig;
  general: GeneralConfig;
}
