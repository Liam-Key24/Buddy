import { useAppStore } from "../stores/useAppStore";

export function Header() {
  const { currentPage } = useAppStore();

  if (currentPage === "spark") return null;

  const titles: Record<string, string> = {
    dashboard: "Dashboard",
    chat: "Chat",
    code: "Code Agent",
    calendar: "Calendar",
    documents: "Documents",
    fitness: "Fitness & Tracking",
    money: "Money",
    study: "Study Planner",
    todo: "To-Do",
    socials: "Socials",
    settings: "Settings",
  };

  return (
    <header className="flex h-12 shrink-0 items-center border-b border-zinc-800 px-5">
      <h1 className="text-sm font-medium text-zinc-200">
        {titles[currentPage] ?? ""}
      </h1>
    </header>
  );
}
