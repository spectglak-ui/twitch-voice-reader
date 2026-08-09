import { create } from "zustand";
import { api } from "@/lib/tauri";
import type { HistoryEntry, SessionStatsSnapshot, StatsSummary } from "@/types/tts";

interface StatsState {
  session: SessionStatsSnapshot | null;
  summary: StatsSummary | null;
  history: HistoryEntry[];

  refreshSession: () => Promise<void>;
  refreshSummary: (days?: number) => Promise<void>;
  refreshHistory: (limit?: number) => Promise<void>;
}

export const useStatsStore = create<StatsState>((set) => ({
  session: null,
  summary: null,
  history: [],

  refreshSession: async () => set({ session: await api.stats.session() }),
  refreshSummary: async (days = 14) => set({ summary: await api.stats.summary(days) }),
  refreshHistory: async (limit = 200) => set({ history: await api.stats.history(limit) }),
}));
