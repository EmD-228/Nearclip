/** "483912" -> "483 912" */
export function formatPairingCode(code: string): string {
  const digits = code.replace(/\D/g, "");
  if (digits.length !== 6) return code;
  return `${digits.slice(0, 3)} ${digits.slice(3)}`;
}

/** Short relative time: "just now", "3m ago", "2h ago", "Yesterday", "12 Mar". */
export function relativeTime(tsMs: number, now: number = Date.now()): string {
  const diff = Math.max(0, now - tsMs);
  const sec = Math.floor(diff / 1000);
  if (sec < 45) return "just now";
  const min = Math.floor(sec / 60);
  if (min < 60) return `${min}m ago`;
  const hr = Math.floor(min / 60);
  if (hr < 24) return `${hr}h ago`;
  const day = Math.floor(hr / 24);
  if (day === 1) return "Yesterday";
  if (day < 7) return `${day}d ago`;
  const d = new Date(tsMs);
  const sameYear = d.getFullYear() === new Date(now).getFullYear();
  return d.toLocaleDateString(undefined, {
    day: "numeric",
    month: "short",
    ...(sameYear ? {} : { year: "numeric" }),
  });
}

export function fullTime(tsMs: number): string {
  return new Date(tsMs).toLocaleString();
}

export function plural(n: number, singular: string, pluralForm = `${singular}s`): string {
  return `${n} ${n === 1 ? singular : pluralForm}`;
}
