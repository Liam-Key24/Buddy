import { useState, type FormEvent } from "react";
import { login, type Me } from "../api";
import { Button } from "../components/ui/Button";
import { Surface } from "../components/ui/Surface";

export function LoginPage({
  onLoggedIn,
  onSubmit = login,
}: {
  onLoggedIn: (user: Me) => void;
  onSubmit?: (username: string, password: string) => Promise<Me>;
}) {
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function handleSubmit(event: FormEvent) {
    event.preventDefault();
    setError(null);
    setBusy(true);
    try {
      const user = await onSubmit(username.trim(), password);
      onLoggedIn(user);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not sign in");
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="grid h-dvh place-items-center bg-page px-4 font-ui text-ink">
      <Surface className="w-full max-w-sm">
        <h1 className="m-0 font-display text-3xl font-medium">Buddy</h1>
        <p className="mt-1 mb-4 text-sm text-muted">Sign in to your private account.</p>
        <form className="flex flex-col gap-3" onSubmit={handleSubmit}>
          <label className="flex flex-col gap-1 text-sm">
            <span className="text-muted">Username</span>
            <input
              autoComplete="username"
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              className="rounded-xl border border-hairline bg-page-deep px-3 py-2 text-ink"
            />
          </label>
          <label className="flex flex-col gap-1 text-sm">
            <span className="text-muted">Password</span>
            <input
              type="password"
              autoComplete="current-password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              className="rounded-xl border border-hairline bg-page-deep px-3 py-2 text-ink"
            />
          </label>
          {error ? <p className="m-0 text-sm text-danger">{error}</p> : null}
          <Button type="submit" tone="primary" block disabled={busy || !username.trim() || !password}>
            {busy ? "Signing in…" : "Sign in"}
          </Button>
        </form>
      </Surface>
    </div>
  );
}
