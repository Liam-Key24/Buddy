import { useEffect, useState, type ReactNode } from "react";

interface PageTransitionProps {
  page: string;
  render: (page: string) => ReactNode;
}

export function PageTransition({ page, render }: PageTransitionProps) {
  const [shownPage, setShownPage] = useState(page);
  const [phase, setPhase] = useState<"enter" | "exit" | null>("enter");

  useEffect(() => {
    if (page === shownPage) return;

    setPhase("exit");
    const exitTimer = setTimeout(() => {
      setShownPage(page);
      setPhase("enter");
    }, 180);

    return () => clearTimeout(exitTimer);
  }, [page, shownPage]);

  useEffect(() => {
    if (phase !== "enter") return;

    const enterTimer = setTimeout(() => setPhase(null), 220);
    return () => clearTimeout(enterTimer);
  }, [phase, shownPage]);

  const animClass =
    phase === "exit"
      ? "page-exit"
      : phase === "enter"
        ? "page-enter"
        : "";

  const isDashboard = shownPage === "dashboard";

  return (
    <div
      className={`flex min-h-0 flex-1 flex-col overflow-hidden ${animClass} ${
        isDashboard ? "dashboard-page" : ""
      }`}
    >
      {render(shownPage)}
    </div>
  );
}
