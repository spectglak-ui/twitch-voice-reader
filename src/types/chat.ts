// Miroir exact de `src-tauri/src/twitch/message.rs`. Toute évolution du
// type Rust `ChatMessage` doit être répercutée ici pour garder le contrat
// IPC cohérent (aucune validation de schéma automatique entre les deux
// côtés : c'est un choix assumé pour éviter la complexité d'une génération
// de types, cf. cahier technique section 6).

export type TwitchRole = "viewer" | "subscriber" | "vip" | "moderator" | "broadcaster";

export interface ChatMessage {
  id: string;
  channel: string;
  username_login: string;
  display_name: string;
  color: string | null;
  role: TwitchRole;
  badges: string[];
  text: string;
  text_for_tts: string;
  is_emote_only: boolean;
  is_action: boolean;
  timestamp_ms: number;
}

export type ConnectionStatus = "connecting" | "connected" | "reconnecting" | "disconnected";

export type RejectionReason =
  | "UserIgnored"
  | "TooShort"
  | "TooLong"
  | "EmoteOnly"
  | "ContainsLink"
  | "Blacklisted"
  | "NotWhitelisted"
  | "RoleNotAllowed";

export interface IncomingChatEvent {
  message: ChatMessage;
  wasReadAloud: boolean;
}
