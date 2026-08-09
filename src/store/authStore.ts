import { create } from "zustand";
import { api, events } from "@/lib/tauri";
import type { DeviceCodeResponse } from "@/types/tts";

interface AuthState {
  currentAccount: string | null;
  pendingDeviceCode: DeviceCodeResponse | null;
  isPolling: boolean;
  error: string | null;

  init: () => Promise<void>;
  startLogin: () => Promise<void>;
  logout: () => Promise<void>;
}

export const useAuthStore = create<AuthState>((set) => ({
  currentAccount: null,
  pendingDeviceCode: null,
  isPolling: false,
  error: null,

  init: async () => {
    const account = await api.auth.currentAccount();
    set({ currentAccount: account });

    await events.onAuthPolling(() => set({ isPolling: true }));
    await events.onAuthCompleted((login) =>
      set({ currentAccount: login, pendingDeviceCode: null, isPolling: false, error: null }),
    );
    await events.onAuthFailed((error) => set({ pendingDeviceCode: null, isPolling: false, error }));
  },

  startLogin: async () => {
    set({ error: null });
    const deviceCode = await api.auth.startLogin();
    set({ pendingDeviceCode: deviceCode });
  },

  logout: async () => {
    await api.auth.logout();
    set({ currentAccount: null });
  },
}));
