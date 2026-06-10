import type { StardexEvent } from "@stardex/types";
import { summarizeFields, timeAgo, truncateMiddle } from "../lib/format";

interface EventTableProps {
  events: StardexEvent[];
}

export function EventTable({ events }: EventTableProps) {
  return (
    <div className="overflow-x-auto rounded-lg border border-[var(--color-border)]">
      <table className="w-full border-collapse text-sm">
        <thead>
          <tr className="bg-[var(--color-surface)] text-left text-xs uppercase tracking-wide text-slate-400">
            <th className="px-4 py-3 font-medium">Kind</th>
            <th className="px-4 py-3 font-medium">Contract</th>
            <th className="px-4 py-3 font-medium">Ledger</th>
            <th className="px-4 py-3 font-medium">Fields</th>
            <th className="px-4 py-3 font-medium">Age</th>
          </tr>
        </thead>
        <tbody>
          {events.map((event) => (
            <tr
              key={event.id}
              className="border-t border-[var(--color-border)] hover:bg-[var(--color-surface)]/50"
            >
              <td className="px-4 py-3">
                <span className="rounded bg-[var(--color-surface)] px-2 py-0.5 font-mono text-xs text-[var(--color-accent)]">
                  {event.kind}
                </span>
              </td>
              <td className="px-4 py-3 font-mono text-xs text-slate-300">
                <span title={event.contractId}>
                  {truncateMiddle(event.contractId)}
                </span>
              </td>
              <td className="px-4 py-3 tabular-nums text-slate-300">
                {event.ledger.toLocaleString()}
              </td>
              <td className="px-4 py-3 font-mono text-xs text-slate-400">
                {summarizeFields(event.fields)}
              </td>
              <td
                className="px-4 py-3 whitespace-nowrap text-slate-400"
                title={event.closedAt}
              >
                {timeAgo(event.closedAt)}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
