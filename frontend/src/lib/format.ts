/** Display helpers for contract ids, timestamps, and event fields. */

/** Shorten a long id (contract / address) to `ABCDEF…UVWXYZ`. */
export function truncateMiddle(value: string, head = 6, tail = 6): string {
  if (value.length <= head + tail + 1) return value;
  return `${value.slice(0, head)}…${value.slice(-tail)}`;
}

/** Human "time ago" for an ISO timestamp, falling back to the raw date. */
export function timeAgo(iso: string): string {
  const then = new Date(iso).getTime();
  if (Number.isNaN(then)) return iso;

  const seconds = Math.round((Date.now() - then) / 1000);
  if (seconds < 60) return `${seconds}s ago`;
  const minutes = Math.round(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.round(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.round(hours / 24);
  if (days < 30) return `${days}d ago`;
  return new Date(then).toLocaleDateString();
}

/** Render decoded event fields as a compact `key=value` summary. */
export function summarizeFields(fields: Record<string, string>): string {
  const entries = Object.entries(fields);
  if (entries.length === 0) return "—";
  return entries.map(([k, v]) => `${k}=${truncateMiddle(v, 8, 6)}`).join("  ");
}
