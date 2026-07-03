import { create } from "zustand";
import {
  createSpark,
  deleteSpark as apiDeleteSpark,
  fetchStaleSparkCount,
  fetchStaleSparks,
  loadSparks,
  updateSpark,
} from "../lib/api";

export interface Spark {
  id: string;
  content: string;
  tags: string[];
  status: string;
  created_at: number;
  updated_at: number;
  last_nudged_at?: number | null;
  source_conversation_id?: string | null;
}

export const SPARK_CATEGORIES = [
  {
    id: "projects",
    label: "Projects",
    bg: "bg-blue-950",
    border: "border-blue-500/30",
    iconColor: "text-blue-400",
    chip: "bg-blue-500/20 text-blue-300",
  },
  {
    id: "the_land",
    label: "The Land",
    bg: "bg-emerald-950",
    border: "border-emerald-500/30",
    iconColor: "text-emerald-400",
    chip: "bg-emerald-500/20 text-emerald-300",
  },
  {
    id: "the_van",
    label: "The Van",
    bg: "bg-amber-950",
    border: "border-amber-500/30",
    iconColor: "text-amber-400",
    chip: "bg-amber-500/20 text-amber-300",
  },
  {
    id: "general_life",
    label: "General Life",
    bg: "bg-violet-950",
    border: "border-violet-500/30",
    iconColor: "text-violet-400",
    chip: "bg-violet-500/20 text-violet-300",
  },
  {
    id: "travelling",
    label: "Travelling",
    bg: "bg-sky-950",
    border: "border-sky-500/30",
    iconColor: "text-sky-400",
    chip: "bg-sky-500/20 text-sky-300",
  },
] as const;

export type SparkTagId = (typeof SPARK_CATEGORIES)[number]["id"];

export function categoryConfig(tag: string) {
  return SPARK_CATEGORIES.find((c) => c.id === tag);
}

export function tagLabel(tag: string): string {
  return categoryConfig(tag)?.label ?? tag;
}

interface SparkState {
  sparks: Spark[];
  staleSparks: Spark[];
  staleCount: number;
  loading: boolean;
  refresh: () => Promise<void>;
  refreshStale: () => Promise<void>;
  addSpark: (content: string, tags: string[]) => Promise<void>;
  respark: (id: string, content?: string) => Promise<void>;
  archiveSpark: (id: string) => Promise<void>;
  deleteSpark: (id: string) => Promise<void>;
}

export const useSparkStore = create<SparkState>((set, get) => ({
  sparks: [],
  staleSparks: [],
  staleCount: 0,
  loading: false,

  refresh: async () => {
    set({ loading: true });
    try {
      const sparks = await loadSparks("active");
      set({ sparks });
      await get().refreshStale();
    } finally {
      set({ loading: false });
    }
  },

  refreshStale: async () => {
    const [staleSparks, staleCount] = await Promise.all([
      fetchStaleSparks(),
      fetchStaleSparkCount(),
    ]);
    set({ staleSparks, staleCount });
  },

  addSpark: async (content, tags) => {
    await createSpark(content, tags);
    await get().refresh();
  },

  respark: async (id, content) => {
    await updateSpark(id, "respark", content);
    await get().refresh();
  },

  archiveSpark: async (id) => {
    await updateSpark(id, "archive");
    await get().refresh();
  },

  deleteSpark: async (id) => {
    await apiDeleteSpark(id);
    await get().refresh();
  },
}));

export function sparksForTag(sparks: Spark[], tag: string): Spark[] {
  return sparks.filter((s) => s.tags.includes(tag));
}

export function filterSparksByTag(
  sparks: Spark[],
  tag: SparkTagId | null,
): Spark[] {
  if (!tag) return sparks;
  return sparks.filter((s) => s.tags.includes(tag));
}

export function formatSparkDate(createdAt: number): string {
  const ms = createdAt > 1e12 ? createdAt : createdAt * 1000;
  return new Date(ms).toLocaleDateString(undefined, {
    day: "numeric",
    month: "short",
    year: "numeric",
  });
}

export function sparkAgeDays(updatedAt: number): number {
  const ms = updatedAt > 1e12 ? updatedAt : updatedAt * 1000;
  return Math.floor((Date.now() - ms) / (1000 * 60 * 60 * 24));
}
