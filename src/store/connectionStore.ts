import { create } from "zustand";
import { api, events } from "@/lib/tauri";
import type { ConnectionStatus } from "@/types/chat";

interface NoticeEntry {
  channel: string;
  text: string;
  receivedAtMs: number;
}

interface ConnectionState {
  statuses: Record<string, ConnectionStatus>;
  notices: NoticeEntry[];
  isInitialized: boolean;
  /** Erreur du dernier connect()/disconnect() ayant échoué côté backend.
   * Comme pour le TTS, un `await` sans `catch` masquait totalement un
   * échec de commande — le bouton semblait "ne rien faire". */
  lastActionError: string | null;

  init: () => Promise<void>;
  connect: (login: string) => Promise<void>;
  disconnect: (login: string) => Promise<void>;
  refresh: () => Promise<void>;
  dismissActionError: () => void;
}

const MAX_NOTICES = 50;

function formatError(err: unknown): string {
  if (err && typeof err === "object" && "message" in err) {
    return String((err as { message: unknown }).message);
  }
  return String(err);
}

export const useConnectionStore = create<ConnectionState>((set, get) => ({
  statuses: {},
  notices: [],
  isInitialized: false,
  lastActionError: null,

  init: async () => {
    if (get().isInitialized) return;
    set({ isInitialized: true });

    await get().refresh();

    await events.onConnectionStatus(({ channel, status }) => {
      set((state) => ({ statuses: { ...state.statuses, [channel]: status } }));
    });

    await events.onNotice(({ channel, text }) => {
      set((state) => ({
        notices: [{ channel, text, receivedAtMs: Date.now() }, ...state.notices].slice(0, MAX_NOTICES),
      }));
    });
  },

  connect: async (login) => {
    set((state) => ({ statuses: { ...state.statuses, [login]: "connecting" }, lastActionError: null }));
    try {
      await api.twitch.connectChannel(login);
    } catch (err) {
      set({ lastActionError: `Connexion à #${login} impossible : ${formatError(err)}` });
    }
  },

  disconnect: async (login) => {
    try {
      await api.twitch.disconnectChannel(login);
      set((state) => ({ statuses: { ...state.statuses, [login]: "disconnected" } }));
    } catch (err) {
      set({ lastActionError: `Déconnexion de #${login} impossible : ${formatError(err)}` });
    }
  },

  refresh: async () => {
    const list = await api.twitch.listConnections();
    set({ statuses: Object.fromEntries(list) });
  },

  dismissActionError: () => set({ lastActionError: null }),
}));
