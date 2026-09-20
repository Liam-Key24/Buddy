import type { Category } from "../api";

/** Distinct hues so events are easy to tell apart on the grid. */
export const CATEGORY_COLORS = [
  "#7eb8da",
  "#9dde9a",
  "#e8c56b",
  "#c4a1ff",
  "#f0a0a0",
  "#eaf6cb",
  "#8fb9a8",
];

/** Built-in names; Fixed is hidden from the filter (Busy hours covers blocking). */
export const SYSTEM_CATEGORY_NAMES = new Set(["other", "fixed"]);

export const UNCATEGORIZED_KEY = "__none__";

const FILTER_KEY = "buddy.cal.categoryFilters";

export function isSystemCategory(name: string) {
  return SYSTEM_CATEGORY_NAMES.has(name.trim().toLowerCase());
}

export function isFixedCategory(name: string) {
  return name.trim().toLowerCase() === "fixed";
}

export function uniqueCategories(categories: Category[]): Category[] {
  const seen = new Set<string>();
  return categories.filter((c) => {
    const key = c.name.trim().toLowerCase();
    if (!key || seen.has(key) || isFixedCategory(key)) return false;
    seen.add(key);
    return true;
  });
}

export function iconFromName(name: string): string {
  const lower = name.trim().toLowerCase();
  if (lower.includes("climb")) return "mountain";
  if (lower.includes("strength") || lower.includes("workout")) return "barbell";
  if (lower.includes("fixed") || lower.includes("work")) return "lock";
  return "circle";
}

export function loadCategoryFilters(): Record<string, boolean> {
  try {
    const raw = localStorage.getItem(FILTER_KEY);
    if (!raw) return {};
    const parsed = JSON.parse(raw) as Record<string, boolean>;
    return parsed && typeof parsed === "object" ? parsed : {};
  } catch {
    return {};
  }
}

export function saveCategoryFilters(map: Record<string, boolean>) {
  try {
    localStorage.setItem(FILTER_KEY, JSON.stringify(map));
  } catch {
    /* ignore quota */
  }
}

export function mergeCategoryFilters(
  categories: Category[],
  prev: Record<string, boolean>,
): Record<string, boolean> {
  const next = { ...prev };
  for (const cat of categories) {
    if (next[cat.id] === undefined) next[cat.id] = true;
  }
  if (next[UNCATEGORIZED_KEY] === undefined) next[UNCATEGORIZED_KEY] = true;
  return next;
}
