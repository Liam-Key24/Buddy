import { useEffect, useState, type ReactNode } from "react";
import { fetchMe, login, type Me } from "./api";
import { LoginPage } from "./pages/LoginPage";

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

  if (me === undefined) {
    return (
      <div className="grid h-dvh place-items-center bg-page font-ui text-muted">
        Loading…
      </div>
    );
  }
  if (me === null) {
    return (
      <LoginPage
        onLoggedIn={(user) => {
          setMe(user);
        }}
        onSubmit={login}
      />
    );
  }
  return children;
}
