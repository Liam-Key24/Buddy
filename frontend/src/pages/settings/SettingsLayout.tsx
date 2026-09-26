import { CaretDown, CalendarBlank, GearSix, Info } from "@phosphor-icons/react";
import { useState } from "react";
import { NavLink, Outlet, useLocation } from "react-router-dom";
import { cn } from "../../lib/cn";

const NAV = [
  { to: "/settings/about", label: "About", icon: Info },
  { to: "/settings/general", label: "Settings", icon: GearSix },
  { to: "/settings/calendar", label: "Calendar settings", icon: CalendarBlank },
] as const;

export function SettingsLayout() {
  const location = useLocation();
  const [open, setOpen] = useState(false);
  const current = NAV.find((item) => location.pathname.startsWith(item.to)) ?? NAV[0];
  const CurrentIcon = current.icon;

  return (
    <section className="settings-shell">
      <div className="settings-switcher">
        <button
          type="button"
          className="settings-switcher-btn"
          aria-expanded={open}
          aria-haspopup="listbox"
          onClick={() => setOpen((v) => !v)}
        >
          <CurrentIcon size={16} weight="duotone" className="settings-path-icon" />
          <span className="min-w-0 truncate">{current.label}</span>
          <CaretDown size={14} className={cn("ml-auto text-muted", open && "rotate-180")} />
        </button>
        {open && (
          <div className="settings-switcher-menu" role="listbox" aria-label="Settings sections">
            {NAV.map(({ to, label, icon: Icon }) => (
              <NavLink
                key={to}
                to={to}
                end
                role="option"
                aria-selected={location.pathname.startsWith(to)}
                className={({ isActive }) =>
                  cn("settings-switcher-item", isActive && "settings-switcher-item-active")
                }
                onClick={() => setOpen(false)}
              >
                <Icon size={16} weight="duotone" className="settings-path-icon" />
                <span className="truncate">{label}</span>
              </NavLink>
            ))}
          </div>
        )}
      </div>
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
