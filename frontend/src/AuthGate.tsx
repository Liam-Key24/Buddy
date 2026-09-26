import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import { fetchMe, login as apiLogin, logout as apiLogout, type Me } from "./api";
import { LoginPage } from "./pages/LoginPage";

type AuthContextValue = {
  me: Me;
  login: (username: string, password: string) => Promise<Me>;
  logout: () => Promise<void>;
};

const AuthContext = createContext<AuthContextValue | null>(null);

export function useAuth(): AuthContextValue {
  const ctx = useContext(AuthContext);
  if (!ctx) throw new Error("useAuth requires AuthGate");
  return ctx;
}

export function AuthGate({ children }: { children: ReactNode }) {
  const [me, setMe] = useState<Me | null | undefined>(undefined);

  useEffect(() => {
    let cancelled = false;
    fetchMe()
      .then((row) => {
        if (!cancelled) setMe(row);
      })
      .catch(() => {
        if (!cancelled) setMe(null);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const login = useCallback(async (username: string, password: string) => {
    const user = await apiLogin(username, password);
    setMe(user);
    return user;
  }, []);

  const logout = useCallback(async () => {
    await apiLogout();
    setMe(null);
  }, []);

  const value = useMemo(
    () => (me ? { me, login, logout } : null),
    [me, login, logout],
  );

  if (me === undefined) {
    return (
      <div className="grid h-dvh place-items-center bg-page font-ui text-muted">Loading…</div>
    );
  }
  if (me === null || !value) {
    return <LoginPage onLoggedIn={(user) => setMe(user)} onSubmit={login} />;
  }
  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
}
