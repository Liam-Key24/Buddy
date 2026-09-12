export function todayIso() {
  return new Date().toISOString().slice(0, 10);
}

export function shortDate(iso: string | null | undefined) {
  if (!iso) return null;
  const d = new Date(`${iso}T00:00:00`);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleDateString(undefined, { day: "numeric", month: "short" });
}

export function formatDay(iso: string) {
  return shortDate(iso) ?? iso;
}

export function gbp(cents: number) {
  return `£${(cents / 100).toFixed(2)}`;
}
