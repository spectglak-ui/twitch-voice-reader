import { create } from "zustand";
import { events } from "@/lib/tauri";
import type { ChatMessage } from "@/types/chat";

const MAX_MESSAGES = 300;

interface CurrentlySpeaking {
  messageId: string;
  displayName: string;
  text: string;
  voiceId: string;
}

interface ChatState {
  messages: (ChatMessage & { wasReadAloud: boolean })[];
  currentlySpeaking: CurrentlySpeaking | null;
  queueSize: number;
  isInitialized: boolean;
  /** Dernière erreur remontée par le pipeline TTS (Piper, audio, timeout).
   * Avant ce correctif, `TtsPlaybackEvent::Error` était reçu mais
   * uniquement utilisé pour réinitialiser `currentlySpeaking` — le message
   * d'erreur lui-même n'était stocké nulle part, donc jamais visible dans
   * l'interface. Un streamer dont le chat cesse silencieusement d'être lu
   * (voix manquante, Piper bloqué, périphérique audio débranché) n'avait
   * aucun moyen de savoir pourquoi. */
  lastTtsError: { message: string; atMs: number } | null;

  init: () => Promise<void>;
  clear: () => void;
  dismissTtsError: () => void;
}

export const useChatStore = create<ChatState>((set, get) => ({
  messages: [],
  currentlySpeaking: null,
  queueSize: 0,
  isInitialized: false,
  lastTtsError: null,

  init: async () => {
    if (get().isInitialized) return;
    set({ isInitialized: true });

    await events.onChatMessage(({ message, wasReadAloud }) => {
      set((state) => ({
        messages: [{ ...message, wasReadAloud }, ...state.messages].slice(0, MAX_MESSAGES),
      }));
    });

    await events.onTtsEvent((evt) => {
      switch (evt.type) {
        case "Started":
          set({
            currentlySpeaking: {
              messageId: evt.message_id,
              displayName: evt.display_name,
              text: evt.text,
              voiceId: evt.voice_id,
            },
          });
          break;
        case "Finished":
          set({ currentlySpeaking: null });
          break;
        case "Error":
          set({ currentlySpeaking: null, lastTtsError: { message: evt.error, atMs: Date.now() } });
          break;
        case "QueueSizeChanged":
          set({ queueSize: evt.size });
          break;
      }
    });
  },

  clear: () => set({ messages: [] }),
  dismissTtsError: () => set({ lastTtsError: null }),
}));
