import { NavLink, Outlet } from "react-router-dom";

const links = [
  { to: "/", label: "Today" },
  { to: "/chat", label: "Chat" },
  { to: "/calendar", label: "Calendar" },
  { to: "/sparks", label: "Sparks" },
];

export function AppShell() {
  return (
    <div className="app-shell">
      <nav className="nav-rail" aria-label="Primary">
        <div className="brand">Buddy</div>
        {links.map((link) => (
          <NavLink
            key={link.to}
            to={link.to}
            end={link.to === "/"}
            className={({ isActive }) => `nav-link${isActive ? " active" : ""}`}
          >
            <span>{link.label}</span>
          </NavLink>
        ))}
      </nav>
      <main className="content">
        <Outlet />
      </main>
      <nav className="mobile-nav" aria-label="Mobile">
        {links.map((link) => (
          <NavLink
            key={link.to}
            to={link.to}
            end={link.to === "/"}
            className={({ isActive }) => `nav-link${isActive ? " active" : ""}`}
          >
            <span>{link.label}</span>
          </NavLink>
        ))}
      </nav>
    </div>
  );
}
