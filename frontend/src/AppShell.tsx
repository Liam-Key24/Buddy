import {
  CalendarBlank,
  ChatCircle,
  Lightning,
  SquaresFour,
  GearSix,
  Target,
} from "@phosphor-icons/react";
import { NavLink, Outlet } from "react-router-dom";

const links = [
  { to: "/", label: "Today", icon: SquaresFour, end: true },
  { to: "/chat", label: "Chat", icon: ChatCircle },
  { to: "/goals", label: "Goals", icon: Target },
  { to: "/calendar", label: "Calendar", icon: CalendarBlank },
  { to: "/sparks", label: "Sparks", icon: Lightning },
];

export function AppShell() {
  return (
    <div className="app-shell">
      <nav className="nav-rail" aria-label="Primary">
        <div className="brand-mark" title="Buddy">
          B
        </div>
        {links.map((link) => {
          const Icon = link.icon;
          return (
            <NavLink
              key={link.to}
              to={link.to}
              end={link.end}
              title={link.label}
              className={({ isActive }) => `nav-link${isActive ? " active" : ""}`}
            >
              <Icon size={20} weight={link.to === "/" ? "duotone" : "regular"} />
            </NavLink>
          );
        })}
        <div className="nav-spacer" />
        <NavLink
          to="/settings"
          title="Settings"
          className={({ isActive }) => `nav-link${isActive ? " active" : ""}`}
        >
          <GearSix size={20} weight="regular" />
        </NavLink>
      </nav>
      <main className="content">
        <Outlet />
      </main>
      <nav className="mobile-nav" aria-label="Mobile">
        {links.map((link) => {
          const Icon = link.icon;
          return (
            <NavLink
              key={link.to}
              to={link.to}
              end={link.end}
              title={link.label}
              className={({ isActive }) => `nav-link${isActive ? " active" : ""}`}
            >
              <Icon size={22} />
            </NavLink>
          );
        })}
      </nav>
    </div>
  );
}
