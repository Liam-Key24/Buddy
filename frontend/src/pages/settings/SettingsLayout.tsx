import { CalendarBlank, GearSix, Info } from "@phosphor-icons/react";
import { NavLink, Outlet } from "react-router-dom";
import { cn } from "../../lib/cn";

const NAV = [
  { to: "/settings/about", label: "About", icon: Info },
  { to: "/settings/general", label: "Settings", icon: GearSix },
  { to: "/settings/calendar", label: "Calendar settings", icon: CalendarBlank },
] as const;

export function SettingsLayout() {
  return (
    <section className="settings-shell">
      <aside className="settings-nav" aria-label="Settings sections">
        <p className="settings-nav-eyebrow">Settings</p>
        {NAV.map(({ to, label, icon: Icon }) => (
          <NavLink
            key={to}
            to={to}
            end
            className={({ isActive }) =>
              cn("settings-nav-link", isActive && "settings-nav-link-active")
            }
          >
            <Icon size={16} weight="duotone" className="settings-path-icon" />
            <span className="truncate">{label}</span>
          </NavLink>
        ))}
      </aside>
      <div className="settings-main">
        <Outlet />
      </div>
    </section>
  );
}
