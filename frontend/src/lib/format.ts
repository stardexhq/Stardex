/** Display helpers for contract ids, timestamps, and event fields. */

/** Shorten a long id (contract / address) to `ABCDEF…UVWXYZ`. */
export function truncateMiddle(value: string, head = 6, tail = 6): string {
  if (value.length <= head + tail + 1) return value;
  return `${value.slice(0, head)}…${value.slice(-tail)}`;
}

/** Absolute local date + time for an ISO timestamp, e.g. "Jun 12, 2026, 11:42". */
export function formatDateTime(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

/** Human "time ago" for an ISO timestamp, falling back to the raw date. */
export function timeAgo(iso: string): string {
  const then = new Date(iso).getTime();
  if (Number.isNaN(then)) return iso;

  const seconds = Math.round((Date.now() - then) / 1000);
  if (seconds < 45) return "just now";
  const minutes = Math.round(seconds / 60);
  if (minutes < 60) return `${minutes} minute${minutes === 1 ? "" : "s"} ago`;
  const hours = Math.round(minutes / 60);
  if (hours < 24) return `${hours} hour${hours === 1 ? "" : "s"} ago`;
  const days = Math.round(hours / 24);
  if (days < 30) return `${days} day${days === 1 ? "" : "s"} ago`;
  return new Date(then).toLocaleDateString();
}

/** Render decoded event fields as a compact `key=value` summary. */
export function summarizeFields(fields: Record<string, string>): string {
  const entries = Object.entries(fields);
  if (entries.length === 0) return "-";
  return entries.map(([k, v]) => `${k}=${truncateMiddle(v, 8, 6)}`).join("  ");
}
