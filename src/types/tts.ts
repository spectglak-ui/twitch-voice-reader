// Miroir de `src-tauri/src/tts/queue.rs`, `stats/mod.rs`, `db/repository.rs`
// et `tts/installer.rs`.

export type InstallProgress =
  | { stage: "CheckingExisting" }
  | { stage: "Downloading"; label: string; percent: number | null }
  | { stage: "Extracting" }
  | { stage: "Verifying" }
  | { stage: "DownloadingVoice"; label: string }
  | { stage: "Done" }
  | { stage: "Error"; message: string };

export type TtsPlaybackEvent =
  | { type: "Started"; message_id: string; display_name: string; text: string; voice_id: string }
  | { type: "Finished"; message_id: string; duration_ms: number }
  | { type: "QueueSizeChanged"; size: number }
  | { type: "Error"; message_id: string; error: string };

export interface SessionStatsSnapshot {
  messages_read: number;
  messages_ignored: number;
  active_users_count: number;
  total_reading_time_ms: number;
}

export interface DailyStats {
  day: string;
  messages_read: number;
  messages_ignored: number;
  reading_time_ms: number;
}

export interface StatsSummary {
  total_messages_read: number;
  total_messages_ignored: number;
  total_reading_time_ms: number;
  active_users_last_30_days: number;
  daily_breakdown: DailyStats[];
}

export interface HistoryEntry {
  id: string;
  channel: string;
  username_login: string;
  display_name: string;
  role: string;
  text: string;
  was_read_aloud: boolean;
  rejection_reason: string | null;
  created_at_ms: number;
}

export interface AudioDeviceInfo {
  name: string;
  is_default: boolean;
}

export interface DeviceCodeResponse {
  device_code: string;
  user_code: string;
  verification_uri: string;
  expires_in: number;
  interval: number;
}
