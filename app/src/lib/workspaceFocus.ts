import type { AppPage } from "../stores/useAppStore";

export const WORKSPACE_PRIORITY: AppPage[] = [
  "documents",
  "calendar",
  "todo",
  "fitness",
  "money",
  "study",
  "socials",
  "spark",
  "chat",
  "code",
];

export interface WorkspaceFocusPayload {
  pages: string[];
  focus_doc?: string | null;
}

/** Highest-priority workspace to open, or null when already there / nothing to show. */
export function pickWorkspacePage(
  pages: string[],
  current: AppPage,
): AppPage | null {
  const set = new Set(pages.filter(Boolean));
  const primary = WORKSPACE_PRIORITY.find((page) => set.has(page));
  if (!primary || current === primary) return null;
  return primary;
}
