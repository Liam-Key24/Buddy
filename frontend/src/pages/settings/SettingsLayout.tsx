import { CaretDown, CalendarBlank, GearSix, Info, List } from "@phosphor-icons/react";
import {
  createContext,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { NavLink, Outlet, useLocation } from "react-router-dom";
import { cn } from "../../lib/cn";

const NAV = [
  { to: "/settings/about", label: "About", icon: Info },
  { to: "/settings/general", label: "Settings", icon: GearSix },
  { to: "/settings/calendar", label: "Calendar settings", icon: CalendarBlank },
] as const;

type SettingsChrome = {
  mobileMenu: ReactNode;
};

const SettingsChromeContext = createContext<SettingsChrome | null>(null);

export function useSettingsChrome(): SettingsChrome | null {
  return useContext(SettingsChromeContext);
}

function SettingsMobileMenu() {
  const location = useLocation();
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  const current = NAV.find((item) => location.pathname.startsWith(item.to)) ?? NAV[0];

  useEffect(() => {
    if (!open) return;
    function onPointer(e: MouseEvent) {
      if (!rootRef.current?.contains(e.target as Node)) setOpen(false);
    }
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") setOpen(false);
    }
    document.addEventListener("mousedown", onPointer);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onPointer);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  return (
    <div ref={rootRef} className="settings-head-menu lg:hidden">
      <button
        type="button"
        className="settings-head-menu-btn"
        aria-expanded={open}
        aria-haspopup="listbox"
        aria-label={`Settings section: ${current.label}`}
        onClick={() => setOpen((v) => !v)}
      >
        <List size={18} weight="bold" />
        <CaretDown size={12} className={cn("text-muted", open && "rotate-180")} />
      </button>
      {open ? (
        <div className="settings-head-menu-panel" role="listbox" aria-label="Settings sections">
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
      ) : null}
    </div>
  );
}

export function SettingsLayout() {
  const chrome = useMemo(() => ({ mobileMenu: <SettingsMobileMenu /> }), []);

  return (
    <SettingsChromeContext.Provider value={chrome}>
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
    </SettingsChromeContext.Provider>
  );
}
